//! CrowdSec remediation: a bouncer for the local API's decisions and a client of the AppSec
//! component (WAF).
//!
//! Decisions are pulled in stream mode, the full list at start and then what changed every
//! `update_interval`, into an in-memory table, so checking a client costs a few hash lookups
//! and no network call. When the local API is unreachable, the decisions already held keep
//! applying until they expire. After a failed pull the next one fetches the full list again:
//! the local API hands out each change once, so a lost delta cannot be asked for again.
//!
//! AppSec runs on sites with `crowdsec: { appsec: true }`, after the decision check: conduit
//! forwards the request's headers, and a body of known length up to `body_limit`, and waits
//! at most `timeout` for the verdict.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, LazyLock, Mutex, PoisonError, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use bytes::Bytes;
use http::{HeaderName, HeaderValue, Method, Uri, header};
use log::{info, warn};
use pingora::http::RequestHeader;
use pingora::server::ShutdownWatch;
use pingora::services::background::BackgroundService;
use serde::Deserialize;
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};

use crate::config::{CrowdSecSection, FailureAction};
use crate::httpc;
use crate::metrics;
use crate::plugin::{Local, Plugin};
use crate::proxy::Shared;
use crate::rules::RequestView;

// ---------------------------------------------------------------------------------------
// Decisions

/// Seconds on a monotonic clock: decision expiry needs no wall time.
static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);

fn now() -> u32 {
    u32::try_from(EPOCH.elapsed().as_secs()).unwrap_or(u32::MAX)
}

/// What a decision asks for. Captcha is not served: such clients are refused like banned
/// ones, which is CrowdSec's fallback for remediations a bouncer does not support.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Remedy {
    Captcha,
    Ban,
}

impl Remedy {
    fn parse(kind: &str) -> Remedy {
        if kind.eq_ignore_ascii_case("captcha") {
            Remedy::Captcha
        } else {
            Remedy::Ban
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Decision {
    id: i64,
    /// Expiry on the `now()` clock.
    until: u32,
    remedy: Remedy,
}

/// The decisions on one address or range: nearly always exactly one.
// Boxed, the rare list fits beside a decision's niche and a slot stays 16 bytes, which
// counts at 100k decisions.
#[allow(clippy::box_collection)]
enum Slot {
    One(Decision),
    Many(Box<Vec<Decision>>),
}

impl Slot {
    fn all(&self) -> &[Decision] {
        match self {
            Slot::One(d) => std::slice::from_ref(d),
            Slot::Many(v) => v,
        }
    }

    /// Add or replace (same id); true when added.
    fn add(&mut self, d: Decision) -> bool {
        match self {
            Slot::One(o) if o.id == d.id => {
                *o = d;
                false
            }
            Slot::One(o) => {
                *self = Slot::Many(Box::new(vec![*o, d]));
                true
            }
            Slot::Many(v) => match v.iter_mut().find(|x| x.id == d.id) {
                Some(x) => {
                    *x = d;
                    false
                }
                None => {
                    v.push(d);
                    true
                }
            },
        }
    }

    /// Keep the decisions `keep` accepts; returns how many went and whether any are left.
    fn retain(&mut self, keep: impl Fn(&Decision) -> bool) -> (usize, bool) {
        match self {
            Slot::One(d) => {
                if keep(d) {
                    (0, true)
                } else {
                    (1, false)
                }
            }
            Slot::Many(v) => {
                let before = v.len();
                v.retain(|d| keep(d));
                let gone = before - v.len();
                if v.len() == 1 {
                    *self = Slot::One(v[0]);
                }
                (gone, !matches!(self, Slot::Many(v) if v.is_empty()))
            }
        }
    }
}

type Map<K> = HashMap<K, Slot, foldhash::fast::RandomState>;

/// An address family as an integer, masked to a prefix length.
trait Bits: Copy + Eq + Hash + fmt::Debug {
    const WIDTH: u8;
    fn mask(self, prefix: u8) -> Self;
}

impl Bits for u32 {
    const WIDTH: u8 = 32;
    fn mask(self, prefix: u8) -> u32 {
        match prefix {
            0 => 0,
            p => self & (u32::MAX << (Self::WIDTH - p)),
        }
    }
}

impl Bits for u128 {
    const WIDTH: u8 = 128;
    fn mask(self, prefix: u8) -> u128 {
        match prefix {
            0 => 0,
            p => self & (u128::MAX << (Self::WIDTH - p)),
        }
    }
}

/// Single addresses in one map; ranges in one map per prefix length, longest first.
/// Ranges stay ranges: a /16 is one entry, looked up by masking the client address.
struct Family<K> {
    exact: Map<K>,
    ranges: Vec<(u8, Map<K>)>,
}

impl<K> Default for Family<K> {
    fn default() -> Self {
        Family {
            exact: Map::default(),
            ranges: Vec::new(),
        }
    }
}

impl<K: Bits> Family<K> {
    fn map(&mut self, prefix: u8) -> &mut Map<K> {
        if prefix == K::WIDTH {
            return &mut self.exact;
        }
        let i = match self.ranges.iter().position(|(p, _)| *p <= prefix) {
            Some(i) if self.ranges[i].0 == prefix => i,
            Some(i) => {
                self.ranges.insert(i, (prefix, Map::default()));
                i
            }
            None => {
                self.ranges.push((prefix, Map::default()));
                self.ranges.len() - 1
            }
        };
        &mut self.ranges[i].1
    }

    fn insert(&mut self, net: K, prefix: u8, d: Decision) -> bool {
        match self.map(prefix).entry(net) {
            std::collections::hash_map::Entry::Occupied(mut e) => e.get_mut().add(d),
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(Slot::One(d));
                true
            }
        }
    }

    fn remove(&mut self, net: K, prefix: u8, id: i64) -> bool {
        let map = if prefix == K::WIDTH {
            &mut self.exact
        } else {
            match self.ranges.iter_mut().find(|(p, _)| *p == prefix) {
                Some((_, m)) => m,
                None => return false,
            }
        };
        let Some(slot) = map.get_mut(&net) else {
            return false;
        };
        let (gone, left) = slot.retain(|d| d.id != id);
        if !left {
            map.remove(&net);
        }
        self.ranges.retain(|(_, m)| !m.is_empty());
        gone > 0
    }

    fn check(&self, ip: K, now: u32) -> Option<Remedy> {
        let mut best = None;
        let slots = self
            .exact
            .get(&ip)
            .into_iter()
            .chain(self.ranges.iter().filter_map(|(p, m)| m.get(&ip.mask(*p))));
        for slot in slots {
            for d in slot.all() {
                if d.until > now {
                    best = best.max(Some(d.remedy));
                }
            }
            if best == Some(Remedy::Ban) {
                break;
            }
        }
        best
    }

    fn prune(&mut self, now: u32) -> usize {
        let mut gone = 0;
        let mut sweep = |m: &mut Map<K>| {
            m.retain(|_, slot| {
                let (g, left) = slot.retain(|d| d.until > now);
                gone += g;
                left
            });
        };
        sweep(&mut self.exact);
        for (_, m) in &mut self.ranges {
            sweep(m);
        }
        self.ranges.retain(|(_, m)| !m.is_empty());
        gone
    }
}

/// An address or a range, host bits cleared. IPv4-mapped IPv6 is stored as IPv4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Net {
    V4(u32, u8),
    V6(u128, u8),
}

/// The `scope` and `value` of a decision. Scopes other than `ip` and `range` (country, AS)
/// are not about addresses and are ignored. An `ip` may be written `1.2.3.4/32`.
fn parse_net(scope: &str, value: &str) -> Option<Net> {
    if !scope.eq_ignore_ascii_case("ip") && !scope.eq_ignore_ascii_case("range") {
        return None;
    }
    let (addr, prefix) = match value.trim().split_once('/') {
        Some((a, p)) => (a, Some(p.parse::<u8>().ok()?)),
        None => (value.trim(), None),
    };
    match addr.parse::<IpAddr>().ok()? {
        IpAddr::V4(v4) => {
            let p = prefix.unwrap_or(32);
            (p <= 32).then(|| Net::V4(u32::from(v4).mask(p), p))
        }
        IpAddr::V6(v6) => {
            let p = prefix.unwrap_or(128);
            if p > 128 {
                return None;
            }
            match v6.to_ipv4_mapped() {
                Some(v4) if p >= 96 => Some(Net::V4(u32::from(v4).mask(p - 96), p - 96)),
                _ => Some(Net::V6(u128::from(v6).mask(p), p)),
            }
        }
    }
}

#[derive(Default)]
struct Table {
    v4: Family<u32>,
    v6: Family<u128>,
    len: usize,
}

impl Table {
    fn insert(&mut self, net: Net, d: Decision) {
        let added = match net {
            Net::V4(a, p) => self.v4.insert(a, p, d),
            Net::V6(a, p) => self.v6.insert(a, p, d),
        };
        self.len += usize::from(added);
    }

