//! The `body_replace` plugin: strings replaced in upstream response bodies as they stream
//! through, as nginx's `sub_filter` does.
//!
//! Only whole, uncompressed bodies are changed: the upstream is asked for an uncompressed
//! response (no `Accept-Encoding`, no `Range`), and a response that still arrives encoded,
//! or partial, passes unchanged. Compression towards the client happens afterwards. Bodies
//! are not buffered: at most the longest `from` minus one byte is held back between chunks,
//! so a match split across two chunks is still found.

use std::sync::Arc;

use aho_corasick::{AhoCorasick, MatchKind};
use http::{HeaderValue, StatusCode, header};
use pingora::Result;
use pingora::http::ResponseHeader;

use crate::config::BodyReplaceConfig;
use crate::limits::Held;
use crate::plugin::{Gate, Plugin};
use crate::rules::{Matcher, RequestView};

pub struct BodyReplace {
    ac: AhoCorasick,
    to: Vec<Vec<u8>>,
    /// The longest `from`, in bytes.
    longest: usize,
    /// Lowercase; `*` is any type.
    types: Vec<String>,
    matcher: Option<Matcher>,
}

impl BodyReplace {
    pub fn new(c: &BodyReplaceConfig) -> Result<BodyReplace, String> {
        if c.replace.is_empty() {
            return Err("replace: give at least one { from, to }".into());
        }
        if c.replace.iter().any(|p| p.from.is_empty()) {
            return Err("replace: from cannot be empty".into());
        }
        if c.types.is_empty() {
            return Err("types: give at least one content type, or \"*\"".into());
        }
        let types = c
            .types
            .iter()
            .map(|t| {
                let t = t.trim().to_ascii_lowercase();
                if t == "*" || (t.contains('/') && !t.contains(';')) {
                    Ok(t)
                } else {
                    Err(format!("types: {t:?} is not a content type like text/html"))
                }
            })
            .collect::<Result<_, String>>()?;
        let ac = AhoCorasick::builder()
            .match_kind(MatchKind::LeftmostFirst)
            .build(c.replace.iter().map(|p| p.from.as_bytes()))
            .map_err(|e| format!("replace: {e}"))?;
        Ok(BodyReplace {
            ac,
            to: c.replace.iter().map(|p| p.to.as_bytes().to_vec()).collect(),
            longest: c.replace.iter().map(|p| p.from.len()).max().unwrap_or(1),
            types,
            matcher: c.matcher.as_ref().map(Matcher::compile).transpose()?,
        })
    }

    /// Whether this response's content type is one of `types`.
    pub fn takes(&self, resp: &ResponseHeader) -> bool {
        let ct = resp
            .headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.split(';').next().unwrap_or("").trim())
            .unwrap_or("");
        self.types
            .iter()
            .any(|t| t == "*" || (!ct.is_empty() && ct.eq_ignore_ascii_case(t)))
    }
}

/// Whether a response has a whole body that can be rewritten: not bodyless, not a byte
/// range, not compressed.
pub fn rewritable(resp: &ResponseHeader, head: bool) -> bool {
    let s = resp.status;
    if head
        || s.is_informational()
        || s == StatusCode::NO_CONTENT
        || s == StatusCode::NOT_MODIFIED
        || s == StatusCode::PARTIAL_CONTENT
    {
        return false;
    }
    resp.headers
        .get(header::CONTENT_ENCODING)
        .is_none_or(|v| v.as_bytes().eq_ignore_ascii_case(b"identity"))
}

/// Headers of a response whose body is rewritten: its length is no longer known, and the
/// validators describe the upstream's bytes, not these (nginx's `sub_filter` drops them too).
pub fn rewritten_headers(resp: &mut ResponseHeader, http11: bool) -> Result<()> {
    for h in [
        header::CONTENT_LENGTH,
        header::ACCEPT_RANGES,
        header::ETAG,
        header::LAST_MODIFIED,
    ] {
        resp.remove_header(&h);
    }
    // HTTP/1.0 clients read until the connection closes; HTTP/2 frames the body itself.
    if http11 {
        resp.insert_header(
            header::TRANSFER_ENCODING,
            HeaderValue::from_static("chunked"),
        )?;
    }
    Ok(())
}

