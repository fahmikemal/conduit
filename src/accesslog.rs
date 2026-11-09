//! Access log: one JSON object per line, written by one background thread.
//!
//! Request tasks never touch the disk. Each worker thread formats its lines into a buffer of
//! its own, and the writer thread swaps those buffers out every `POLL` and appends them to the
//! file. On the request path that is one lock nobody else takes (save the writer, fifty times
//! a second) and, once the buffers have grown, no allocation. A thread whose buffer is full
//! drops the line and counts it rather than slowing traffic. The file reopens by itself after
//! logrotate moves it.
//!
//! Within one `POLL` window lines land grouped by worker thread rather than strictly in
//! completion order, as with nginx's buffered access log; `ts` orders them.

use std::borrow::Cow;
use std::cell::RefCell;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::net::IpAddr;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use log::{error, warn};
use serde::{Serialize, Serializer};

use crate::metrics::ACCESS_LOG_DROPPED;
use crate::timefmt;

/// One request. Field names are stable: log pipelines and CrowdSec parsers depend on them.
///
/// The `Serialize` derive is the format's definition; [`Entry::write_json`] writes the same
/// bytes faster, and the tests hold the two equal.
#[derive(Serialize)]
pub struct Entry<'a> {
    #[serde(serialize_with = "ser_ts")]
    pub ts: SystemTime,
    pub request_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<&'a str>,
    pub host: &'a str,
    pub client: Option<IpAddr>,
    pub method: &'a str,
    pub scheme: &'static str,
    pub uri: Cow<'a, str>,
    pub proto: &'static str,
    pub status: u16,
    pub bytes_sent: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub bytes_received: u64,
    #[serde(serialize_with = "ser_millis")]
    pub duration_ms: Duration,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_status: Option<u16>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "ser_opt_millis"
    )]
    pub upstream_connect_ms: Option<Duration>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "ser_opt_millis"
    )]
    pub upstream_header_ms: Option<Duration>,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub retries: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls: Option<&'a str>,
    #[serde(skip_serializing_if = "str::is_empty")]
    pub user_agent: Cow<'a, str>,
    #[serde(skip_serializing_if = "str::is_empty")]
    pub referer: Cow<'a, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<&'a str>,
    /// `crowdsec` (a decision on the client) or `appsec` (an AppSec verdict).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_by: Option<&'static str>,
    /// For sites with `cache:`: hit, stale, revalidated, miss, uncacheable or bypass.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache: Option<&'static str>,
}

fn is_zero(v: &u64) -> bool {
    *v == 0
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

fn ser_ts<S: Serializer>(t: &SystemTime, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&timefmt::rfc3339_millis(*t))
}

/// Milliseconds with microsecond precision, as a JSON number.
fn ser_millis<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_f64(d.as_micros() as f64 / 1000.0)
}

fn ser_opt_millis<S: Serializer>(d: &Option<Duration>, s: S) -> Result<S::Ok, S::Error> {
    match d {
        Some(d) => ser_millis(d, s),
        None => s.serialize_none(),
    }
}