    fn remove(&mut self, net: Net, id: i64) {
        let removed = match net {
            Net::V4(a, p) => self.v4.remove(a, p, id),
            Net::V6(a, p) => self.v6.remove(a, p, id),
        };
        self.len -= usize::from(removed);
    }

    fn check(&self, ip: IpAddr, now: u32) -> Option<Remedy> {
        match ip.to_canonical() {
            IpAddr::V4(a) => self.v4.check(u32::from(a), now),
            IpAddr::V6(a) => self.v6.check(u128::from(a), now),
        }
    }

    fn prune(&mut self, now: u32) -> usize {
        let gone = self.v4.prune(now) + self.v6.prune(now);
        self.len -= gone;
        gone
    }
}

/// A Go `time.Duration` as printed by the local API ("22h20m11.909s", "-5m", "300ms"), in
/// milliseconds.
fn go_duration_ms(s: &str) -> Option<i64> {
    let (neg, mut rest) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    if rest == "0" {
        return Some(0);
    }
    if rest.is_empty() {
        return None;
    }
    let mut total = 0f64;
    while !rest.is_empty() {
        let n = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(rest.len());
        let num: f64 = rest[..n].parse().ok()?;
        rest = &rest[n..];
        let u = rest
            .find(|c: char| c.is_ascii_digit() || c == '.')
            .unwrap_or(rest.len());
        let unit = match &rest[..u] {
            "h" => 3_600_000.0,
            "m" => 60_000.0,
            "s" => 1_000.0,
            "ms" => 1.0,
            "us" | "µs" | "μs" => 1e-3,
            "ns" => 1e-6,
            _ => return None,
        };
        rest = &rest[u..];
        total += num * unit;
    }
    let ms = total.round() as i64;
    Some(if neg { -ms } else { ms })
}

// ---------------------------------------------------------------------------------------
// The stream response, applied while it is parsed

/// One decision as the local API sends it; strings borrow from the response where possible.
#[derive(Deserialize)]
struct Wire<'a> {
    #[serde(default)]
    id: i64,
    #[serde(borrow)]
    scope: Cow<'a, str>,
    #[serde(borrow)]
    value: Cow<'a, str>,
    #[serde(borrow, rename = "type")]
    kind: Cow<'a, str>,
    #[serde(borrow, default)]
    duration: Cow<'a, str>,
}

/// Where parsed decisions go: a new table (full list) or a list of changes (delta).
trait Sink {
    fn add(&mut self, net: Net, d: Decision);
    fn delete(&mut self, net: Net, id: i64);
}

impl Sink for Table {
    fn add(&mut self, net: Net, d: Decision) {
        self.insert(net, d);
    }

    /// A full list starts empty: deleted decisions are not in it.
    fn delete(&mut self, _net: Net, _id: i64) {}
}

