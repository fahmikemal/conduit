//! Static files for sites with `files:`.
//!
//! Request paths are decoded and normalized (`pathnorm`) before they reach the disk, and every
//! file is opened with `openat2(RESOLVE_BENEATH)` relative to the root: `..`, absolute
//! symlinks and symlinks leading out of the root all fail in the kernel, so nothing outside
//! the root is served even if a link is planted inside it. Dotfiles (except `.well-known`)
//! and `hide:` names are treated as missing.
//!
//! Disk work runs on Tokio's blocking pool: one hop opens, stats and, for files up to one
//! chunk, reads; larger files are read a chunk per hop while the previous chunk is written.

use std::ffi::{CString, OsStr};
use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use http::{HeaderValue, Method, header};
use pingora::http::{RequestHeader, ResponseHeader};
use pingora::{Error, ErrorType, Result};

use crate::config::FilesConfig;
use crate::pathnorm;
use crate::plugin::Local;
use crate::rules::{Matcher, RequestView};

/// Bytes read per blocking-pool hop, and the largest file read in the same hop as its open.
const CHUNK: usize = 128 << 10;
/// Directory listings stop here.
const LIST_LIMIT: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Br,
    Zstd,
    Gzip,
}

impl Encoding {
    fn parse(s: &str) -> Result<Encoding, String> {
        match s {
            "br" => Ok(Encoding::Br),
            "zstd" => Ok(Encoding::Zstd),
            "gzip" => Ok(Encoding::Gzip),
            _ => Err(format!(
                "precompressed: unknown encoding {s:?}: use br, zstd or gzip"
            )),
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Encoding::Br => ".br",
            Encoding::Zstd => ".zst",
            Encoding::Gzip => ".gz",
        }
    }

    fn token(self) -> &'static str {
        match self {
            Encoding::Br => "br",
            Encoding::Zstd => "zstd",
            Encoding::Gzip => "gzip",
        }
    }

    fn bit(self) -> u8 {
        match self {
            Encoding::Br => 1,
            Encoding::Zstd => 2,
            Encoding::Gzip => 4,
        }
    }
}

/// The encodings a client accepts, from `Accept-Encoding` (`q=0` refuses one).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Accepted(u8);

impl Accepted {
    pub fn from_request(req: &RequestHeader) -> Accepted {
        let mut yes = 0u8;
        let mut no = 0u8;
        let mut any = false;
        for v in req.headers.get_all(header::ACCEPT_ENCODING) {
            for item in v.as_bytes().split(|b| *b == b',') {
                let mut parts = item.split(|b| *b == b';');
                let name = parts.next().unwrap_or_default().trim_ascii();
                let refused = parts.any(|p| {
                    let p = p.trim_ascii();
                    p.len() >= 2
                        && p[..2].eq_ignore_ascii_case(b"q=")
                        && p[2..].iter().all(|b| *b == b'0' || *b == b'.')
                });
                let bit = match name.to_ascii_lowercase().as_slice() {
                    b"br" => 1,
                    b"zstd" => 2,
                    b"gzip" | b"x-gzip" => 4,
                    b"*" => {
                        any = !refused;
                        0
                    }
                    _ => 0,
                };
                if refused {
                    no |= bit;
                } else {
                    yes |= bit;
                }
            }
        }
        if any {
            yes |= 7;
        }
        Accepted(yes & !no)
    }

    fn accepts(self, e: Encoding) -> bool {
        self.0 & e.bit() != 0
    }

    /// The accepted encodings as a bit set (br 1, zstd 2, gzip 4).
    pub fn bits(self) -> u8 {
        self.0
    }

    /// Whether the client takes a body in this content coding; `None` for codings other
    /// than br, zstd and gzip.
    pub fn accepts_coding(self, coding: &str) -> Option<bool> {
        Encoding::parse(&coding.trim().to_ascii_lowercase())
            .ok()
            .map(|e| self.accepts(e))
    }
}

#[derive(Debug)]
enum Part {
    Lit(Vec<u8>),
    Path,
}

/// One entry of `try_files`.
#[derive(Debug)]
struct Candidate {
    parts: Vec<Part>,
    /// Written as `{path}` alone: the request path itself, which may be redirected to
    /// its canonical form (trailing slash on directories, none on files).
    is_path: bool,
}

impl Candidate {
    fn parse(s: &str) -> Result<Candidate, String> {
        if !(s.starts_with('/') || s.starts_with("{path}")) {
            return Err(format!("try_files {s:?}: start with / or {{path}}"));
        }
        let mut parts = Vec::new();
        let mut rest = s;
        while let Some(i) = rest.find("{path}") {
            if i > 0 {
                parts.push(Part::Lit(rest.as_bytes()[..i].to_vec()));
            }
            parts.push(Part::Path);
            rest = &rest[i + 6..];
        }
        if rest.contains(['{', '}']) {
            return Err(format!("try_files {s:?}: only {{path}} is known"));
        }
        if !rest.is_empty() {
            parts.push(Part::Lit(rest.as_bytes().to_vec()));
        }
        Ok(Candidate {
            is_path: s == "{path}",
            parts,
        })
    }

    fn expand(&self, path: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(path.len() + 16);
        for p in &self.parts {
            match p {
                Part::Lit(l) => out.extend_from_slice(l),
                Part::Path => out.extend_from_slice(path),
            }
        }
        out
    }

    /// Written with a trailing `/`: only a directory (its index) matches.
    fn wants_dir(&self) -> bool {
        matches!(self.parts.last(), Some(Part::Lit(l)) if l.ends_with(b"/"))
    }
}