impl Entry<'_> {
    /// Append the line `serde_json` makes of this entry, without the newline. Keys are
    /// literals here, strings are scanned for escapes eight bytes at a time and durations are
    /// written from integer microseconds, which is what makes this cheaper than the derive.
    pub fn write_json(&self, out: &mut Vec<u8>) {
        out.reserve(512);
        out.extend_from_slice(b"{\"ts\":\"");
        timefmt::write_rfc3339_millis(out, self.ts);
        out.extend_from_slice(b"\",\"request_id\":");
        json::str(out, self.request_id);
        if let Some(site) = self.site {
            out.extend_from_slice(b",\"site\":");
            json::str(out, site);
        }
        out.extend_from_slice(b",\"host\":");
        json::str(out, self.host);
        out.extend_from_slice(b",\"client\":");
        match self.client {
            Some(ip) => json::ip(out, ip),
            None => out.extend_from_slice(b"null"),
        }
        out.extend_from_slice(b",\"method\":");
        json::str(out, self.method);
        out.extend_from_slice(b",\"scheme\":");
        json::str(out, self.scheme);
        out.extend_from_slice(b",\"uri\":");
        json::str(out, &self.uri);
        out.extend_from_slice(b",\"proto\":");
        json::str(out, self.proto);
        out.extend_from_slice(b",\"status\":");
        json::int(out, self.status);
        out.extend_from_slice(b",\"bytes_sent\":");
        json::int(out, self.bytes_sent);
        if self.bytes_received != 0 {
            out.extend_from_slice(b",\"bytes_received\":");
            json::int(out, self.bytes_received);
        }
        out.extend_from_slice(b",\"duration_ms\":");
        json::millis(out, self.duration_ms);
        if let Some(upstream) = self.upstream {
            out.extend_from_slice(b",\"upstream\":");
            json::str(out, upstream);
        }
        if let Some(status) = self.upstream_status {
            out.extend_from_slice(b",\"upstream_status\":");
            json::int(out, status);
        }
        if let Some(d) = self.upstream_connect_ms {
            out.extend_from_slice(b",\"upstream_connect_ms\":");
            json::millis(out, d);
        }
        if let Some(d) = self.upstream_header_ms {
            out.extend_from_slice(b",\"upstream_header_ms\":");
            json::millis(out, d);
        }
        if self.retries != 0 {
            out.extend_from_slice(b",\"retries\":");
            json::int(out, self.retries);
        }
        if let Some(tls) = self.tls {
            out.extend_from_slice(b",\"tls\":");
            json::str(out, tls);
        }
        if !self.user_agent.is_empty() {
            out.extend_from_slice(b",\"user_agent\":");
            json::str(out, &self.user_agent);
        }
        if !self.referer.is_empty() {
            out.extend_from_slice(b",\"referer\":");
            json::str(out, &self.referer);
        }
        if let Some(error) = self.error {
            out.extend_from_slice(b",\"error\":");
            json::str(out, error);
        }
        if let Some(blocked_by) = self.blocked_by {
            out.extend_from_slice(b",\"blocked_by\":");
            json::str(out, blocked_by);
        }
        if let Some(cache) = self.cache {
            out.extend_from_slice(b",\"cache\":");
            json::str(out, cache);
        }
        out.push(b'}');
    }
}

/// The JSON pieces of an access log line, byte for byte as `serde_json` writes them.
mod json {
    use std::io::Write as _;
    use std::net::IpAddr;
    use std::time::Duration;

    /// serde_json's escape table: the short escape for a byte, `u` for `\u00XX`, 0 for none.
    const ESCAPE: [u8; 256] = {
        let mut t = [0u8; 256];
        let mut i = 0;
        while i < 0x20 {
            t[i] = b'u';
            i += 1;
        }
        t[0x08] = b'b';
        t[0x09] = b't';
        t[0x0a] = b'n';
        t[0x0c] = b'f';
        t[0x0d] = b'r';
        t[b'"' as usize] = b'"';
        t[b'\\' as usize] = b'\\';
        t
    };

    const HEX: &[u8; 16] = b"0123456789abcdef";
    const ONES: u64 = 0x0101_0101_0101_0101;
    const HIGHS: u64 = 0x8080_8080_8080_8080;

    /// Whether any of the eight bytes in `w` is a control byte, `"` or `\`. Exact as a yes or
    /// no; bytes of 0x80 and up (UTF-8 sequences) never count.
    #[inline]
    fn needs_escape(w: u64) -> bool {
        let zero_byte = |v: u64| v.wrapping_sub(ONES) & !v & HIGHS;
        let control = w.wrapping_sub(ONES * 0x20) & !w & HIGHS;
        control | zero_byte(w ^ (ONES * u64::from(b'"'))) | zero_byte(w ^ (ONES * u64::from(b'\\')))
            != 0
    }