/// Changes collected off the lock, then applied in one short write.
#[derive(Default)]
struct Delta(Vec<(Net, Result<Decision, i64>)>);

impl Sink for Delta {
    fn add(&mut self, net: Net, d: Decision) {
        self.0.push((net, Ok(d)));
    }

    fn delete(&mut self, net: Net, id: i64) {
        self.0.push((net, Err(id)));
    }
}

struct Apply<'s, S> {
    sink: &'s mut S,
    now: u32,
    new: usize,
    deleted: usize,
    ignored: usize,
}

impl<S: Sink> Apply<'_, S> {
    fn one(&mut self, w: Wire<'_>, deleted: bool) {
        let Some(net) = parse_net(&w.scope, &w.value) else {
            self.ignored += 1;
            return;
        };
        if deleted {
            self.sink.delete(net, w.id);
            self.deleted += 1;
            return;
        }
        let Some(ms) = go_duration_ms(&w.duration) else {
            self.ignored += 1;
            return;
        };
        if ms <= 0 {
            return;
        }
        let secs = u32::try_from((ms + 999) / 1000).unwrap_or(u32::MAX);
        self.sink.add(
            net,
            Decision {
                id: w.id,
                until: self.now.saturating_add(secs),
                remedy: Remedy::parse(&w.kind),
            },
        );
        self.new += 1;
    }
}

impl<'de, S: Sink> DeserializeSeed<'de> for &mut Apply<'_, S> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de, S: Sink> Visitor<'de> for &mut Apply<'_, S> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a decision stream object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while let Some(key) = map.next_key::<Cow<'de, str>>()? {
            match &*key {
                "new" => map.next_value_seed(List(&mut *self, false))?,
                "deleted" => map.next_value_seed(List(&mut *self, true))?,
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(())
    }
}

/// `new` or `deleted`: an array, or null when empty.
struct List<'a, 's, S>(&'a mut Apply<'s, S>, bool);

impl<'de, S: Sink> DeserializeSeed<'de> for List<'_, '_, S> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_option(self)
    }
}

impl<'de, S: Sink> Visitor<'de> for List<'_, '_, S> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a list of decisions or null")
    }

    fn visit_none<E>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_unit<E>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_some<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while let Some(w) = seq.next_element::<Wire<'de>>()? {
            self.0.one(w, self.1);
        }
        Ok(())
    }
}

/// Counts of one applied response.
#[derive(Debug, Default, PartialEq, Eq)]
struct Applied {
    new: usize,
    deleted: usize,
    ignored: usize,
}

fn parse_into<S: Sink>(body: &[u8], sink: &mut S, now: u32) -> Result<Applied, String> {
    let mut a = Apply {
        sink,
        now,
        new: 0,
        deleted: 0,
        ignored: 0,
    };
    let mut de = serde_json::Deserializer::from_slice(body);
    (&mut a)
        .deserialize(&mut de)
        .and_then(|()| de.end())
        .map_err(|e| format!("decision stream: {e}"))?;
    Ok(Applied {
        new: a.new,
        deleted: a.deleted,
        ignored: a.ignored,
    })
}

// ---------------------------------------------------------------------------------------
// The process-wide store

#[derive(Clone, Default)]
pub struct PullStatus {
    pub last_success: Option<SystemTime>,
    pub last_error: Option<String>,
    /// Failed pulls in a row.
    pub failures: u32,
}

pub struct Store {
    table: RwLock<Table>,
    pub status: Mutex<PullStatus>,
}

/// One bouncer per process: decisions are shared by every site and survive reloads.
pub static STORE: LazyLock<Store> = LazyLock::new(|| Store {
    table: RwLock::new(Table::default()),
    status: Mutex::new(PullStatus::default()),
});

impl Store {
    pub fn check(&self, ip: IpAddr) -> Option<Remedy> {
        let t = self.table.read().unwrap_or_else(PoisonError::into_inner);
        if t.len == 0 {
            return None;
        }
        t.check(ip, now())
    }

    pub fn len(&self) -> usize {
        self.table
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .len
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, Table> {
        self.table.write().unwrap_or_else(PoisonError::into_inner)
    }
}

// ---------------------------------------------------------------------------------------
// Compiled configuration

static X_API_KEY: HeaderName = HeaderName::from_static("x-api-key");

const BAN_PAGE: &str = "<!DOCTYPE html>\r\n<html>\r\n<head><meta charset=\"utf-8\">\
    <title>403 Forbidden</title></head>\r\n<body>\r\n<center><h1>403 Forbidden</h1>\
    <p>Requests from your address are blocked.</p></center>\r\n<hr><center>conduit</center>\r\n\
    </body>\r\n</html>\r\n";

/// The `crowdsec:` section, checked and ready; rebuilt with every reload.
pub struct Plan {
    pub lapi_url: String,
    /// Stream URL without `startup`.
    stream: String,
    api_key: HeaderValue,
    /// What the decisions come from; when it changes, the next pull fetches the full list.
    source: String,
    pub update_interval: Duration,
    lapi: httpc::Client,
    ban_page: Bytes,
    pub appsec: Option<AppSec>,
}

impl fmt::Debug for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Plan")
            .field("lapi_url", &self.lapi_url)
            .finish_non_exhaustive()
    }
}

/// Characters a query value may hold as they are; the rest is percent-encoded.
fn query_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/:".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn base_url(what: &str, s: &str) -> Result<(Uri, String), String> {
    let s = s.trim().trim_end_matches('/');
    let uri: Uri = s
        .parse()
        .map_err(|_| format!("{what} {s:?} is not a URL"))?;
    if !matches!(uri.scheme_str(), Some("http" | "https")) || uri.host().is_none() {
        return Err(format!("{what} {s:?} must be an http:// or https:// URL"));
    }
    Ok((uri, s.to_string()))
}