#[derive(Debug)]
enum Hide {
    Name(Vec<u8>),
    Suffix(Vec<u8>),
}

impl Hide {
    fn parse(s: &str) -> Result<Hide, String> {
        if s.is_empty() || s.contains('/') {
            return Err(format!(
                "hide {s:?}: give a file or directory name, or *.ext"
            ));
        }
        Ok(match s.strip_prefix('*') {
            Some(suffix) if !suffix.is_empty() && !suffix.contains('*') => {
                Hide::Suffix(suffix.as_bytes().to_vec())
            }
            Some(_) => return Err(format!("hide {s:?}: only *.ext patterns are known")),
            None if s.contains('*') => {
                return Err(format!("hide {s:?}: only *.ext patterns are known"));
            }
            None => Hide::Name(s.as_bytes().to_vec()),
        })
    }

    fn hides(&self, name: &[u8]) -> bool {
        match self {
            Hide::Name(n) => name == n.as_slice(),
            Hide::Suffix(s) => name.ends_with(s),
        }
    }
}

/// A site's compiled `files:` settings.
#[derive(Debug)]
pub struct Files {
    pub root: PathBuf,
    matcher: Option<Matcher>,
    /// Normalized, without a trailing slash.
    strip_prefix: Option<Vec<u8>>,
    index: Vec<String>,
    try_files: Vec<Candidate>,
    browse: bool,
    precompressed: Vec<Encoding>,
    hide: Vec<Hide>,
    pub pass_thru: bool,
}

impl Files {
    pub fn compile(c: &FilesConfig, has_upstream: bool) -> Result<Files, String> {
        let root = PathBuf::from(&c.root);
        if !root.is_dir() {
            return Err(format!("files.root {} is not a directory", c.root));
        }
        if !has_upstream && (c.matcher.is_some() || c.pass_thru) {
            return Err(
                "files: match and pass_thru send other requests upstream; the site has no upstream"
                    .into(),
            );
        }
        let strip_prefix = match &c.strip_prefix {
            None => None,
            Some(p) => {
                let n = pathnorm::normalize(p.as_bytes())
                    .ok()
                    .filter(|_| p.starts_with('/'))
                    .ok_or_else(|| {
                        format!("files.strip_prefix {p:?} must be a path like /static")
                    })?;
                let n = n.strip_suffix(b"/").unwrap_or(&n).to_vec();
                (!n.is_empty()).then_some(n)
            }
        };
        for name in &c.index {
            if name.is_empty() || name.contains('/') || name.starts_with('.') {
                return Err(format!("files.index {name:?} must be a plain file name"));
            }
        }
        let mut precompressed = Vec::new();
        for e in &c.precompressed {
            let e = Encoding::parse(e)?;
            if !precompressed.contains(&e) {
                precompressed.push(e);
            }
        }
        Ok(Files {
            root,
            matcher: c.matcher.as_ref().map(Matcher::compile).transpose()?,
            strip_prefix,
            index: c.index.clone(),
            try_files: c
                .try_files
                .iter()
                .map(|s| Candidate::parse(s))
                .collect::<Result<_, _>>()?,
            browse: c.browse,
            precompressed,
            hide: c
                .hide
                .iter()
                .map(|s| Hide::parse(s))
                .collect::<Result<_, _>>()?,
            pass_thru: c.pass_thru,
        })
    }

    /// Whether the file server answers this request (otherwise it goes upstream).
    pub fn takes(&self, req: &RequestView<'_>) -> bool {
        self.matcher.as_ref().is_none_or(|m| m.is_match(req))
    }

    fn hidden(&self, path: &[u8]) -> bool {
        path.split(|b| *b == b'/').any(|seg| {
            (seg.first() == Some(&b'.') && seg != b".well-known")
                || self.hide.iter().any(|h| h.hides(seg))
        })
    }

    /// Find what to answer for a GET or HEAD of `path` (normalized). Blocking: run it on the
    /// blocking pool.
    fn find(&self, req: &Lookup) -> Found {
        let path = match &self.strip_prefix {
            None => &req.path[..],
            Some(p) => match req.path.strip_prefix(p.as_slice()) {
                Some([]) => b"/",
                Some(rest) if rest.starts_with(b"/") => rest,
                _ => return Found::Missing,
            },
        };
        let direct = [Candidate {
            parts: vec![Part::Path],
            is_path: true,
        }];
        let candidates = if self.try_files.is_empty() {
            &direct[..]
        } else {
            &self.try_files[..]
        };
        for c in candidates {
            let expanded = c.expand(path);
            let Ok(target) = pathnorm::normalize(&expanded) else {
                continue;
            };
            if self.hidden(&target) {
                continue;
            }
            // Opened without its trailing slash, so `/file/` finds the file and redirects.
            let slash = target.len() > 1 && target.ends_with(b"/");
            let file = match open_beneath(&self.root, &target[..target.len() - slash as usize]) {
                Ok(f) => f,
                Err(e) => match e.kind() {
                    io::ErrorKind::PermissionDenied => return Found::Status(403),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory => continue,
                    _ if e.raw_os_error() == Some(libc::ELOOP) => continue,
                    _ => {
                        log::warn!("files: {}: {e}", show(&target));
                        return Found::Status(500);
                    }
                },
            };
            let meta = match file.metadata() {
                Ok(m) => m,
                Err(e) => {
                    log::warn!("files: {}: {e}", show(&target));
                    return Found::Status(500);
                }
            };
            let canonical = c.is_path;
            if meta.is_dir() {
                if c.is_path && !c.wants_dir() && !self.try_files.is_empty() {
                    // `{path}` in try_files names files only; `{path}/` names directories.
                    continue;
                }
                if canonical && !req.raw_path.ends_with(b"/") {
                    return Found::Redirect(with_query(
                        &[req.raw_path.as_slice(), b"/"].concat(),
                        &req.query,
                    ));
                }
                for name in &self.index {
                    let mut p = target.clone();
                    if !p.ends_with(b"/") {
                        p.push(b'/');
                    }
                    p.extend_from_slice(name.as_bytes());
                    match open_beneath(&self.root, &p) {
                        Ok(f) if f.metadata().is_ok_and(|m| m.is_file()) => {
                            return self.file(f, &p, req.accepted);
                        }
                        _ => {}
                    }
                }
                if self.browse {
                    return listing(&self.root, &target, &req.path, |name| {
                        name.first() == Some(&b'.') || self.hide.iter().any(|h| h.hides(name))
                    });
                }
                continue;
            }
            if !meta.is_file() || c.wants_dir() {
                continue;
            }
            if slash {
                if !canonical {
                    continue;
                }
                let trimmed = req.raw_path.strip_suffix(b"/").unwrap_or(&req.raw_path);
                return Found::Redirect(with_query(trimmed, &req.query));
            }
            return self.file(file, &target, req.accepted);
        }
        Found::Missing
    }