    pub fn str(out: &mut Vec<u8>, s: &str) {
        let b = s.as_bytes();
        out.push(b'"');
        let mut i = 0;
        while let Some(chunk) = b.get(i..i + 8) {
            let w = u64::from_le_bytes(chunk.try_into().expect("eight bytes"));
            if needs_escape(w) {
                break;
            }
            i += 8;
        }
        let mut start = 0;
        for (j, &c) in b.iter().enumerate().skip(i) {
            let e = ESCAPE[usize::from(c)];
            if e == 0 {
                continue;
            }
            out.extend_from_slice(&b[start..j]);
            if e == b'u' {
                let hex = [HEX[usize::from(c >> 4)], HEX[usize::from(c & 0xf)]];
                out.extend_from_slice(&[b'\\', b'u', b'0', b'0', hex[0], hex[1]]);
            } else {
                out.extend_from_slice(&[b'\\', e]);
            }
            start = j + 1;
        }
        out.extend_from_slice(&b[start..]);
        out.push(b'"');
    }

    pub fn int(out: &mut Vec<u8>, n: impl itoa::Integer) {
        out.extend_from_slice(itoa::Buffer::new().format(n).as_bytes());
    }

    /// serde_json writes an IP address as its `Display` form in a string.
    pub fn ip(out: &mut Vec<u8>, ip: IpAddr) {
        out.push(b'"');
        match ip {
            IpAddr::V4(v4) => {
                let [a, b, c, d] = v4.octets();
                int(out, a);
                for o in [b, c, d] {
                    out.push(b'.');
                    int(out, o);
                }
            }
            IpAddr::V6(v6) => {
                let _ = write!(out, "{v6}");
            }
        }
        out.push(b'"');
    }

    /// `d` in milliseconds, as serde_json writes the f64 `micros / 1000.0`: the shortest
    /// decimal that reads back as that f64, with `.0` on whole numbers. Below 10^15 µs (31
    /// years) the value has at most 15 significant digits, so that decimal is exactly
    /// `micros / 1000` with trailing zeros dropped.
    pub fn millis(out: &mut Vec<u8>, d: Duration) {
        let micros = d.as_micros();
        if micros >= 1_000_000_000_000_000 {
            let _ = serde_json::to_writer(&mut *out, &(micros as f64 / 1000.0));
            return;
        }
        let micros = micros as u64;
        int(out, micros / 1000);
        let frac = (micros % 1000) as u32;
        let digits = [
            b'.',
            b'0' + (frac / 100) as u8,
            b'0' + (frac / 10 % 10) as u8,
            b'0' + (frac % 10) as u8,
        ];
        // ".0" for whole milliseconds, otherwise without trailing zeros.
        let len = if frac % 100 == 0 {
            2
        } else if frac % 10 == 0 {
            3
        } else {
            4
        };
        out.extend_from_slice(&digits[..len]);
    }
}

/// One worker thread's lines waiting for the writer.
#[derive(Default)]
struct Shard {
    buf: Vec<u8>,
    lines: usize,
}