/// One response body going through one `body_replace`.
pub struct Stream {
    r: Arc<BodyReplace>,
    /// The end of what came so far, where a match may still begin.
    carry: Vec<u8>,
}

impl Stream {
    pub fn new(r: Arc<BodyReplace>) -> Stream {
        Stream {
            r,
            carry: Vec::new(),
        }
    }

    /// What can be sent for the next `chunk`; with `end`, everything still held back.
    pub fn feed(&mut self, chunk: &[u8], end: bool) -> Vec<u8> {
        let mut buf = std::mem::take(&mut self.carry);
        buf.extend_from_slice(chunk);
        // A match starting before `hold` fits in `buf` whatever comes next, so it is the
        // match the whole body would give there. Later ones wait for more bytes.
        let hold = if end {
            buf.len()
        } else {
            buf.len().saturating_sub(self.r.longest - 1)
        };
        let mut out = Vec::with_capacity(buf.len());
        let mut at = 0;
        for m in self.r.ac.find_iter(&buf) {
            if m.start() >= hold {
                break;
            }
            out.extend_from_slice(&buf[at..m.start()]);
            out.extend_from_slice(&self.r.to[m.pattern().as_usize()]);
            at = m.end();
        }
        let keep = at.max(hold);
        out.extend_from_slice(&buf[at..keep]);
        buf.drain(..keep);
        self.carry = buf;
        out
    }
}

/// `body_replace` as a plugin: marks the requests whose responses are rewritten.
pub struct BodyReplacePlugin(Arc<BodyReplace>);

impl BodyReplacePlugin {
    pub fn new(c: &BodyReplaceConfig) -> Result<BodyReplacePlugin, String> {
        Ok(BodyReplacePlugin(Arc::new(BodyReplace::new(c)?)))
    }
}