    /// A regular file, or a precompressed sibling the client accepts.
    fn file(&self, file: File, path: &[u8], accepted: Accepted) -> Found {
        let vary = !self.precompressed.is_empty();
        for &enc in &self.precompressed {
            if !accepted.accepts(enc) {
                continue;
            }
            let mut p = path.to_vec();
            p.extend_from_slice(enc.suffix().as_bytes());
            if let Ok(f) = open_beneath(&self.root, &p)
                && let Ok(m) = f.metadata()
                && m.is_file()
            {
                return reply(f, &m, path, Some(enc), vary);
            }
        }
        match file.metadata() {
            Ok(m) => reply(file, &m, path, None, vary),
            Err(_) => Found::Status(500),
        }
    }

    /// Look the request up on the blocking pool.
    pub async fn lookup(self: &Arc<Self>, req: Lookup) -> Found {
        let files = self.clone();
        match tokio::task::spawn_blocking(move || files.find(&req)).await {
            Ok(found) => found,
            Err(e) => {
                log::error!("files: lookup task failed: {e}");
                Found::Status(500)
            }
        }
    }
}

/// What `find` needs to know about the request.
pub struct Lookup {
    /// Decoded and normalized.
    pub path: Vec<u8>,
    /// As received, for redirects.
    pub raw_path: Vec<u8>,
    pub query: Vec<u8>,
    pub accepted: Accepted,
}

/// A redirect target: the path as received plus the query. Leading slashes (and the
/// backslashes browsers read as slashes) collapse to one, so `//host/` can never become a
/// redirect to another host.
fn with_query(path: &[u8], query: &[u8]) -> Vec<u8> {
    let rest = path.trim_ascii_start();
    let rest = &rest[rest
        .iter()
        .take_while(|b| matches!(b, b'/' | b'\\'))
        .count()..];
    let mut l = Vec::with_capacity(rest.len() + query.len() + 2);
    l.push(b'/');
    l.extend_from_slice(rest);
    if !query.is_empty() {
        l.push(b'?');
        l.extend_from_slice(query);
    }
    l
}

fn show(p: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(p)
}

pub enum Found {
    File(FileReply),
    /// Location of the canonical URL.
    Redirect(Vec<u8>),
    Listing(Bytes),
    Status(u16),
    Missing,
}

impl Found {
    /// The answer for anything but a file to send.
    pub fn into_local(self) -> Option<Local> {
        match self {
            Found::File(_) | Found::Missing => None,
            Found::Redirect(location) => Some(Local::redirect(308, &location)),
            Found::Listing(body) => Some(Local::with_body(
                200,
                vec![
                    (
                        header::CONTENT_TYPE,
                        HeaderValue::from_static("text/html; charset=utf-8"),
                    ),
                    (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
                ],
                body,
            )),
            Found::Status(code) => Some(Local::status(code)),
        }
    }
}

pub struct FileReply {
    file: Arc<File>,
    len: u64,
    modified: SystemTime,
    content_type: HeaderValue,
    encoding: Option<Encoding>,
    etag: HeaderValue,
    last_modified: HeaderValue,
    vary: bool,
    /// The whole body, when it fit in the first read.
    body: Option<Bytes>,
}

fn reply(
    file: File,
    meta: &std::fs::Metadata,
    path: &[u8],
    enc: Option<Encoding>,
    vary: bool,
) -> Found {
    let len = meta.len();
    let modified = meta.modified().unwrap_or(UNIX_EPOCH);
    let nanos = modified
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_nanos();
    let etag = match enc {
        None => format!("\"{nanos:x}-{len:x}\""),
        Some(e) => format!("\"{nanos:x}-{len:x}-{}\"", e.token()),
    };
    let body = if len as usize <= CHUNK {
        match read_exact_at(&file, 0, len as usize) {
            Ok(b) => Some(b),
            Err(e) => {
                log::warn!("files: {}: {e}", show(path));
                return Found::Status(500);
            }
        }
    } else {
        None
    };
    Found::File(FileReply {
        file: Arc::new(file),
        len,
        modified,
        content_type: content_type(path),
        encoding: enc,
        etag: HeaderValue::from_str(&etag).expect("hex digits"),
        last_modified: HeaderValue::from_str(&httpdate::fmt_http_date(modified))
            .expect("http date"),
        vary,
        body,
    })
}

fn read_exact_at(file: &File, offset: u64, len: usize) -> io::Result<Bytes> {
    let mut buf = vec![0u8; len];
    let mut done = 0;
    while done < len {
        match file.read_at(&mut buf[done..], offset + done as u64) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "file shrank while being served",
                ));
            }
            Ok(n) => done += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(Bytes::from(buf))
}