struct Shards {
    /// Tells this log's shard apart in a thread's [`MINE`] (tests run several logs at once).
    id: u64,
    all: Mutex<Vec<Arc<Mutex<Shard>>>>,
    max_lines: usize,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

thread_local! {
    /// This thread's shard of each log it has written to.
    static MINE: RefCell<Vec<(u64, Arc<Mutex<Shard>>)>> = const { RefCell::new(Vec::new()) };
}

pub struct AccessLog {
    shards: Option<Arc<Shards>>,
    /// Flush requests; the writer acknowledges each on the sender it carries.
    flush_tx: Option<SyncSender<SyncSender<()>>>,
    flush_rx: Mutex<Option<Receiver<SyncSender<()>>>>,
    path: Option<PathBuf>,
}

impl AccessLog {
    /// `capacity` lines may wait in memory, split evenly across `threads` worker threads
    /// (at least 1024 each).
    pub fn new(path: Option<PathBuf>, capacity: usize, threads: usize) -> AccessLog {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let Some(path) = path else {
            return AccessLog {
                shards: None,
                flush_tx: None,
                flush_rx: Mutex::new(None),
                path: None,
            };
        };
        let (tx, rx) = sync_channel(8);
        AccessLog {
            shards: Some(Arc::new(Shards {
                id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
                all: Mutex::new(Vec::new()),
                max_lines: (capacity / threads.max(1)).max(1024),
            })),
            flush_tx: Some(tx),
            flush_rx: Mutex::new(Some(rx)),
            path: Some(path),
        }
    }

    pub fn enabled(&self) -> bool {
        self.shards.is_some()
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn write(&self, entry: &Entry<'_>) {
        let Some(shards) = &self.shards else { return };
        MINE.with_borrow_mut(|mine| {
            let i = match mine.iter().position(|(id, _)| *id == shards.id) {
                Some(i) => i,
                None => {
                    let shard = Arc::new(Mutex::new(Shard::default()));
                    lock(&shards.all).push(shard.clone());
                    mine.push((shards.id, shard));
                    mine.len() - 1
                }
            };
            let mut shard = lock(&mine[i].1);
            if shard.lines >= shards.max_lines {
                ACCESS_LOG_DROPPED.inc();
                return;
            }
            entry.write_json(&mut shard.buf);
            shard.buf.push(b'\n');
            shard.lines += 1;
        });
    }

    /// Start the writer thread. Called after daemonizing: threads do not survive fork().
    pub fn start(&self) {
        let (Some(path), Some(shards), Some(rx)) = (
            self.path.clone(),
            self.shards.clone(),
            lock(&self.flush_rx).take(),
        ) else {
            return;
        };
        if let Some(dir) = path.parent()
            && let Err(e) = std::fs::create_dir_all(dir)
        {
            error!("access log dir {}: {e}", dir.display());
        }
        std::thread::Builder::new()
            .name("conduit-accesslog".into())
            .spawn(move || Writer::new(path).run(&shards, rx))
            .expect("spawn access log thread");
    }

    /// Write out every line logged so far; waits at most `timeout`.
    pub fn flush(&self, timeout: Duration) {
        let Some(tx) = &self.flush_tx else { return };
        let (ack_tx, ack_rx) = sync_channel(1);
        if tx.try_send(ack_tx).is_ok() {
            let _ = ack_rx.recv_timeout(timeout);
        }
    }
}

struct OpenFile {
    out: BufWriter<File>,
    ino: u64,
    dirty: bool,
}

struct Writer {
    path: PathBuf,
    file: Option<OpenFile>,
    /// Whether the last attempt to open the file failed and was reported.
    open_failed: bool,
    reported_drops: u64,
    last_check: Instant,
    /// The buffer handed to the next shard drained; its capacity circulates between shards.
    spare: Vec<u8>,
    taken: Vec<Arc<Mutex<Shard>>>,
}

const POLL: Duration = Duration::from_millis(20);
const FLUSH_EVERY: Duration = Duration::from_secs(1);
const REOPEN_CHECK_EVERY: Duration = Duration::from_secs(5);

impl Writer {
    fn new(path: PathBuf) -> Writer {
        Writer {
            path,
            file: None,
            open_failed: false,
            reported_drops: ACCESS_LOG_DROPPED.get(),
            last_check: Instant::now(),
            spare: Vec::new(),
            taken: Vec::new(),
        }
    }

    fn open(path: &Path) -> std::io::Result<OpenFile> {
        let f = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o640)
            .open(path)?;
        let ino = f.metadata()?.ino();
        Ok(OpenFile {
            out: BufWriter::with_capacity(64 * 1024, f),
            ino,
            dirty: false,
        })
    }

