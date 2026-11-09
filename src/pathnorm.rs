//! Request path normalization for rule matching, equivalent to nginx's `$uri`.
//!
//! Rules match the decoded path with `//`, `.` and `..` resolved, so `/%5F/`, `//_/` and
//! `/a/../_/` cannot slip past a rule for `/_/`. The request is still forwarded unchanged.

#[derive(Debug, PartialEq, Eq)]
pub enum PathError {
    /// Bad percent escape, NUL byte, or `..` above the root: answered with 400 like nginx.
    Invalid,
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn percent_decode(raw: &[u8]) -> Result<Vec<u8>, PathError> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        let b = raw[i];
        if b == b'%' {
            let hi = raw
                .get(i + 1)
                .copied()
                .and_then(hex)
                .ok_or(PathError::Invalid)?;
            let lo = raw
                .get(i + 2)
                .copied()
                .and_then(hex)
                .ok_or(PathError::Invalid)?;
            out.push(hi << 4 | lo);
            i += 3;
        } else {
            out.push(b);
            i += 1;
        }
    }
    if out.contains(&0) {
        return Err(PathError::Invalid);
    }
    Ok(out)
}

/// Normalize the path part of a request target (no query string).
pub fn normalize(raw_path: &[u8]) -> Result<Vec<u8>, PathError> {
    let decoded = percent_decode(raw_path)?;
    let mut segments: Vec<&[u8]> = Vec::new();
    let mut trailing_slash = decoded.ends_with(b"/");
    for seg in decoded.split(|b| *b == b'/') {
        match seg {
            b"" => {}
            b"." => trailing_slash = true,
            b".." => {
                segments.pop().ok_or(PathError::Invalid)?;
                trailing_slash = true;
            }
            s => {
                segments.push(s);
                trailing_slash = false;
            }
        }
    }
    if decoded.ends_with(b"/") {
        trailing_slash = true;
    }
    let mut out = Vec::with_capacity(decoded.len() + 1);
    for s in &segments {
        out.push(b'/');
        out.extend_from_slice(s);
    }
    if out.is_empty() || trailing_slash {
        out.push(b'/');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> String {
        String::from_utf8(normalize(s.as_bytes()).unwrap()).unwrap()
    }

    #[test]
    fn resolves_like_nginx() {
        assert_eq!(n("/"), "/");
        assert_eq!(n(""), "/");
        assert_eq!(n("/_/"), "/_/");
        assert_eq!(n("/_"), "/_");
        assert_eq!(n("/%5F/"), "/_/");
        assert_eq!(n("/%5f"), "/_");
        assert_eq!(n("//_//x"), "/_/x");
        assert_eq!(n("/a/../_/"), "/_/");
        assert_eq!(n("/a/./b"), "/a/b");
        assert_eq!(n("/a/b/.."), "/a/");
        assert_eq!(
            n("/api/collections/%5Fsuperusers/records"),
            "/api/collections/_superusers/records"
        );
        assert_eq!(n("/wp-content/debug.log"), "/wp-content/debug.log");
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(normalize(b"/../etc/passwd"), Err(PathError::Invalid));
        assert_eq!(normalize(b"/%2e%2e/x"), Err(PathError::Invalid));
        assert_eq!(normalize(b"/a%00b"), Err(PathError::Invalid));
        assert_eq!(normalize(b"/a%zz"), Err(PathError::Invalid));
        assert_eq!(normalize(b"/a%2"), Err(PathError::Invalid));
    }
}