impl Plan {
    pub fn build(
        c: &CrowdSecSection,
        watched: &mut Vec<std::path::PathBuf>,
    ) -> Result<Plan, String> {
        let key = match (&c.api_key, &c.api_key_file) {
            (Some(k), None) => k.trim().to_string(),
            (None, Some(f)) => {
                watched.push(f.into());
                std::fs::read_to_string(f)
                    .map_err(|e| format!("crowdsec.api_key_file {f}: {e}"))?
                    .trim()
                    .to_string()
            }
            _ => return Err("crowdsec: give exactly one of api_key and api_key_file".into()),
        };
        let mut api_key = HeaderValue::try_from(key.as_str())
            .ok()
            .filter(|_| !key.is_empty())
            .ok_or("crowdsec: the API key is empty or not printable")?;
        api_key.set_sensitive(true);

        let (_, base) = base_url("crowdsec.lapi_url", &c.lapi_url)?;
        let mut stream = format!("{base}/v1/decisions/stream?scopes=ip,range");
        for (name, values) in [
            ("origins", &c.origins),
            ("scenarios_containing", &c.scenarios_containing),
            ("scenarios_not_containing", &c.scenarios_not_containing),
        ] {
            if !values.is_empty() {
                let v: Vec<_> = values.iter().map(|v| query_value(v.trim())).collect();
                stream.push_str(&format!("&{name}={}", v.join(",")));
            }
        }
        stream
            .parse::<Uri>()
            .map_err(|e| format!("crowdsec: stream URL {stream}: {e}"))?;
        let update_interval = c.update_interval.0;
        if update_interval < Duration::from_secs(1) {
            return Err("crowdsec.update_interval must be at least 1s".into());
        }
        let ban_page = match &c.ban_page {
            Some(p) => {
                watched.push(p.into());
                Bytes::from(std::fs::read(p).map_err(|e| format!("crowdsec.ban_page {p}: {e}"))?)
            }
            None => Bytes::from_static(BAN_PAGE.as_bytes()),
        };
        let appsec = match &c.appsec {
            Some(a) => {
                let (url, _) = base_url("crowdsec.appsec.url", &a.url)?;
                let body_limit = match a.body_limit.0 {
                    Some(n) if n <= MAX_BODY as u64 => n as usize,
                    _ => {
                        return Err(format!(
                            "crowdsec.appsec.body_limit: at most {}k (the request body \
                             conduit can hold and replay)",
                            MAX_BODY >> 10
                        ));
                    }
                };
                if a.timeout.0.is_zero() {
                    return Err("crowdsec.appsec.timeout must be more than 0".into());
                }
                Some(AppSec {
                    url,
                    api_key: api_key.clone(),
                    timeout: a.timeout.0,
                    block_on_failure: a.failure_action == FailureAction::Block,
                    body_limit,
                    client: httpc::Client::new(c.ca_file.as_deref(), a.timeout.0, 64 << 10, 128),
                })
            }
            None => None,
        };
        Ok(Plan {
            lapi_url: base,
            source: format!("{stream}\n{key}\n{:?}", c.ca_file),
            stream,
            api_key,
            update_interval,
            lapi: httpc::Client::new(c.ca_file.as_deref(), LAPI_TIMEOUT, LAPI_MAX_RESPONSE, 2),
            ban_page,
            appsec,
        })
    }

    /// The response to a refused client.
    pub fn refuse(&self, status: u16, by: &'static str) -> Local {
        metrics::CROWDSEC_BLOCKED.with_label_values(&[by]).inc();
        Local {
            status,
            headers: vec![
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/html; charset=utf-8"),
                ),
                (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
            ],
            body: self.ban_page.clone(),
            blocked_by: Some(by),
        }
    }
}

/// Bounds connecting and each read or write.
const LAPI_TIMEOUT: Duration = Duration::from_secs(10);
/// Bounds a whole pull, however the answer trickles in.
const LAPI_DEADLINE: Duration = Duration::from_secs(60);
/// A full list of 100k decisions is about 15 MB of JSON.
const LAPI_MAX_RESPONSE: usize = 64 << 20;

// ---------------------------------------------------------------------------------------
// The pulling service

async fn pull(plan: &Plan, startup: bool) -> Result<Bytes, String> {
    let uri: Uri = format!("{}&startup={startup}", plan.stream)
        .parse()
        .map_err(|e| format!("stream URL: {e}"))?;
    let headers = [
        (X_API_KEY.clone(), plan.api_key.clone()),
        (
            header::USER_AGENT,
            HeaderValue::from_static(concat!("conduit/", env!("CARGO_PKG_VERSION"))),
        ),
    ];
    let send = plan
        .lapi
        .send(Method::GET, &uri, &headers, Bytes::new(), true);
    let reply = tokio::time::timeout(LAPI_DEADLINE, send)
        .await
        .map_err(|_| format!("no complete answer within {LAPI_DEADLINE:?}"))?
        .map_err(|e| e.to_string())?;
    match reply.status.as_u16() {
        200 => Ok(reply.body),
        401 | 403 => Err(format!(
            "the local API refused the bouncer key ({})",
            reply.status
        )),
        _ => Err(format!("the local API answered {}", reply.status)),
    }
}

/// One pull, applied. `full`: fetch the whole list and replace the table with it.
async fn sync(plan: &Plan, full: bool) -> Result<Applied, String> {
    let body = pull(plan, full).await?;
    let now = now();
    if full {
        let mut table = Table::default();
        let applied = parse_into(&body, &mut table, now)?;
        drop(body);
        *STORE.write() = table;
        Ok(applied)
    } else {
        let mut delta = Delta::default();
        let applied = parse_into(&body, &mut delta, now)?;
        let mut t = STORE.write();
        for (net, change) in delta.0 {
            match change {
                Ok(d) => t.insert(net, d),
                Err(id) => t.remove(net, id),
            }
        }
        Ok(applied)
    }
}

/// Expired decisions are dropped this often; lookups skip them meanwhile.
const PRUNE_EVERY: u32 = 60;