    /// Wakes every `POLL` rather than when lines arrive: waking a parked writer per line costs
    /// the request path a futex syscall, and a context switch, each time.
    fn run(mut self, shards: &Shards, rx: Receiver<SyncSender<()>>) {
        let mut last_flush = Instant::now();
        loop {
            self.drain(shards);
            loop {
                match rx.try_recv() {
                    Ok(ack) => {
                        self.drain(shards);
                        self.flush();
                        let _ = ack.try_send(());
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.drain(shards);
                        self.flush();
                        return;
                    }
                }
            }
            if last_flush.elapsed() >= FLUSH_EVERY {
                self.flush();
                last_flush = Instant::now();
            }
            if self.last_check.elapsed() >= REOPEN_CHECK_EVERY {
                self.flush();
                self.reopen_rotated();
                self.report_drops();
                self.last_check = Instant::now();
            }
            std::thread::sleep(POLL);
        }
    }

    /// Take every shard's lines, holding each shard's lock only for a swap.
    fn drain(&mut self, shards: &Shards) {
        self.taken.extend(lock(&shards.all).iter().cloned());
        for shard in std::mem::take(&mut self.taken) {
            {
                let mut s = lock(&shard);
                if s.lines == 0 {
                    continue;
                }
                std::mem::swap(&mut s.buf, &mut self.spare);
                s.lines = 0;
            }
            let lines = std::mem::take(&mut self.spare);
            self.append(&lines);
            self.spare = lines;
            self.spare.clear();
        }
        // A shard only the list still holds belongs to a thread that has exited; nothing can
        // add to it, so once empty it can go.
        lock(&shards.all).retain(|s| Arc::strong_count(s) > 1 || lock(s).lines > 0);
    }

    fn append(&mut self, lines: &[u8]) {
        if self.file.is_none() {
            match Self::open(&self.path) {
                Ok(f) => {
                    self.file = Some(f);
                    self.open_failed = false;
                }
                Err(e) => {
                    if !self.open_failed {
                        error!("cannot open access log {}: {e}", self.path.display());
                        self.open_failed = true;
                    }
                    return;
                }
            }
        }
        let f = self.file.as_mut().expect("just opened");
        if let Err(e) = f.out.write_all(lines) {
            error!("access log {}: {e}", self.path.display());
        }
        f.dirty = true;
    }

    fn flush(&mut self) {
        if let Some(f) = self.file.as_mut().filter(|f| f.dirty) {
            if let Err(e) = f.out.flush() {
                error!("access log {}: {e}", self.path.display());
            }
            f.dirty = false;
        }
    }

    /// logrotate (create mode) renames the file; follow the new one at the original path.
    fn reopen_rotated(&mut self) {
        let same = self
            .file
            .as_ref()
            .is_some_and(|f| std::fs::metadata(&self.path).is_ok_and(|m| m.ino() == f.ino));
        if !same {
            self.file = None;
        }
    }

    fn report_drops(&mut self) {
        let total = ACCESS_LOG_DROPPED.get();
        if total > self.reported_drops {
            warn!(
                "access log buffer full: dropped {} lines ({total} since start)",
                total - self.reported_drops
            );
            self.reported_drops = total;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    fn entry<'a>(ua: &'a str) -> Entry<'a> {
        Entry {
            // 2026-11-07T06:45:30.123Z
            ts: UNIX_EPOCH + Duration::from_millis(1_794_033_930_123),
            request_id: "0123456789abcdef01234567",
            site: Some("api"),
            host: "api.example.com",
            client: Some("198.51.100.14".parse().unwrap()),
            method: "GET",
            scheme: "https",
            uri: Cow::Borrowed("/api/records?id=1"),
            proto: "HTTP/2",
            status: 200,
            bytes_sent: 228,
            bytes_received: 0,
            duration_ms: Duration::from_micros(1534),
            upstream: Some("10.0.0.2:8090"),
            upstream_status: Some(200),
            upstream_connect_ms: Some(Duration::ZERO),
            upstream_header_ms: Some(Duration::from_micros(1200)),
            retries: 0,
            tls: Some("TLSv1.3"),
            user_agent: Cow::Borrowed(ua),
            referer: Cow::Borrowed(""),
            error: None,
            blocked_by: None,
            cache: None,
        }
    }