/// Set when the kernel (or a container's seccomp filter) has no openat2.
static NO_OPENAT2: AtomicBool = AtomicBool::new(false);

/// Open `rel` (normalized, `/`-rooted) under `root`, never resolving to anything outside it.
fn open_beneath(root: &Path, rel: &[u8]) -> io::Result<File> {
    let rel = rel.strip_prefix(b"/").unwrap_or(rel);
    let rel: &[u8] = if rel.is_empty() { b"." } else { rel };
    let flags = libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NONBLOCK | libc::O_NOCTTY;
    if !NO_OPENAT2.load(Relaxed) {
        let dir = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_PATH | libc::O_DIRECTORY)
            .open(root)?;
        let c = CString::new(rel).map_err(|_| io::Error::from(io::ErrorKind::NotFound))?;
        // SAFETY: open_how is plain data; zero is a valid value for every field.
        let mut how: libc::open_how = unsafe { std::mem::zeroed() };
        how.flags = flags as u64;
        how.resolve = libc::RESOLVE_BENEATH | libc::RESOLVE_NO_MAGICLINKS;
        // SAFETY: valid directory fd, NUL-terminated path, and a correctly sized open_how.
        let fd = unsafe {
            libc::syscall(
                libc::SYS_openat2,
                dir.as_raw_fd(),
                c.as_ptr(),
                &how as *const libc::open_how,
                std::mem::size_of::<libc::open_how>(),
            )
        };
        if fd >= 0 {
            // SAFETY: the kernel just returned this descriptor to us.
            return Ok(unsafe { File::from_raw_fd(fd as RawFd) });
        }
        let e = io::Error::last_os_error();
        match e.raw_os_error() {
            Some(libc::ENOSYS | libc::EPERM) => {
                log::warn!(
                    "files: openat2 unavailable ({e}); checking paths with realpath instead"
                );
                NO_OPENAT2.store(true, Relaxed);
            }
            // The path leads out of the root (a symlink); as if it did not exist.
            Some(libc::EXDEV) => return Err(io::ErrorKind::NotFound.into()),
            _ => return Err(e),
        }
    }
    let real = std::fs::canonicalize(root.join(OsStr::from_bytes(rel)))?;
    if !real.starts_with(std::fs::canonicalize(root)?) {
        return Err(io::ErrorKind::NotFound.into());
    }
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY)
        .open(real)
}

fn content_type(path: &[u8]) -> HeaderValue {
    let name = path.rsplit(|b| *b == b'/').next().unwrap_or(path);
    let ext = match name.iter().rposition(|b| *b == b'.') {
        Some(i) if i > 0 => name[i + 1..].to_ascii_lowercase(),
        _ => Vec::new(),
    };
    let t = match ext.as_slice() {
        b"html" | b"htm" | b"shtml" => "text/html; charset=utf-8",
        b"css" => "text/css; charset=utf-8",
        b"js" | b"mjs" | b"cjs" => "text/javascript; charset=utf-8",
        b"json" | b"map" => "application/json",
        b"webmanifest" => "application/manifest+json",
        b"xml" => "application/xml",
        b"rss" => "application/rss+xml",
        b"atom" => "application/atom+xml",
        b"txt" | b"text" | b"log" => "text/plain; charset=utf-8",
        b"md" | b"markdown" => "text/markdown; charset=utf-8",
        b"csv" => "text/csv; charset=utf-8",
        b"yaml" | b"yml" => "application/yaml",
        b"toml" => "application/toml",
        b"svg" | b"svgz" => "image/svg+xml",
        b"png" => "image/png",
        b"jpg" | b"jpeg" | b"jfif" => "image/jpeg",
        b"gif" => "image/gif",
        b"webp" => "image/webp",
        b"avif" => "image/avif",
        b"ico" => "image/x-icon",
        b"bmp" => "image/bmp",
        b"tif" | b"tiff" => "image/tiff",
        b"woff" => "font/woff",
        b"woff2" => "font/woff2",
        b"ttf" => "font/ttf",
        b"otf" => "font/otf",
        b"eot" => "application/vnd.ms-fontobject",
        b"wasm" => "application/wasm",
        b"pdf" => "application/pdf",
        b"zip" => "application/zip",
        b"gz" | b"tgz" => "application/gzip",
        b"br" => "application/x-brotli",
        b"zst" => "application/zstd",
        b"tar" => "application/x-tar",
        b"7z" => "application/x-7z-compressed",
        b"rar" => "application/vnd.rar",
        b"apk" => "application/vnd.android.package-archive",
        b"jar" => "application/java-archive",
        b"doc" => "application/msword",
        b"docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        b"xls" => "application/vnd.ms-excel",
        b"xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        b"ppt" => "application/vnd.ms-powerpoint",
        b"pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        b"mp4" | b"m4v" => "video/mp4",
        b"webm" => "video/webm",
        b"ogv" => "video/ogg",
        b"mov" => "video/quicktime",
        b"ts" => "video/mp2t",
        b"m3u8" => "application/vnd.apple.mpegurl",
        b"mpd" => "application/dash+xml",
        b"mp3" => "audio/mpeg",
        b"ogg" | b"oga" | b"opus" => "audio/ogg",
        b"wav" => "audio/wav",
        b"flac" => "audio/flac",
        b"m4a" => "audio/mp4",
        b"aac" => "audio/aac",
        _ => "application/octet-stream",
    };
    HeaderValue::from_static(t)
}