impl Plugin for BodyReplacePlugin {
    fn name(&self) -> &'static str {
        "body_replace"
    }

    fn needs_path(&self) -> bool {
        self.0.matcher.is_some()
    }

    fn gate(&self, req: &RequestView<'_>, _held: &mut Vec<Held>) -> Gate {
        match &self.0.matcher {
            Some(m) if !m.is_match(req) => Gate::Pass,
            _ => Gate::Replace(self.0.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ReplacePair;

    fn replacer(pairs: &[(&str, &str)]) -> Arc<BodyReplace> {
        Arc::new(
            BodyReplace::new(&BodyReplaceConfig {
                replace: pairs
                    .iter()
                    .map(|(f, t)| ReplacePair {
                        from: f.to_string(),
                        to: t.to_string(),
                    })
                    .collect(),
                types: vec!["text/html".into()],
                matcher: None,
            })
            .unwrap(),
        )
    }

    fn whole(r: &Arc<BodyReplace>, body: &[u8]) -> Vec<u8> {
        Stream::new(r.clone()).feed(body, true)
    }

    #[test]
    fn replaces_every_occurrence_first_pattern_wins() {
        let r = replacer(&[("http://old", "https://new"), ("old", "OLD"), ("ab", "x")]);
        assert_eq!(
            whole(&r, b"<a href=http://old/>old http://old</a>"),
            b"<a href=https://new/>OLD https://new</a>"
        );
        assert_eq!(whole(&r, b"aab abab"), b"ax xx");
        assert_eq!(whole(&r, b""), b"");
    }

    /// Every way of cutting the body into chunks gives what the whole body gives.
    #[test]
    fn chunk_boundaries_do_not_matter() {
        let r = replacer(&[("abcab", "1"), ("bca", "22"), ("c", "333"), ("aa", "")]);
        let bodies: [&[u8]; 4] = [b"abcabcabca", b"aabcaabcabaa", b"xxabcabxx", b"cccabcab"];
        for body in bodies {
            let want = whole(&r, body);
            // Two cuts anywhere, including empty chunks.
            for i in 0..=body.len() {
                for j in i..=body.len() {
                    let mut s = Stream::new(r.clone());
                    let mut got = s.feed(&body[..i], false);
                    got.extend(s.feed(&body[i..j], false));
                    got.extend(s.feed(&body[j..], false));
                    got.extend(s.feed(b"", true));
                    assert_eq!(got, want, "{body:?} cut at {i}, {j}");
                }
            }
            // One byte at a time.
            let mut s = Stream::new(r.clone());
            let mut got = Vec::new();
            for b in body {
                got.extend(s.feed(std::slice::from_ref(b), false));
            }
            got.extend(s.feed(b"", true));
            assert_eq!(got, want);
        }
    }

    #[test]
    fn holds_back_no_more_than_needed() {
        let r = replacer(&[("needle", "pin")]);
        let mut s = Stream::new(r);
        // Only the last 5 bytes (the longest `from` minus one) wait for the next chunk.
        assert_eq!(s.feed(b"hay hay needle hay nee", false), b"hay hay pin ha");
        assert_eq!(s.feed(b"dle!", true), b"y pin!");
    }

    #[test]
    fn which_responses() {
        let r = replacer(&[("a", "b")]);
        let resp = |status: u16, ct: Option<&str>, ce: Option<&str>| {
            let mut h = ResponseHeader::build(status, None).unwrap();
            if let Some(ct) = ct {
                h.insert_header(header::CONTENT_TYPE, ct).unwrap();
            }
            if let Some(ce) = ce {
                h.insert_header(header::CONTENT_ENCODING, ce).unwrap();
            }
            h
        };
        let html = resp(200, Some("Text/HTML; charset=utf-8"), None);
        assert!(r.takes(&html) && rewritable(&html, false));
        assert!(!rewritable(&html, true));
        assert!(!r.takes(&resp(200, Some("application/json"), None)));
        assert!(!r.takes(&resp(200, None, None)));
        assert!(rewritable(&resp(404, Some("text/html"), None), false));
        assert!(!rewritable(&resp(206, Some("text/html"), None), false));
        assert!(!rewritable(&resp(304, Some("text/html"), None), false));
        assert!(!rewritable(
            &resp(200, Some("text/html"), Some("gzip")),
            false
        ));
        assert!(rewritable(
            &resp(200, Some("text/html"), Some("identity")),
            false
        ));

        let mut h = resp(200, Some("text/html"), None);
        h.insert_header(header::CONTENT_LENGTH, "10").unwrap();
        h.insert_header(header::ETAG, "\"x\"").unwrap();
        rewritten_headers(&mut h, true).unwrap();
        assert!(h.headers.get(header::CONTENT_LENGTH).is_none());
        assert!(h.headers.get(header::ETAG).is_none());
        assert_eq!(h.headers[header::TRANSFER_ENCODING], "chunked");
    }

    #[test]
    fn bad_configs() {
        let cfg = |pairs: Vec<(&str, &str)>, types: Vec<&str>| BodyReplaceConfig {
            replace: pairs
                .into_iter()
                .map(|(f, t)| ReplacePair {
                    from: f.into(),
                    to: t.into(),
                })
                .collect(),
            types: types.into_iter().map(String::from).collect(),
            matcher: None,
        };
        assert!(BodyReplace::new(&cfg(vec![], vec!["text/html"])).is_err());
        assert!(BodyReplace::new(&cfg(vec![("", "x")], vec!["text/html"])).is_err());
        assert!(BodyReplace::new(&cfg(vec![("a", "x")], vec![])).is_err());
        assert!(BodyReplace::new(&cfg(vec![("a", "x")], vec!["html"])).is_err());
        let any = BodyReplace::new(&cfg(vec![("a", "x")], vec!["*"])).unwrap();
        assert!(any.takes(&ResponseHeader::build(200, None).unwrap()));
    }
}