pub struct Service {
    shared: Arc<Shared>,
}

impl Service {
    pub fn new(shared: Arc<Shared>) -> Service {
        Service { shared }
    }
}

#[async_trait]
impl BackgroundService for Service {
    async fn start(&self, mut shutdown: ShutdownWatch) {
        // The source the table is in step with; `None` after a failure, so the next pull
        // fetches the full list.
        let mut synced: Option<String> = None;
        let mut pruned = now();
        loop {
            let plan = self.shared.runtime.load().crowdsec.clone();
            let wait = match plan {
                None => {
                    if synced.take().is_some() || STORE.len() > 0 {
                        *STORE.write() = Table::default();
                        metrics::CROWDSEC_DECISIONS.set(0);
                    }
                    Duration::from_secs(10)
                }
                Some(plan) => {
                    let full = synced.as_deref() != Some(plan.source.as_str());
                    let result = tokio::select! {
                        _ = shutdown.changed() => return,
                        r = sync(&plan, full) => r,
                    };
                    let mut st = STORE.status.lock().unwrap_or_else(PoisonError::into_inner);
                    match result {
                        Ok(a) => {
                            metrics::CROWDSEC_PULLS
                                .with_label_values(&["success"])
                                .inc();
                            if full || st.failures > 0 {
                                info!(
                                    "crowdsec: {} decisions from {} ({} ignored)",
                                    STORE.len(),
                                    plan.lapi_url,
                                    a.ignored
                                );
                            }
                            synced = Some(plan.source.clone());
                            st.last_success = Some(SystemTime::now());
                            st.last_error = None;
                            st.failures = 0;
                        }
                        Err(e) => {
                            metrics::CROWDSEC_PULLS
                                .with_label_values(&["failure"])
                                .inc();
                            st.failures += 1;
                            // About every 5 minutes while it lasts.
                            if st.failures == 1 || st.failures.is_multiple_of(30) {
                                warn!(
                                    "crowdsec: pulling decisions from {} failed ({} in a row), \
                                     keeping {} held: {e}",
                                    plan.lapi_url,
                                    st.failures,
                                    STORE.len()
                                );
                            }
                            st.last_error = Some(e);
                            synced = None;
                        }
                    }
                    drop(st);
                    if now().saturating_sub(pruned) >= PRUNE_EVERY {
                        pruned = now();
                        STORE.write().prune(pruned);
                    }
                    metrics::CROWDSEC_DECISIONS.set(STORE.len() as i64);
                    metrics::CROWDSEC_LAST_PULL.set(
                        STORE
                            .status
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .last_success
                            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                            .map_or(0, |d| d.as_secs() as i64),
                    );
                    plan.update_interval
                }
            };
            tokio::select! {
                _ = shutdown.changed() => return,
                _ = tokio::time::sleep(wait) => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// AppSec

/// What conduit can hold of a request body and still forward it: Pingora's replay buffer.
pub const MAX_BODY: usize = 64 << 10;

pub struct AppSec {
    url: Uri,
    api_key: HeaderValue,
    pub timeout: Duration,
    block_on_failure: bool,
    body_limit: usize,
    client: httpc::Client,
}

pub enum Verdict {
    Allow,
    Block(u16),
}

/// Headers of the client's request that are about its own connection, or that it could use
/// to pass itself off as conduit.
fn forwarded(name: &HeaderName) -> bool {
    !matches!(
        name.as_str(),
        "host"
            | "content-length"
            | "transfer-encoding"
            | "connection"
            | "keep-alive"
            | "proxy-connection"
            | "te"
            | "trailer"
            | "upgrade"
            | "expect"
    ) && !name.as_str().starts_with("x-crowdsec-appsec-")
}

fn http_version(v: http::Version) -> &'static str {
    match v {
        http::Version::HTTP_09 => "09",
        http::Version::HTTP_10 => "10",
        http::Version::HTTP_2 => "20",
        http::Version::HTTP_3 => "30",
        _ => "11",
    }
}

/// The status AppSec asks for in a 403 verdict: `{"action": "ban", "http_status": 403}`.
fn verdict_status(body: &[u8]) -> u16 {
    #[derive(Deserialize)]
    struct V {
        http_status: Option<u16>,
    }
    serde_json::from_slice::<V>(body)
        .ok()
        .and_then(|v| v.http_status)
        .filter(|s| (400..=599).contains(s))
        .unwrap_or(403)
}

/// Last AppSec error warning, in `now()` seconds plus one (0: never).
static APPSEC_WARNED: AtomicU64 = AtomicU64::new(0);

impl AppSec {
    /// How much of the request body goes with the headers: all of it when its length is
    /// declared and within `body_limit`, and the client is not waiting for `100 Continue`
    /// first. Otherwise none, and AppSec judges the headers alone.
    pub fn body_len(&self, req: &RequestHeader) -> usize {
        let h = &req.headers;
        if h.contains_key(header::TRANSFER_ENCODING)
            || h.get(header::EXPECT)
                .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"100-continue"))
        {
            return 0;
        }
        h.get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<usize>().ok())
            .filter(|n| *n <= self.body_limit)
            .unwrap_or(0)
    }