    fn line(e: &Entry<'_>) -> String {
        let mut out = Vec::new();
        e.write_json(&mut out);
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn json_line() {
        let e = entry("Dart/3.9 (dart:io)");
        let want = r#"{"ts":"2026-11-07T06:45:30.123Z","request_id":"0123456789abcdef01234567","site":"api","host":"api.example.com","client":"198.51.100.14","method":"GET","scheme":"https","uri":"/api/records?id=1","proto":"HTTP/2","status":200,"bytes_sent":228,"duration_ms":1.534,"upstream":"10.0.0.2:8090","upstream_status":200,"upstream_connect_ms":0.0,"upstream_header_ms":1.2,"tls":"TLSv1.3","user_agent":"Dart/3.9 (dart:io)"}"#;
        assert_eq!(serde_json::to_string(&e).unwrap(), want);
        assert_eq!(line(&e), want);
    }

    #[test]
    fn hostile_values_stay_one_line() {
        let ua = String::from_utf8_lossy(b"evil\" agent\\\n\x01\xff").into_owned();
        let line = line(&entry(&ua));
        assert!(!line.contains('\n'));
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["user_agent"], ua.as_str());
    }

    /// xorshift64*: deterministic, so a failure reproduces.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }

        /// A string of up to 40 bytes mixing plain ASCII, every byte that needs escaping,
        /// DEL and multibyte characters, so escapes fall on every offset of the 8-byte scan.
        fn text(&mut self) -> String {
            const PIECES: &[&str] = &[
                "a", "Z", "0", "/", " ", "~", "\u{7f}", "\"", "\\", "\n", "\r", "\t", "\u{8}",
                "\u{c}", "\u{0}", "\u{1}", "\u{1f}", "é", "€", "😀", "\u{fffd}", "abcdefgh",
            ];
            let n = self.below(41) as usize;
            let mut s = String::new();
            while s.len() < n {
                // Mostly plain text, like real values.
                let i = if self.below(4) == 0 {
                    self.below(PIECES.len() as u64)
                } else {
                    self.below(6)
                };
                s.push_str(PIECES[i as usize]);
            }
            s
        }

        fn duration(&mut self) -> Duration {
            let micros = match self.below(6) {
                0 => self.below(10),
                1 => self.below(1000),
                2 => self.below(10_000_000),
                3 => self.below(1_000_000_000_000_000),
                4 => 1_000_000_000_000_000 + self.below(1 << 60),
                _ => 10u64.pow(self.below(16) as u32) * (1 + self.below(9)),
            };
            Duration::from_micros(micros)
        }

        fn ip(&mut self) -> Option<IpAddr> {
            match self.below(4) {
                0 => None,
                1 => Some(IpAddr::from((self.next() as u32).to_be_bytes())),
                // ::1-like and IPv4-mapped addresses, which Display writes specially.
                2 => Some(IpAddr::V6(if self.below(2) == 0 {
                    std::net::Ipv6Addr::from(u128::from(self.next() as u16))
                } else {
                    std::net::Ipv4Addr::from(self.next() as u32).to_ipv6_mapped()
                })),
                _ => Some(IpAddr::from(
                    (u128::from(self.next()) << 64 | u128::from(self.next())).to_be_bytes(),
                )),
            }
        }
    }