/// RFC 9110 entity-tag comparison over a list header (`If-Match`, `If-None-Match`).
pub(crate) fn etag_listed(list: &[u8], etag: &[u8], weak: bool) -> bool {
    let bare = |t: &[u8]| -> (bool, Vec<u8>) {
        let t = t.trim_ascii();
        match t.strip_prefix(b"W/") {
            Some(rest) => (true, rest.to_vec()),
            None => (false, t.to_vec()),
        }
    };
    let (_, mine) = bare(etag);
    list.split(|b| *b == b',').any(|item| {
        let item = item.trim_ascii();
        if item == b"*" {
            return true;
        }
        let (is_weak, tag) = bare(item);
        tag == mine && (weak || !is_weak)
    })
}

fn http_date(v: &HeaderValue) -> Option<SystemTime> {
    httpdate::parse_http_date(v.to_str().ok()?).ok()
}

fn secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[derive(Debug, PartialEq, Eq)]
enum Plan {
    Full,
    /// Inclusive byte range.
    Partial(u64, u64),
    NotModified,
    PreconditionFailed,
    Unsatisfiable,
}

/// Conditional requests and ranges, in the order of RFC 9110 section 13.2.2.
fn plan(req: &RequestHeader, f: &FileReply) -> Plan {
    let h = &req.headers;
    let etag = f.etag.as_bytes();
    if let Some(v) = h.get(header::IF_MATCH) {
        if !etag_listed(v.as_bytes(), etag, false) {
            return Plan::PreconditionFailed;
        }
    } else if let Some(t) = h.get(header::IF_UNMODIFIED_SINCE).and_then(http_date)
        && secs(f.modified) > secs(t)
    {
        return Plan::PreconditionFailed;
    }
    if let Some(v) = h.get(header::IF_NONE_MATCH) {
        if etag_listed(v.as_bytes(), etag, true) {
            return Plan::NotModified;
        }
    } else if let Some(t) = h.get(header::IF_MODIFIED_SINCE).and_then(http_date)
        && secs(f.modified) <= secs(t)
    {
        return Plan::NotModified;
    }
    if req.method != Method::GET {
        return Plan::Full;
    }
    let Some(range) = h.get(header::RANGE) else {
        return Plan::Full;
    };
    if let Some(cond) = h.get(header::IF_RANGE) {
        let c = cond.as_bytes();
        let current = if c.starts_with(b"\"") || c.starts_with(b"W/") {
            c == etag
        } else {
            c == f.last_modified.as_bytes()
        };
        if !current {
            return Plan::Full;
        }
    }
    match parse_range(range.as_bytes(), f.len) {
        Some(Some((a, b))) => Plan::Partial(a, b),
        Some(None) => Plan::Unsatisfiable,
        None => Plan::Full,
    }
}

/// One `bytes=` range. `None`: ignore the header (malformed, several ranges);
/// `Some(None)`: not satisfiable.
pub(crate) fn parse_range(v: &[u8], len: u64) -> Option<Option<(u64, u64)>> {
    let spec = v.trim_ascii().strip_prefix(b"bytes=")?.trim_ascii();
    if spec.contains(&b',') {
        return None;
    }
    let dash = spec.iter().position(|b| *b == b'-')?;
    let num = |s: &[u8]| -> Option<u64> {
        let s = s.trim_ascii();
        if s.is_empty() || !s.iter().all(u8::is_ascii_digit) {
            return None;
        }
        std::str::from_utf8(s).ok()?.parse().ok()
    };
    let (first, last) = (&spec[..dash], &spec[dash + 1..]);
    if first.trim_ascii().is_empty() {
        let n = num(last)?;
        if n == 0 || len == 0 {
            return Some(None);
        }
        return Some(Some((len.saturating_sub(n), len - 1)));
    }
    let a = num(first)?;
    let b = if last.trim_ascii().is_empty() {
        u64::MAX
    } else {
        num(last)?
    };
    if b < a {
        return None;
    }
    if a >= len {
        return Some(None);
    }
    Some(Some((a, b.min(len - 1))))
}