    pub async fn inspect(
        &self,
        req: &RequestHeader,
        uri: &[u8],
        client: IpAddr,
        host: &str,
        body: Bytes,
    ) -> Verdict {
        let mut h: Vec<(HeaderName, HeaderValue)> = Vec::with_capacity(req.headers.len() + 7);
        let mut add = |name: &'static str, v: Option<HeaderValue>| {
            if let Some(v) = v {
                h.push((HeaderName::from_static(name), v));
            }
        };
        add(
            "x-crowdsec-appsec-ip",
            HeaderValue::try_from(client.to_canonical().to_string()).ok(),
        );
        add("x-crowdsec-appsec-uri", HeaderValue::from_bytes(uri).ok());
        add("x-crowdsec-appsec-host", HeaderValue::try_from(host).ok());
        add(
            "x-crowdsec-appsec-verb",
            HeaderValue::try_from(req.method.as_str()).ok(),
        );
        add("x-crowdsec-appsec-api-key", Some(self.api_key.clone()));
        add(
            "x-crowdsec-appsec-user-agent",
            req.headers.get(header::USER_AGENT).cloned(),
        );
        add(
            "x-crowdsec-appsec-http-version",
            Some(HeaderValue::from_static(http_version(req.version))),
        );
        for (name, value) in &req.headers {
            if forwarded(name) {
                h.push((name.clone(), value.clone()));
            }
        }
        let method = if body.is_empty() {
            Method::GET
        } else {
            Method::POST
        };
        let sent = tokio::time::timeout(
            self.timeout,
            self.client.send(method, &self.url, &h, body, true),
        )
        .await;
        let error = match sent {
            Ok(Ok(reply)) => match reply.status.as_u16() {
                200 => {
                    metrics::APPSEC.with_label_values(&["allow"]).inc();
                    return Verdict::Allow;
                }
                403 => {
                    metrics::APPSEC.with_label_values(&["block"]).inc();
                    return Verdict::Block(verdict_status(&reply.body));
                }
                401 => "AppSec refused the bouncer key (401)".to_string(),
                s => format!("AppSec answered {s}"),
            },
            Ok(Err(e)) => e.to_string(),
            Err(_) => format!("no verdict within {:?}", self.timeout),
        };
        metrics::APPSEC.with_label_values(&["error"]).inc();
        // At most one warning a minute: under load every request would log.
        let t = u64::from(now()) + 1;
        let last = APPSEC_WARNED.load(Relaxed);
        if (last == 0 || t >= last + 60)
            && APPSEC_WARNED
                .compare_exchange(last, t, Relaxed, Relaxed)
                .is_ok()
        {
            let action = if self.block_on_failure {
                "blocking"
            } else {
                "allowing"
            };
            warn!("crowdsec: {error}; {action} requests meanwhile");
        }
        if self.block_on_failure {
            Verdict::Block(403)
        } else {
            Verdict::Allow
        }
    }
}

// ---------------------------------------------------------------------------------------
// The site plugin

/// `crowdsec`: refuse clients with a decision. AppSec, if asked for, runs later in the
/// request (it is asynchronous; plugin hooks are not).
pub struct CrowdSecPlugin {
    plan: Arc<Plan>,
    pub appsec: bool,
}

impl CrowdSecPlugin {
    pub fn new(plan: Option<&Arc<Plan>>, appsec: bool) -> Result<CrowdSecPlugin, String> {
        let plan = plan.ok_or("needs the top-level crowdsec: section")?;
        if appsec && plan.appsec.is_none() {
            return Err("appsec: true needs crowdsec.appsec".into());
        }
        Ok(CrowdSecPlugin {
            plan: plan.clone(),
            appsec,
        })
    }
}