    #[test]
    fn write_json_matches_serde_json() {
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        let mut out = Vec::new();
        for _ in 0..20_000 {
            let t: Vec<String> = (0..12).map(|_| rng.text()).collect();
            let some = |rng: &mut Rng, s: &str| -> Option<String> {
                (rng.below(3) != 0).then(|| s.to_owned())
            };
            let site = some(&mut rng, &t[0]);
            let upstream = some(&mut rng, &t[1]);
            let tls = some(&mut rng, &t[2]);
            let error = some(&mut rng, &t[3]);
            let e = Entry {
                ts: UNIX_EPOCH + Duration::from_micros(rng.below(4_000_000_000_000_000)),
                request_id: &t[4],
                site: site.as_deref(),
                host: &t[5],
                client: rng.ip(),
                method: &t[6],
                scheme: ["http", "https"][rng.below(2) as usize],
                uri: Cow::Borrowed(&t[7]),
                proto: ["HTTP/1.0", "HTTP/1.1", "HTTP/2"][rng.below(3) as usize],
                status: rng.next() as u16,
                bytes_sent: rng.next() as usize >> rng.below(64),
                bytes_received: rng.next() >> rng.below(64),
                duration_ms: rng.duration(),
                upstream: upstream.as_deref(),
                upstream_status: (rng.below(2) == 0).then(|| rng.next() as u16),
                upstream_connect_ms: (rng.below(2) == 0).then(|| rng.duration()),
                upstream_header_ms: (rng.below(2) == 0).then(|| rng.duration()),
                retries: (rng.next() as u32) >> rng.below(32),
                tls: tls.as_deref(),
                user_agent: Cow::Borrowed(&t[8]),
                referer: Cow::Borrowed(&t[9]),
                error: error.as_deref(),
                blocked_by: [None, Some("crowdsec"), Some("appsec")][rng.below(3) as usize],
                cache: [None, Some("hit"), Some("miss")][rng.below(3) as usize],
            };
            out.clear();
            e.write_json(&mut out);
            let want = serde_json::to_string(&e).unwrap();
            assert_eq!(std::str::from_utf8(&out).unwrap(), want);
        }
    }

    #[test]
    fn full_shard_drops_and_counts() {
        let log = AccessLog::new(Some("/nonexistent/access.log".into()), 0, 1);
        let before = ACCESS_LOG_DROPPED.get();
        for _ in 0..1100 {
            log.write(&entry("x"));
        }
        // Not started, so nothing drains: 1024 lines fit, the rest are counted.
        assert!(ACCESS_LOG_DROPPED.get() - before >= 76);
        let shards = log.shards.as_ref().unwrap();
        assert_eq!(lock(&lock(&shards.all)[0]).lines, 1024);
    }

    #[test]
    fn writer_appends_and_follows_rotation() {
        let dir = std::env::temp_dir().join(format!("conduit-alog-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("access.log");
        let log = AccessLog::new(Some(path.clone()), 1024, 1);
        log.start();
        log.write(&entry("one"));
        // A line from another thread, which exits before the flush.
        std::thread::scope(|s| {
            s.spawn(|| log.write(&entry("other")));
        });
        log.flush(Duration::from_secs(2));
        std::fs::rename(&path, dir.join("access.log.1")).unwrap();
        // Rotation is noticed within REOPEN_CHECK_EVERY.
        std::thread::sleep(REOPEN_CHECK_EVERY + Duration::from_millis(1200));
        log.write(&entry("two"));
        log.flush(Duration::from_secs(2));
        let old = std::fs::read_to_string(dir.join("access.log.1")).unwrap();
        let new = std::fs::read_to_string(&path).unwrap();
        assert!(old.contains(r#""user_agent":"one""#) && old.contains(r#""user_agent":"other""#));
        assert_eq!(old.lines().count(), 2);
        assert!(new.contains(r#""user_agent":"two""#) && new.lines().count() == 1);
        // The exited thread's shard was dropped once drained.
        assert_eq!(lock(&log.shards.as_ref().unwrap().all).len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