/// Write the file response. `decorate` adds the site's headers; `compress` says whether
/// a full response may be compressed on the fly.
pub async fn send(
    session: &mut pingora::proxy::Session,
    f: FileReply,
    decorate: impl FnOnce(&mut ResponseHeader) -> Result<()>,
    compress: bool,
) -> Result<()> {
    let req = session.req_header();
    let head = req.method == Method::HEAD;
    let plan = plan(req, &f);
    let (status, range) = match plan {
        Plan::Full => (200, Some((0, f.len.saturating_sub(1)))),
        Plan::Partial(a, b) => (206, Some((a, b))),
        Plan::NotModified => (304, None),
        Plan::PreconditionFailed => (412, None),
        Plan::Unsatisfiable => (416, None),
    };
    let mut resp = ResponseHeader::build(status, Some(10))?;
    resp.insert_header(header::ETAG, f.etag.clone())?;
    resp.insert_header(header::LAST_MODIFIED, f.last_modified.clone())?;
    if f.vary {
        resp.insert_header(header::VARY, "Accept-Encoding")?;
    }
    let body_len = match plan {
        Plan::Full => f.len,
        Plan::Partial(a, b) => b - a + 1,
        _ => 0,
    };
    match plan {
        Plan::Full | Plan::Partial(..) => {
            resp.insert_header(header::CONTENT_TYPE, f.content_type.clone())?;
            if let Some(e) = f.encoding {
                resp.insert_header(header::CONTENT_ENCODING, e.token())?;
            }
            resp.insert_header(header::ACCEPT_RANGES, "bytes")?;
            resp.insert_header(header::CONTENT_LENGTH, body_len)?;
            if let Plan::Partial(a, b) = plan {
                resp.insert_header(header::CONTENT_RANGE, format!("bytes {a}-{b}/{}", f.len))?;
            }
        }
        Plan::Unsatisfiable => {
            resp.insert_header(header::CONTENT_RANGE, format!("bytes */{}", f.len))?;
            resp.insert_header(header::CONTENT_LENGTH, 0)?;
        }
        Plan::PreconditionFailed => resp.insert_header(header::CONTENT_LENGTH, 0)?,
        Plan::NotModified => {}
    }
    decorate(&mut resp)?;
    // Whole files only: a range is of the bytes on disk, and a precompressed file is done.
    if compress
        && plan == Plan::Full
        && f.encoding.is_none()
        && !head
        && crate::proxy::compressible(&resp)
    {
        crate::proxy::enable_compression(session);
    }
    if crate::proxy::has_body(session.req_header()) {
        session.set_keepalive(None);
    }
    let end = head || body_len == 0;
    session.write_response_header(Box::new(resp), end).await?;
    if end {
        return Ok(());
    }
    let (start, last) = range.expect("full and partial plans have a range");
    if let Some(body) = &f.body {
        let chunk = body.slice(start as usize..=last as usize);
        return session.write_response_body(Some(chunk), true).await;
    }
    let mut at = start;
    while at <= last {
        let n = ((last - at + 1) as usize).min(CHUNK);
        let file = f.file.clone();
        let chunk = tokio::task::spawn_blocking(move || read_exact_at(&file, at, n))
            .await
            .map_err(|e| Error::explain(ErrorType::InternalError, format!("read task: {e}")))?
            .map_err(|e| Error::explain(ErrorType::InternalError, format!("reading file: {e}")))?;
        at += n as u64;
        session.write_response_body(Some(chunk), at > last).await?;
    }
    Ok(())
}

fn html_escape(s: &[u8], out: &mut String) {
    for c in String::from_utf8_lossy(s).chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
}