impl Plugin for CrowdSecPlugin {
    fn name(&self) -> &'static str {
        "crowdsec"
    }

    fn request(&self, req: &RequestView<'_>) -> Option<Local> {
        let ip = req.client?;
        STORE.check(ip)?;
        Some(self.plan.refuse(403, "crowdsec"))
    }

    fn appsec(&self) -> bool {
        self.appsec
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(id: i64, until: u32, remedy: Remedy) -> Decision {
        Decision { id, until, remedy }
    }

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn go_durations() {
        assert_eq!(go_duration_ms("22h20m11.909761348s"), Some(80_411_910));
        assert!(go_duration_ms("-18897h25m52.809576151s").unwrap() < 0);
        assert_eq!(go_duration_ms("4m"), Some(240_000));
        assert_eq!(go_duration_ms("300ms"), Some(300));
        assert_eq!(go_duration_ms("1.5h"), Some(5_400_000));
        assert_eq!(go_duration_ms("0"), Some(0));
        assert_eq!(go_duration_ms("0s"), Some(0));
        assert_eq!(go_duration_ms("2µs"), Some(0));
        assert_eq!(go_duration_ms(""), None);
        assert_eq!(go_duration_ms("5d"), None);
        assert_eq!(go_duration_ms("h"), None);
    }

    /// `cargo test --release lookup_cost -- --ignored --nocapture`
    #[test]
    #[ignore = "measurement"]
    fn lookup_cost() {
        let mut t = Table::default();
        let mut x: u32 = 0x9e37_79b9;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x
        };
        for id in 0..100_000 {
            t.insert(Net::V4(next(), 32), d(id, 3600, Remedy::Ban));
        }
        for (id, p) in (100_000..).zip([24u8, 24, 16, 20, 28, 12, 24, 22]) {
            t.insert(Net::V4(next().mask(p), p), d(id, 3600, Remedy::Ban));
        }
        let probes: Vec<IpAddr> = (0..1_000_000)
            .map(|_| IpAddr::from(next().to_be_bytes()))
            .collect();
        let start = Instant::now();
        let hits = probes
            .iter()
            .filter(|ip| t.check(**ip, 1).is_some())
            .count();
        let ns = start.elapsed().as_nanos() as f64 / probes.len() as f64;
        println!(
            "{} decisions, {} range prefixes: {ns:.0} ns per lookup ({hits} hits)",
            t.len,
            t.v4.ranges.len()
        );
    }

    #[test]
    fn slot_stays_small() {
        assert_eq!(std::mem::size_of::<Slot>(), 16);
    }

    #[test]
    fn nets() {
        assert_eq!(parse_net("Ip", "1.2.3.4"), Some(Net::V4(0x0102_0304, 32)));
        assert_eq!(
            parse_net("ip", "1.2.3.4/32"),
            Some(Net::V4(0x0102_0304, 32))
        );
        assert_eq!(
            parse_net("Range", "1.2.3.77/24"),
            Some(Net::V4(0x0102_0300, 24))
        );
        assert_eq!(parse_net("range", "0.0.0.0/0"), Some(Net::V4(0, 0)));
        assert_eq!(
            parse_net("range", "::ffff:10.1.2.0/120"),
            Some(Net::V4(0x0a01_0200, 24))
        );
        assert_eq!(
            parse_net("range", "2001:db8:1:2::/64"),
            Some(Net::V6(0x2001_0db8_0001_0002 << 64, 64))
        );
        assert_eq!(parse_net("country", "FR"), None);
        assert_eq!(parse_net("ip", "1.2.3.4/33"), None);
        assert_eq!(parse_net("ip", "nope"), None);
    }

    #[test]
    fn table_lookups_and_priority() {
        let mut t = Table::default();
        let now = 1000;
        t.insert(
            parse_net("ip", "192.0.2.1").unwrap(),
            d(1, now + 60, Remedy::Captcha),
        );
        t.insert(
            parse_net("range", "192.0.2.0/24").unwrap(),
            d(2, now + 60, Remedy::Ban),
        );
        t.insert(
            parse_net("range", "198.51.100.0/24").unwrap(),
            d(3, now + 60, Remedy::Captcha),
        );
        t.insert(
            parse_net("ip", "2001:db8::1").unwrap(),
            d(4, now + 60, Remedy::Ban),
        );
        t.insert(
            parse_net("range", "2001:db8:ff::/48").unwrap(),
            d(5, now + 60, Remedy::Ban),
        );
        assert_eq!(t.len, 5);
        // The range's ban outranks the address's captcha.
        assert_eq!(t.check(ip("192.0.2.1"), now), Some(Remedy::Ban));
        assert_eq!(t.check(ip("::ffff:192.0.2.9"), now), Some(Remedy::Ban));
        assert_eq!(t.check(ip("198.51.100.200"), now), Some(Remedy::Captcha));
        assert_eq!(t.check(ip("192.0.3.1"), now), None);
        assert_eq!(t.check(ip("2001:db8::1"), now), Some(Remedy::Ban));
        assert_eq!(t.check(ip("2001:db8:ff:1::7"), now), Some(Remedy::Ban));
        assert_eq!(t.check(ip("2001:db8::2"), now), None);
        // Expired decisions no longer count, even before pruning.
        assert_eq!(t.check(ip("192.0.2.1"), now + 60), None);

        // Removal is by id: the address keeps its own decision when the range goes.
        t.remove(parse_net("range", "192.0.2.0/24").unwrap(), 2);
        assert_eq!(t.check(ip("192.0.2.1"), now), Some(Remedy::Captcha));
        assert_eq!(t.check(ip("192.0.2.9"), now), None);
        t.remove(parse_net("ip", "192.0.2.1").unwrap(), 99);
        assert_eq!(t.len, 4);
        assert_eq!(t.v4.ranges.len(), 1);

        // Two decisions on one address; a re-sent id replaces instead of adding.
        let n = parse_net("ip", "203.0.113.5").unwrap();
        t.insert(n, d(10, now + 10, Remedy::Ban));
        t.insert(n, d(11, now + 100, Remedy::Captcha));
        t.insert(n, d(11, now + 200, Remedy::Captcha));
        assert_eq!(t.len, 6);
        assert_eq!(t.check(ip("203.0.113.5"), now + 50), Some(Remedy::Captcha));
        assert_eq!(t.prune(now + 50), 1);
        assert_eq!(t.len, 5);
        assert_eq!(t.prune(now + 1000), 5);
        assert_eq!(t.len, 0);
        assert!(t.v4.exact.is_empty() && t.v4.ranges.is_empty() && t.v6.ranges.is_empty());
    }

    const STARTUP: &str = r#"{"new":[
        {"duration":"22h20m11.9s","id":2266,"origin":"CAPI","scenario":"crowdsecurity/http-sensitive-files","scope":"ip","type":"ban","value":"91.241.19.122/32","simulated":false,"uuid":"x"},
        {"duration":"1h","id":7,"origin":"cscli","scenario":"manual","scope":"Range","type":"captcha","value":"10.0.0.0/8"},
        {"duration":"4h","id":8,"origin":"cscli","scenario":"manual","scope":"Country","type":"ban","value":"FR"},
        {"duration":"1h","id":9,"origin":"cscli","scenario":"weird \"quoted\"","scope":"Ip","type":"custom","value":"2001:db8::7"}
      ],
      "deleted":[{"duration":"-18897h25m52.8s","id":1,"origin":"crowdsec","scenario":"x","scope":"Ip","type":"ban","value":"123.206.50.249"}]}"#;

    #[test]
    fn stream_full_then_delta() {
        let now = 50;
        let mut t = Table::default();
        let a = parse_into(STARTUP.as_bytes(), &mut t, now).unwrap();
        assert_eq!(
            a,
            Applied {
                new: 3,
                deleted: 1,
                ignored: 1
            }
        );
        assert_eq!(t.len, 3);
        assert_eq!(t.check(ip("91.241.19.122"), now), Some(Remedy::Ban));
        assert_eq!(t.check(ip("10.9.8.7"), now), Some(Remedy::Captcha));
        // Unknown remediation types fall back to ban.
        assert_eq!(t.check(ip("2001:db8::7"), now), Some(Remedy::Ban));
        assert_eq!(t.check(ip("91.241.19.122"), now + 80_412), None);

        let mut delta = Delta::default();
        let a = parse_into(
            br#"{"new":null,"deleted":[{"duration":"-1s","id":7,"scope":"range","type":"captcha","value":"10.0.0.0/8"}]}"#,
            &mut delta,
            now,
        )
        .unwrap();
        assert_eq!((a.new, a.deleted), (0, 1));
        for (net, change) in delta.0 {
            match change {
                Ok(d) => t.insert(net, d),
                Err(id) => t.remove(net, id),
            }
        }
        assert_eq!(t.check(ip("10.9.8.7"), now), None);
        assert_eq!(t.len, 2);

        // Nothing changed: both null.
        let a = parse_into(
            br#"{"new":null,"deleted":null}"#,
            &mut Delta::default(),
            now,
        )
        .unwrap();
        assert_eq!(a, Applied::default());
        assert!(parse_into(b"{\"new\":[{]}", &mut Delta::default(), now).is_err());
        assert!(parse_into(b"[]", &mut Delta::default(), now).is_err());
    }

    fn plan(yaml: &str) -> Result<Plan, String> {
        let c: CrowdSecSection = crate::config::yaml(yaml).unwrap();
        Plan::build(&c, &mut Vec::new())
    }

    #[test]
    fn plan_from_config() {
        let p = plan(
            "api_key: k3y\norigins: [crowdsec, cscli]\nscenarios_containing: [\"http probing\"]\nappsec: { url: \"http://127.0.0.1:7422/\" }\n",
        )
        .unwrap();
        assert_eq!(
            p.stream,
            "http://127.0.0.1:8080/v1/decisions/stream?scopes=ip,range&origins=crowdsec,cscli\
             &scenarios_containing=http%20probing"
        );
        assert!(p.api_key.is_sensitive());
        let a = p.appsec.as_ref().unwrap();
        assert_eq!(a.url, "http://127.0.0.1:7422");
        assert_eq!(a.body_limit, 64 << 10);
        assert!(!a.block_on_failure);
        assert!(!format!("{p:?}").contains("k3y"));

        assert!(
            plan("lapi_url: http://x\n")
                .unwrap_err()
                .contains("exactly one")
        );
        assert!(
            plan("api_key: k\nlapi_url: ftp://x\n")
                .unwrap_err()
                .contains("http://")
        );
        assert!(
            plan("api_key: k\nappsec: { body_limit: 1m }\n")
                .unwrap_err()
                .contains("at most 64k")
        );
        assert!(plan("api_key: k\nupdate_interval: 500ms\n").is_err());
        let blocked = plan("api_key: k\nappsec: { failure_action: block, timeout: 1s }\n").unwrap();
        assert!(blocked.appsec.unwrap().block_on_failure);
    }

    #[test]
    fn appsec_request_shape() {
        let p = plan("api_key: k\nappsec: {}\n").unwrap();
        let a = p.appsec.unwrap();
        let mut req = RequestHeader::build("POST", b"/login", None).unwrap();
        req.insert_header("content-length", "12").unwrap();
        assert_eq!(a.body_len(&req), 12);
        req.insert_header("expect", "100-continue").unwrap();
        assert_eq!(a.body_len(&req), 0);
        req.remove_header("expect");
        req.insert_header("content-length", "70000").unwrap();
        assert_eq!(a.body_len(&req), 0);
        assert!(!forwarded(&HeaderName::from_static("x-crowdsec-appsec-ip")));
        assert!(!forwarded(&header::CONTENT_LENGTH));
        assert!(forwarded(&header::COOKIE));
        assert_eq!(
            verdict_status(br#"{"action":"ban","http_status":418}"#),
            418
        );
        assert_eq!(
            verdict_status(br#"{"action":"ban","http_status":200}"#),
            403
        );
        assert_eq!(verdict_status(b""), 403);
        assert_eq!(http_version(http::Version::HTTP_2), "20");
    }

    #[test]
    fn site_plugin_refuses_held_decisions() {
        use crate::config::parse;
        use crate::runtime::build;
        let yaml = |plugins: &str, section: &str| {
            format!(
                "listen: {{ http: [\"127.0.0.1:0\"] }}\n{section}sites:\n  - id: t\n    hosts: [t.test]\n    upstream: http://127.0.0.1:1\n    plugins:\n{plugins}"
            )
        };
        let section = "crowdsec: { api_key: k, appsec: {} }\n";
        let cfg = parse(&yaml("      - crowdsec:\n", section)).unwrap();
        let rt = build(&cfg, None, None).unwrap().runtime;
        let site = rt.router.lookup("t.test").unwrap();
        assert_eq!(site.plugins.names(), ["crowdsec"]);
        assert!(!site.plugins.appsec());

        // 192.0.2.0/24 is used by no other test: the store is process-wide.
        STORE.write().insert(
            parse_net("range", "192.0.2.128/25").unwrap(),
            d(1, now() + 3600, Remedy::Ban),
        );
        let headers = http::HeaderMap::new();
        let view = |client: &str| RequestView {
            method: &Method::GET,
            https: true,
            host: "t.test",
            client: client.parse().ok(),
            path: b"/",
            uri: b"/",
            query: b"",
            headers: &headers,
        };
        let refused = site
            .plugins
            .request(&view("192.0.2.200"), &mut Vec::new())
            .unwrap();
        assert_eq!(refused.status, 403);
        assert_eq!(refused.blocked_by, Some("crowdsec"));
        assert!(refused.body.starts_with(b"<!DOCTYPE html>"));
        assert!(
            site.plugins
                .request(&view("192.0.2.100"), &mut Vec::new())
                .is_none()
        );
        assert!(site.plugins.request(&view(""), &mut Vec::new()).is_none());

        let cfg = parse(&yaml("      - crowdsec: { appsec: true }\n", section)).unwrap();
        let rt = build(&cfg, None, None).unwrap().runtime;
        assert!(rt.router.lookup("t.test").unwrap().plugins.appsec());

        let err = |plugins: &str, section: &str| match build(
            &parse(&yaml(plugins, section)).unwrap(),
            None,
            None,
        ) {
            Ok(_) => panic!("built"),
            Err(e) => e,
        };
        assert!(err("      - crowdsec: {}\n", "").contains("needs the top-level crowdsec"));
        assert!(
            err(
                "      - crowdsec: { appsec: true }\n",
                "crowdsec: { api_key: k }\n"
            )
            .contains("needs crowdsec.appsec")
        );
    }
}