fn url_escape(s: &[u8], out: &mut String) {
    for &b in s {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
}

fn human_size(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

/// An HTML index of a directory without an index file.
fn listing(root: &Path, dir: &[u8], shown: &[u8], hidden: impl Fn(&[u8]) -> bool) -> Found {
    let rel = dir.strip_prefix(b"/").unwrap_or(dir);
    let path = root.join(OsStr::from_bytes(rel));
    let rd = match std::fs::read_dir(&path) {
        Ok(rd) => rd,
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => return Found::Status(403),
        Err(_) => return Found::Status(404),
    };
    let mut entries: Vec<(bool, Vec<u8>, u64, u64)> = Vec::new();
    for entry in rd.flatten().take(LIST_LIMIT) {
        let name = entry.file_name().as_bytes().to_vec();
        if hidden(&name) {
            continue;
        }
        let Ok(meta) = std::fs::metadata(entry.path()) else {
            continue;
        };
        entries.push((meta.is_dir(), name, meta.len(), meta.mtime().max(0) as u64));
    }
    entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let mut title = String::new();
    html_escape(shown, &mut title);
    let mut html = format!(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><meta name=\"viewport\" \
         content=\"width=device-width\"><title>Index of {title}</title><style>body{{font-family:\
         system-ui,sans-serif;margin:2em}}td{{padding:.15em 1.5em .15em 0}}td.n{{text-align:right}}\
         </style></head><body><h1>Index of {title}</h1><table>\n"
    );
    if shown != b"/" {
        html.push_str("<tr><td><a href=\"../\">../</a></td><td></td><td></td></tr>\n");
    }
    for (is_dir, name, size, mtime) in entries {
        let mut href = String::new();
        url_escape(&name, &mut href);
        let mut label = String::new();
        html_escape(&name, &mut label);
        let slash = if is_dir { "/" } else { "" };
        let size = if is_dir {
            String::new()
        } else {
            human_size(size)
        };
        html.push_str(&format!(
            "<tr><td><a href=\"{href}{slash}\">{label}{slash}</a></td><td class=\"n\">{size}</td><td>{}</td></tr>\n",
            crate::timefmt::rfc3339(mtime)
        ));
    }
    html.push_str("</table></body></html>\n");
    Found::Listing(Bytes::from(html))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    struct Dir(PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn tree(tag: &str) -> Dir {
        let d = std::env::temp_dir().join(format!("conduit-files-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        let w = |p: &str, body: &[u8]| {
            let p = d.join(p);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        };
        w("www/index.html", b"<h1>home</h1>");
        w("www/app.js", b"console.log(1)");
        w("www/app.js.br", b"BROTLI");
        w("www/app.js.gz", b"GZIP");
        w("www/docs/guide.txt", b"guide");
        w("www/empty/x.bin", &[0u8; 300_000]);
        w("www/.env", b"SECRET=1");
        w("www/.well-known/security.txt", b"Contact: x");
        w("www/notes.bak", b"old");
        w("secret.txt", b"outside");
        symlink(d.join("secret.txt"), d.join("www/escape.txt")).unwrap();
        symlink("../secret.txt", d.join("www/rel-escape.txt")).unwrap();
        symlink("docs/guide.txt", d.join("www/inside.txt")).unwrap();
        Dir(d)
    }

    fn files(d: &Dir, extra: &str) -> Files {
        let yaml = format!("root: {:?}\n{extra}", d.0.join("www").to_string_lossy());
        let c: FilesConfig = crate::config::yaml(&yaml).unwrap();
        Files::compile(&c, true).unwrap()
    }

    fn lookup(f: &Files, raw: &str, accept: &str) -> Found {
        let mut r = RequestHeader::build("GET", raw.as_bytes(), None).unwrap();
        if !accept.is_empty() {
            r.insert_header("accept-encoding", accept).unwrap();
        }
        let (raw_path, query) = raw.split_once('?').unwrap_or((raw, ""));
        f.find(&Lookup {
            path: pathnorm::normalize(raw_path.as_bytes()).unwrap(),
            raw_path: raw_path.as_bytes().to_vec(),
            query: query.as_bytes().to_vec(),
            accepted: Accepted::from_request(&r),
        })
    }

    fn body(found: Found) -> Vec<u8> {
        match found {
            Found::File(f) => f.body.expect("small file").to_vec(),
            Found::Status(s) => format!("status {s}").into_bytes(),
            Found::Redirect(l) => [b"redirect ".as_slice(), &l].concat(),
            Found::Missing => b"missing".to_vec(),
            Found::Listing(b) => b.to_vec(),
        }
    }

    #[test]
    fn serving_and_confinement() {
        let d = tree("serve");
        let f = files(&d, "");
        assert_eq!(body(lookup(&f, "/", "")), b"<h1>home</h1>");
        assert_eq!(body(lookup(&f, "/docs/guide.txt", "")), b"guide");
        assert_eq!(body(lookup(&f, "/docs/../app.js", "")), b"console.log(1)");
        // Directories get their slash, files lose it; the query survives.
        assert_eq!(body(lookup(&f, "/docs?x=1", "")), b"redirect /docs/?x=1");
        // Never a protocol-relative Location (another host).
        assert_eq!(body(lookup(&f, "//docs", "")), b"redirect /docs/");
        assert_eq!(body(lookup(&f, "///app.js/", "")), b"redirect /app.js");
        assert_eq!(body(lookup(&f, "/app.js/", "")), b"redirect /app.js");
        // No index and no browse.
        assert_eq!(body(lookup(&f, "/docs/", "")), b"missing");
        // Dotfiles and hidden names are missing; .well-known is not hidden.
        assert_eq!(body(lookup(&f, "/.env", "")), b"missing");
        assert_eq!(body(lookup(&f, "/%2eenv", "")), b"missing");
        assert_eq!(
            body(lookup(&f, "/.well-known/security.txt", "")),
            b"Contact: x"
        );
        // Symlinks leading out of the root fail in the kernel; inside ones work.
        assert_eq!(body(lookup(&f, "/escape.txt", "")), b"missing");
        assert_eq!(body(lookup(&f, "/rel-escape.txt", "")), b"missing");
        assert_eq!(body(lookup(&f, "/inside.txt", "")), b"guide");
        assert_eq!(body(lookup(&f, "/nope", "")), b"missing");
        let f = files(&d, "hide: [\"*.bak\", docs]\n");
        assert_eq!(body(lookup(&f, "/notes.bak", "")), b"missing");
        assert_eq!(body(lookup(&f, "/docs/guide.txt", "")), b"missing");
    }

    #[test]
    fn precompressed_try_files_browse_prefix() {
        let d = tree("variants");
        let f = files(&d, "precompressed: [br, gzip]\n");
        let Found::File(r) = lookup(&f, "/app.js", "gzip, br") else {
            panic!()
        };
        assert_eq!(
            (r.encoding, &r.body.as_ref().unwrap()[..], r.vary),
            (Some(Encoding::Br), &b"BROTLI"[..], true)
        );
        assert_eq!(r.content_type, "text/javascript; charset=utf-8");
        assert!(r.etag.to_str().unwrap().ends_with("-br\""));
        assert_eq!(body(lookup(&f, "/app.js", "gzip;q=1, br;q=0")), b"GZIP");
        assert_eq!(body(lookup(&f, "/app.js", "identity")), b"console.log(1)");
        assert_eq!(body(lookup(&f, "/app.js", "*")), b"BROTLI");

        let spa = files(&d, "try_files: [\"{path}\", \"{path}/\", /index.html]\n");
        assert_eq!(body(lookup(&spa, "/some/route", "")), b"<h1>home</h1>");
        assert_eq!(body(lookup(&spa, "/app.js", "")), b"console.log(1)");
        // A directory without its slash: through `{path}/`, no redirect.
        assert_eq!(body(lookup(&spa, "/docs", "")), b"<h1>home</h1>");

        let b = files(&d, "browse: true\n");
        let html = String::from_utf8(body(lookup(&b, "/empty/", ""))).unwrap();
        assert!(html.contains("<a href=\"x.bin\">x.bin</a>"), "{html}");
        assert!(
            html.contains("292.97 KiB") || html.contains("293.0 KiB"),
            "{html}"
        );
        let root = String::from_utf8(body(lookup(&b, "/docs/", ""))).unwrap();
        assert!(root.contains("guide.txt") && !root.contains(".env"));

        let p = files(&d, "strip_prefix: /static/\n");
        assert_eq!(body(lookup(&p, "/static/app.js", "")), b"console.log(1)");
        assert_eq!(body(lookup(&p, "/staticx/app.js", "")), b"missing");
        assert_eq!(body(lookup(&p, "/app.js", "")), b"missing");
        // Large files are streamed, not read up front.
        let Found::File(big) = lookup(&f, "/empty/x.bin", "") else {
            panic!()
        };
        assert!(big.body.is_none() && big.len == 300_000);
    }

    #[test]
    fn settings_are_checked() {
        let d = tree("settings");
        let bad = |extra: &str, upstream: bool| {
            let yaml = format!("root: {:?}\n{extra}", d.0.join("www").to_string_lossy());
            let c: FilesConfig = crate::config::yaml(&yaml).unwrap();
            Files::compile(&c, upstream).err().unwrap()
        };
        assert!(bad("precompressed: [deflate]\n", true).contains("deflate"));
        assert!(bad("try_files: [index.html]\n", true).contains("start with"));
        assert!(bad("hide: [a/b]\n", true).contains("name"));
        assert!(bad("pass_thru: true\n", false).contains("no upstream"));
        assert!(bad("index: [../x]\n", true).contains("plain file name"));
        let c: FilesConfig = crate::config::yaml("root: /nonexistent-conduit\n").unwrap();
        assert!(
            Files::compile(&c, true)
                .err()
                .unwrap()
                .contains("not a directory")
        );
    }

    fn reply_for(len: u64) -> FileReply {
        let modified = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        FileReply {
            file: Arc::new(File::open("/dev/null").unwrap()),
            len,
            modified,
            content_type: HeaderValue::from_static("text/plain"),
            encoding: None,
            etag: HeaderValue::from_static("\"abc-10\""),
            last_modified: HeaderValue::from_str(&httpdate::fmt_http_date(modified)).unwrap(),
            vary: false,
            body: None,
        }
    }

    fn plan_with(method: &str, headers: &[(&str, &str)]) -> Plan {
        let mut r = RequestHeader::build(method, b"/", None).unwrap();
        for (k, v) in headers {
            r.insert_header(k.to_string(), *v).unwrap();
        }
        plan(&r, &reply_for(1000))
    }

    #[test]
    fn conditionals_and_ranges() {
        use Plan::*;
        let lm = "Tue, 14 Nov 2023 22:13:20 GMT";
        assert_eq!(plan_with("GET", &[]), Full);
        assert_eq!(
            plan_with("GET", &[("if-none-match", "\"x\", W/\"abc-10\"")]),
            NotModified
        );
        assert_eq!(plan_with("HEAD", &[("if-none-match", "*")]), NotModified);
        assert_eq!(plan_with("GET", &[("if-modified-since", lm)]), NotModified);
        assert_eq!(
            plan_with(
                "GET",
                &[("if-modified-since", "Tue, 14 Nov 2023 22:13:19 GMT")]
            ),
            Full
        );
        // If-None-Match wins over If-Modified-Since.
        assert_eq!(
            plan_with(
                "GET",
                &[("if-none-match", "\"old\""), ("if-modified-since", lm)]
            ),
            Full
        );
        assert_eq!(
            plan_with("GET", &[("if-match", "W/\"abc-10\"")]),
            PreconditionFailed
        );
        assert_eq!(plan_with("GET", &[("if-match", "\"abc-10\"")]), Full);
        assert_eq!(
            plan_with(
                "GET",
                &[("if-unmodified-since", "Mon, 13 Nov 2023 00:00:00 GMT")]
            ),
            PreconditionFailed
        );
        assert_eq!(plan_with("GET", &[("range", "bytes=0-99")]), Partial(0, 99));
        assert_eq!(
            plan_with("GET", &[("range", "bytes=900-")]),
            Partial(900, 999)
        );
        assert_eq!(
            plan_with("GET", &[("range", "bytes=-100")]),
            Partial(900, 999)
        );
        assert_eq!(
            plan_with("GET", &[("range", "bytes=990-5000")]),
            Partial(990, 999)
        );
        assert_eq!(plan_with("GET", &[("range", "bytes=1000-")]), Unsatisfiable);
        assert_eq!(plan_with("GET", &[("range", "bytes=0-1,5-6")]), Full);
        assert_eq!(plan_with("GET", &[("range", "items=0-1")]), Full);
        assert_eq!(plan_with("HEAD", &[("range", "bytes=0-1")]), Full);
        assert_eq!(
            plan_with("GET", &[("range", "bytes=0-1"), ("if-range", "\"abc-10\"")]),
            Partial(0, 1)
        );
        assert_eq!(
            plan_with("GET", &[("range", "bytes=0-1"), ("if-range", "\"old\"")]),
            Full
        );
        assert_eq!(
            plan_with("GET", &[("range", "bytes=0-1"), ("if-range", lm)]),
            Partial(0, 1)
        );
    }

    #[test]
    fn content_types() {
        assert_eq!(content_type(b"/a/B.HTML"), "text/html; charset=utf-8");
        assert_eq!(content_type(b"/x.woff2"), "font/woff2");
        assert_eq!(content_type(b"/Makefile"), "application/octet-stream");
        assert_eq!(content_type(b"/.hidden"), "application/octet-stream");
    }
}
