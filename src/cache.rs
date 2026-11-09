//! Response cache for sites with `cache:`, on Pingora's HTTP cache.
//!
//! Pingora decides when to look up, store, revalidate (conditional requests upstream on
//! stale entries), serve stale (`stale-while-revalidate`, `stale-if-error`), collapse
//! concurrent misses (the cache lock) and remember uncacheable URLs (the predictor). This
//! module supplies the parts Pingora leaves to the application: the storage, the key, and
//! the rules for what may be stored.
//!
//! Storage is in memory: sharded maps of serialized metadata and a `Bytes` body, so a hit
//! hands out the body without copying it. Size is bounded by Pingora's LRU eviction
//! manager, which this store keeps exact: replaced and purged entries leave it at once.
//!
//! What may be stored follows RFC 9111 for a shared cache, with two stricter rules:
//! responses that set cookies are never stored, and `Vary` covers `Accept-Encoding` whenever
//! the stored body is encoded, so a compressed body never reaches a client that cannot read it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use bytes::Bytes;
use http::{StatusCode, header};
use pingora::cache::cache_control::{CacheControl, Cacheable, InterpretCacheControl};
use pingora::cache::eviction::{EvictionManager, lru};
use pingora::cache::key::{CacheHashKey, CompactCacheKey, HashBinary};
use pingora::cache::lock::{CacheKeyLockImpl, CacheLock};
use pingora::cache::predictor::Predictor;
use pingora::cache::storage::{HandleHit, HandleMiss, MissFinishType};
use pingora::cache::trace::SpanHandle;
use pingora::cache::{
    CacheEntryId, CacheEntryKeyRef, CacheKey, CacheMeta, CacheMetaDefaults, CachePhase, HitHandler,
    MissHandler, NoCacheReason, PurgeOutcome, PurgeTarget, PurgeType, RespCacheable, Storage,
    VarianceBuilder, filters,
};
use pingora::http::{RequestHeader, ResponseHeader};
use pingora::{Error, ErrorType, Result};

use crate::config::{CacheSection, SiteCacheConfig};
use crate::files::Accepted;
use crate::rules::{Matcher, RequestView};

const SHARDS: usize = 16;
/// Bytes counted per entry on top of its body and serialized headers (map slot, key, Arc).
const ENTRY_OVERHEAD: usize = 256;

/// A site's compiled `cache:` settings.
pub struct SiteCache {
    matcher: Option<Matcher>,
    default_ttl: Option<Duration>,
    defaults: CacheMetaDefaults,
}

fn no_default_ttl(_: StatusCode) -> Option<Duration> {
    None
}

impl SiteCache {
    pub fn compile(c: &SiteCacheConfig) -> Result<SiteCache, String> {
        let sie = c
            .stale_if_error
            .map_or(0, |d| d.0.as_secs().min(u32::MAX as u64) as u32);
        if c.default_ttl.is_some_and(|d| d.0.is_zero()) {
            return Err("cache.default_ttl must be more than 0".into());
        }
        Ok(SiteCache {
            matcher: c.matcher.as_ref().map(Matcher::compile).transpose()?,
            default_ttl: c.default_ttl.map(|d| d.0),
            defaults: CacheMetaDefaults::new(no_default_ttl, 0, sie),
        })
    }

    /// Whether this request may be answered from, and stored in, the cache.
    pub fn takes(&self, req: &RequestView<'_>, headers: &RequestHeader) -> bool {
        (req.method == http::Method::GET || req.method == http::Method::HEAD)
            && !headers.headers.contains_key(header::UPGRADE)
            && !crate::proxy::has_body(headers)
            && self.matcher.as_ref().is_none_or(|m| m.is_match(req))
    }

    /// May this upstream response be stored, and until when is it fresh?
    pub fn cacheable(&self, req: &RequestHeader, resp: &ResponseHeader) -> RespCacheable {
        if resp.headers.contains_key(header::SET_COOKIE) {
            return RespCacheable::Uncacheable(NoCacheReason::Custom("set-cookie"));
        }
        if resp.status == StatusCode::PARTIAL_CONTENT {
            return RespCacheable::Uncacheable(NoCacheReason::Custom("partial"));
        }
        if vary_names(resp).any(|n| n == "*") {
            return RespCacheable::Uncacheable(NoCacheReason::Custom("vary"));
        }
        let cc = CacheControl::from_resp_headers(resp);
        let auth = req.headers.contains_key(header::AUTHORIZATION);
        match filters::resp_cacheable(cc.as_ref(), resp.clone(), auth, &self.defaults) {
            RespCacheable::Cacheable(meta) => RespCacheable::Cacheable(meta),
            RespCacheable::Uncacheable(reason) => match self.default_ttl {
                Some(ttl)
                    if !auth
                        && heuristic(resp.status)
                        && cc
                            .as_ref()
                            .is_none_or(|c| c.is_cacheable() != Cacheable::No) =>
                {
                    let now = SystemTime::now();
                    let (swr, sie) =
                        filters::calculate_serve_stale_durations(cc.as_ref(), &self.defaults);
                    let mut stored = resp.clone();
                    if let Some(c) = &cc {
                        c.strip_private_headers(&mut stored);
                    }
                    RespCacheable::Cacheable(CacheMeta::new(now + ttl, now, swr, sie, stored))
                }
                _ => RespCacheable::Uncacheable(reason),
            },
        }
    }
}

/// Statuses cacheable without explicit freshness (RFC 9110 section 15.1), less 206.
fn heuristic(s: StatusCode) -> bool {
    matches!(
        s.as_u16(),
        200 | 203 | 204 | 300 | 301 | 308 | 404 | 405 | 410 | 414 | 501
    )
}

fn vary_names(resp: &ResponseHeader) -> impl Iterator<Item = String> + '_ {
    resp.headers
        .get_all(header::VARY)
        .iter()
        .flat_map(|v| v.as_bytes().split(|b| *b == b','))
        .map(|n| String::from_utf8_lossy(n.trim_ascii()).to_ascii_lowercase())
        .filter(|n| !n.is_empty())
}

/// Which stored variant a request gets: the request headers the response varies on.
/// An encoded body always varies on whether the client accepts that encoding.
pub fn variance(meta: &CacheMeta, req: &RequestHeader) -> Option<HashBinary> {
    let resp = meta.response_header();
    let mut names: Vec<String> = vary_names(resp).collect();
    let listed = names.iter().any(|n| n == "accept-encoding");
    let coding = resp.headers.get(header::CONTENT_ENCODING);
    if coding.is_some() {
        names.push("accept-encoding".into());
    }
    if names.is_empty() {
        return None;
    }
    names.sort();
    names.dedup();
    let mut vb = VarianceBuilder::new();
    for name in names {
        let value = if name == "accept-encoding" {
            let accepted = Accepted::from_request(req);
            match coding.and_then(|c| c.to_str().ok()) {
                // Only the body's encoding matters here: does the client take it.
                Some(c) if !listed => match accepted.accepts_coding(c) {
                    Some(yes) => vec![b'=', u8::from(yes)],
                    None => req
                        .headers
                        .get(header::ACCEPT_ENCODING)
                        .map_or(Vec::new(), |v| v.as_bytes().to_vec()),
                },
                // The upstream chose from the client's whole list: the same encodings, in
                // any spelling, share a variant.
                _ => vec![accepted.bits()],
            }
        } else {
            let mut v = Vec::new();
            for (i, h) in req.headers.get_all(name.as_str()).iter().enumerate() {
                if i > 0 {
                    v.extend_from_slice(b", ");
                }
                v.extend_from_slice(h.as_bytes());
            }
            v
        };
        vb.add_owned_name_value(name, value);
    }
    vb.finalize()
}

/// Which site and URL a key belongs to, for purging by site or path prefix.
#[derive(Clone)]
struct Tag {
    site: Arc<str>,
    uri: Arc<[u8]>,
}

fn push_framed(out: &mut Vec<u8>, part: &[u8]) {
    out.extend_from_slice(&(part.len() as u32).to_le_bytes());
    out.extend_from_slice(part);
}

/// The key of a request: site, scheme, host and target, plus whatever conduit adds to the
/// upstream request that tells clients apart (an authenticated user, a client certificate).
pub fn key(site: &Arc<str>, scheme: &str, host: &str, uri: &[u8], identity: &[&[u8]]) -> CacheKey {
    let mut primary = Vec::with_capacity(32 + site.len() + host.len() + uri.len());
    for part in [site.as_bytes(), scheme.as_bytes(), host.as_bytes(), uri] {
        push_framed(&mut primary, part);
    }
    for part in identity {
        push_framed(&mut primary, part);
    }
    let mut key = CacheKey::new(primary, "");
    key.extensions.insert(Tag {
        site: site.clone(),
        uri: Arc::from(uri),
    });
    key
}

/// The outcome for the access log, metrics and `Cache-Status`.
pub fn outcome(phase: CachePhase, eligible: bool) -> (&'static str, &'static str) {
    match phase {
        CachePhase::Hit => ("hit", "conduit; hit"),
        CachePhase::Stale | CachePhase::StaleUpdating => ("stale", "conduit; hit; detail=stale"),
        CachePhase::Revalidated => ("revalidated", "conduit; fwd=stale; fwd-status=304"),
        CachePhase::Miss => ("miss", "conduit; fwd=miss; stored"),
        CachePhase::Expired => ("miss", "conduit; fwd=stale; stored"),
        _ if !eligible => ("bypass", "conduit; fwd=bypass"),
        _ => ("uncacheable", "conduit; fwd=miss"),
    }
}

pub const RESULTS: [&str; 6] = [
    "hit",
    "stale",
    "revalidated",
    "miss",
    "uncacheable",
    "bypass",
];

pub fn result_index(result: &str) -> usize {
    RESULTS.iter().position(|r| *r == result).unwrap_or(5)
}

// ---- shared state ----

pub static STORE: LazyLock<Store> = LazyLock::new(Store::new);
pub static EVICTION: LazyLock<lru::Manager<SHARDS>> = LazyLock::new(|| {
    lru::Manager::with_capacity(
        CacheSection::default().max_size.0.unwrap_or(0) as usize,
        1024,
    )
});
pub static PREDICTOR: LazyLock<Predictor<SHARDS>> = LazyLock::new(|| Predictor::new(1024, None));
/// Set once, before the first request: the lock keeps the age timeout it was built with.
static LOCK_AGE: OnceLock<Duration> = OnceLock::new();
static LOCK: LazyLock<CacheLock> = LazyLock::new(|| {
    CacheLock::new(*LOCK_AGE.get_or_init(|| CacheSection::default().lock_timeout.0))
});

pub fn lock() -> &'static CacheKeyLockImpl {
    &*LOCK
}

/// Apply the `cache:` section: at start and after each reload.
pub fn apply(c: &CacheSection) {
    let _ = LOCK_AGE.set(c.lock_timeout.0);
    // 0 means no limit.
    EVICTION.set_weight_limit(c.max_size.0.map_or(usize::MAX, |n| n as usize));
}

pub struct Stats {
    pub objects: usize,
    pub bytes: usize,
    pub evicted: usize,
}

pub fn stats() -> Stats {
    Stats {
        objects: STORE.objects.load(Relaxed),
        bytes: STORE.bytes.load(Relaxed),
        evicted: EVICTION.evicted_items(),
    }
}

// ---- storage ----

struct Object {
    id: u64,
    compact: CompactCacheKey,
    meta: (Vec<u8>, Vec<u8>),
    body: Bytes,
    weight: usize,
    tag: Option<Tag>,
}

impl Object {
    fn entry(&self) -> CacheEntryKeyRef<'_> {
        CacheEntryKeyRef::from_entry_id(&self.compact, Some(CacheEntryId::new(self.id)))
    }
}

type Shard = Mutex<HashMap<HashBinary, Arc<Object>>>;

pub struct Store {
    shards: Box<[Shard]>,
    next_id: AtomicU64,
    bytes: AtomicUsize,
    objects: AtomicUsize,
}

fn lock_shard(s: &Shard) -> std::sync::MutexGuard<'_, HashMap<HashBinary, Arc<Object>>> {
    s.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Store {
    fn new() -> Store {
        Store {
            shards: (0..SHARDS).map(|_| Mutex::new(HashMap::new())).collect(),
            next_id: AtomicU64::new(1),
            bytes: AtomicUsize::new(0),
            objects: AtomicUsize::new(0),
        }
    }

    fn shard(&self, k: &HashBinary) -> &Shard {
        &self.shards[k[0] as usize % SHARDS]
    }

    fn get(&self, k: &HashBinary) -> Option<Arc<Object>> {
        lock_shard(self.shard(k)).get(k).cloned()
    }

    fn forget(&self, obj: &Object, tell_eviction: bool) {
        self.bytes.fetch_sub(obj.weight, Relaxed);
        self.objects.fetch_sub(1, Relaxed);
        if tell_eviction {
            EVICTION.remove(obj.entry());
        }
    }

    fn insert(&self, k: HashBinary, obj: Arc<Object>) {
        self.bytes.fetch_add(obj.weight, Relaxed);
        self.objects.fetch_add(1, Relaxed);
        let old = lock_shard(self.shard(&k)).insert(k, obj);
        if let Some(old) = old {
            self.forget(&old, true);
        }
    }

    /// Replace the metadata of an entry, keeping its body and identity.
    fn rewrite(
        &self,
        k: &HashBinary,
        f: impl FnOnce(&Object) -> Result<(Vec<u8>, Vec<u8>)>,
    ) -> Result<bool> {
        let mut shard = lock_shard(self.shard(k));
        let Some(old) = shard.get(k) else {
            return Ok(false);
        };
        let meta = f(old)?;
        let weight = old.weight - old.meta.0.len() - old.meta.1.len() + meta.0.len() + meta.1.len();
        let new = Arc::new(Object {
            id: old.id,
            compact: old.compact.clone(),
            meta,
            body: old.body.clone(),
            weight,
            tag: old.tag.clone(),
        });
        self.bytes.fetch_add(weight, Relaxed);
        self.bytes.fetch_sub(old.weight, Relaxed);
        shard.insert(*k, new);
        Ok(true)
    }

    /// Drop every entry `f` selects; returns how many.
    fn purge_where(&self, f: impl Fn(&Object) -> bool) -> usize {
        let mut n = 0;
        for s in self.shards.iter() {
            let gone: Vec<Arc<Object>> = {
                let mut map = lock_shard(s);
                let keys: Vec<HashBinary> =
                    map.iter().filter(|(_, o)| f(o)).map(|(k, _)| *k).collect();
                keys.iter().filter_map(|k| map.remove(k)).collect()
            };
            for o in &gone {
                self.forget(o, true);
            }
            n += gone.len();
        }
        n
    }
}

/// Drop a site's entries, all of them or those whose target starts with `prefix`.
pub fn purge(site: &str, prefix: Option<&[u8]>) -> usize {
    STORE.purge_where(|o| {
        o.tag
            .as_ref()
            .is_some_and(|t| &*t.site == site && prefix.is_none_or(|p| t.uri.starts_with(p)))
    })
}

/// Drop the entries of sites that no longer cache (removed, or `cache:` taken out).
pub fn retain_sites(caching: &[&str]) -> usize {
    STORE.purge_where(|o| o.tag.as_ref().is_none_or(|t| !caching.contains(&&*t.site)))
}

#[async_trait]
impl Storage for Store {
    async fn lookup(
        &'static self,
        key: &CacheKey,
        _trace: &SpanHandle,
    ) -> Result<Option<(CacheMeta, HitHandler)>> {
        let Some(obj) = self.get(&key.combined_bin()) else {
            return Ok(None);
        };
        let meta = CacheMeta::deserialize(&obj.meta.0, &obj.meta.1)?;
        let len = obj.body.len();
        Ok(Some((
            meta,
            Box::new(Hit {
                body: obj.body.clone(),
                start: 0,
                end: len,
                done: false,
                weight: obj.weight,
                id: obj.id,
            }),
        )))
    }

    async fn get_miss_handler(
        &'static self,
        key: &CacheKey,
        meta: &CacheMeta,
        _trace: &SpanHandle,
    ) -> Result<MissHandler> {
        let declared = meta
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        Ok(Box::new(Miss {
            store: self,
            key: key.combined_bin(),
            compact: key.to_compact(),
            id: self.next_id.fetch_add(1, Relaxed),
            meta: meta.serialize()?,
            body: Vec::with_capacity(declared.min(16 << 20)),
            tag: key.extensions.get::<Tag>().cloned(),
        }))
    }

    async fn purge(
        &'static self,
        target: PurgeTarget<'_>,
        purge_type: PurgeType,
        _trace: &SpanHandle,
    ) -> Result<PurgeOutcome> {
        let k = target.key().combined_bin();
        let removed = {
            let mut shard = lock_shard(self.shard(&k));
            let matches = shard.get(&k).is_some_and(|o| match target {
                PurgeTarget::Active(_) => true,
                PurgeTarget::Exact(e) => e.entry_id() == Some(CacheEntryId::new(o.id)),
            });
            if matches { shard.remove(&k) } else { None }
        };
        Ok(match removed {
            None => PurgeOutcome::NotFound,
            Some(o) => {
                // Evictions come from the eviction manager, which already let go of the entry.
                self.forget(&o, matches!(purge_type, PurgeType::Invalidation));
                PurgeOutcome::Purged(Some(CacheEntryId::new(o.id)))
            }
        })
    }

    async fn expire(
        &'static self,
        target: PurgeTarget<'_>,
        _trace: &SpanHandle,
    ) -> Result<PurgeOutcome> {
        let k = target.key().combined_bin();
        let done = self.rewrite(&k, |o| {
            let mut meta = CacheMeta::deserialize(&o.meta.0, &o.meta.1)?;
            meta.expire_at(SystemTime::now());
            meta.serialize()
        })?;
        Ok(if done {
            PurgeOutcome::Expired
        } else {
            PurgeOutcome::NotFound
        })
    }

    async fn update_meta(
        &'static self,
        key: &CacheKey,
        meta: &CacheMeta,
        _trace: &SpanHandle,
    ) -> Result<bool> {
        let serialized = meta.serialize()?;
        self.rewrite(&key.combined_bin(), |_| Ok(serialized))
    }

    fn as_any(&self) -> &(dyn std::any::Any + Send + Sync + 'static) {
        self
    }
}

struct Hit {
    body: Bytes,
    start: usize,
    end: usize,
    done: bool,
    weight: usize,
    id: u64,
}

#[async_trait]
impl HandleHit for Hit {
    async fn read_body(&mut self) -> Result<Option<Bytes>> {
        if self.done || self.start >= self.end {
            return Ok(None);
        }
        self.done = true;
        Ok(Some(self.body.slice(self.start..self.end)))
    }

    async fn finish(
        self: Box<Self>,
        _storage: &'static (dyn Storage + Sync),
        _key: &CacheKey,
        _trace: &SpanHandle,
    ) -> Result<()> {
        Ok(())
    }

    fn can_seek(&self) -> bool {
        true
    }

    fn seek(&mut self, start: usize, end: Option<usize>) -> Result<()> {
        let len = self.body.len();
        if start >= len {
            return Error::e_explain(
                ErrorType::InternalError,
                format!("seek start {start} beyond body of {len}"),
            );
        }
        self.start = start;
        self.end = end.map_or(len, |e| e.min(len));
        self.done = false;
        Ok(())
    }

    fn get_eviction_weight(&self) -> usize {
        self.weight
    }

    fn entry_id(&self) -> Option<CacheEntryId> {
        Some(CacheEntryId::new(self.id))
    }

    fn as_any(&self) -> &(dyn std::any::Any + Send + Sync) {
        self
    }

    fn as_any_mut(&mut self) -> &mut (dyn std::any::Any + Send + Sync) {
        self
    }
}

struct Miss {
    store: &'static Store,
    key: HashBinary,
    compact: CompactCacheKey,
    id: u64,
    meta: (Vec<u8>, Vec<u8>),
    body: Vec<u8>,
    tag: Option<Tag>,
}

#[async_trait]
impl HandleMiss for Miss {
    async fn write_body(&mut self, data: Bytes, _eof: bool) -> Result<()> {
        self.body.extend_from_slice(&data);
        Ok(())
    }

    async fn finish(self: Box<Self>) -> Result<MissFinishType> {
        let mut body = self.body;
        body.shrink_to_fit();
        let weight = body.len() + self.meta.0.len() + self.meta.1.len() + ENTRY_OVERHEAD;
        self.store.insert(
            self.key,
            Arc::new(Object {
                id: self.id,
                compact: self.compact,
                meta: self.meta,
                body: Bytes::from(body),
                weight,
                tag: self.tag,
            }),
        );
        Ok(MissFinishType::Created(weight))
    }

    fn entry_id(&self) -> Option<CacheEntryId> {
        Some(CacheEntryId::new(self.id))
    }
}

/// For tests: the cache entry key Pingora's eviction manager would track for an object.
#[cfg(test)]
fn tracked(key: &CacheKey, id: u64) -> pingora::cache::CacheEntryKey {
    pingora::cache::CacheEntryKey::from_entry_id(key.to_compact(), Some(CacheEntryId::new(id)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MatchConfig;

    fn resp(status: u16, headers: &[(&str, &str)]) -> ResponseHeader {
        let mut r = ResponseHeader::build(status, None).unwrap();
        for (k, v) in headers {
            r.append_header(k.to_string(), *v).unwrap();
        }
        r
    }

    fn req(headers: &[(&str, &str)]) -> RequestHeader {
        let mut r = RequestHeader::build("GET", b"/x", None).unwrap();
        for (k, v) in headers {
            r.append_header(k.to_string(), *v).unwrap();
        }
        r
    }

    fn site(default_ttl: Option<u64>) -> SiteCache {
        SiteCache::compile(&SiteCacheConfig {
            matcher: None,
            default_ttl: default_ttl.map(|s| crate::units::Dur(Duration::from_secs(s))),
            stale_if_error: None,
        })
        .unwrap()
    }

    fn stored(r: RespCacheable) -> Option<Duration> {
        match r {
            RespCacheable::Cacheable(m) => Some(
                m.fresh_until()
                    .duration_since(SystemTime::now())
                    .unwrap_or_default(),
            ),
            RespCacheable::Uncacheable(_) => None,
        }
    }

    #[test]
    fn what_is_stored() {
        let s = site(None);
        let plain = req(&[]);
        let ttl = stored(s.cacheable(&plain, &resp(200, &[("cache-control", "max-age=60")])));
        assert!(ttl.is_some_and(|t| t > Duration::from_secs(55)));
        assert!(
            stored(s.cacheable(
                &plain,
                &resp(200, &[("cache-control", "s-maxage=60, max-age=0")])
            ))
            .is_some()
        );
        // Nothing stated: not stored without default_ttl.
        assert!(stored(s.cacheable(&plain, &resp(200, &[]))).is_none());
        for no in [
            &[("cache-control", "private, max-age=60")][..],
            &[("cache-control", "no-store")],
            &[("cache-control", "max-age=60"), ("set-cookie", "a=1")],
            &[("cache-control", "max-age=60"), ("vary", "Accept, *")],
        ] {
            assert!(
                stored(s.cacheable(&plain, &resp(200, no))).is_none(),
                "{no:?}"
            );
        }
        assert!(
            stored(s.cacheable(&plain, &resp(206, &[("cache-control", "max-age=60")]))).is_none()
        );
        // Authorization: only when the response allows it for shared caches.
        let auth = req(&[("authorization", "Basic eDp5")]);
        assert!(
            stored(s.cacheable(&auth, &resp(200, &[("cache-control", "max-age=60")]))).is_none()
        );
        assert!(
            stored(s.cacheable(
                &auth,
                &resp(200, &[("cache-control", "public, max-age=60")])
            ))
            .is_some()
        );

        let d = site(Some(600));
        let ttl = stored(d.cacheable(&plain, &resp(200, &[])));
        assert!(ttl.is_some_and(|t| t > Duration::from_secs(590)), "{ttl:?}");
        assert!(stored(d.cacheable(&plain, &resp(404, &[]))).is_some());
        // default_ttl never overrides what the response says, nor applies to 5xx or auth.
        assert!(
            stored(d.cacheable(&plain, &resp(200, &[("cache-control", "no-store")]))).is_none()
        );
        assert!(stored(d.cacheable(&plain, &resp(500, &[]))).is_none());
        assert!(stored(d.cacheable(&auth, &resp(200, &[]))).is_none());
        let short = stored(d.cacheable(&plain, &resp(200, &[("cache-control", "max-age=5")])));
        assert!(short.is_some_and(|t| t <= Duration::from_secs(5)));
    }

    #[test]
    fn eligibility() {
        let m = SiteCache::compile(&SiteCacheConfig {
            matcher: Some(MatchConfig {
                path: Some(crate::config::StrMatch::Prefix("/assets/".into())),
                ..Default::default()
            }),
            ..Default::default()
        })
        .unwrap();
        let view = |method: &'static http::Method, path: &'static [u8]| RequestView {
            method,
            https: true,
            host: "a",
            client: None,
            path,
            uri: path,
            query: b"",
            headers: Box::leak(Box::new(http::HeaderMap::new())),
        };
        let r = req(&[]);
        assert!(m.takes(&view(&http::Method::GET, b"/assets/a.js"), &r));
        assert!(m.takes(&view(&http::Method::HEAD, b"/assets/a.js"), &r));
        assert!(!m.takes(&view(&http::Method::POST, b"/assets/a.js"), &r));
        assert!(!m.takes(&view(&http::Method::GET, b"/api"), &r));
        let ws = req(&[("upgrade", "websocket")]);
        assert!(!m.takes(&view(&http::Method::GET, b"/assets/a.js"), &ws));
    }

    #[test]
    fn keys_and_variants() {
        let site: Arc<str> = Arc::from("s");
        let a = key(&site, "https", "a.test", b"/x?y=1", &[]);
        assert_eq!(
            a.combined_bin(),
            key(&site, "https", "a.test", b"/x?y=1", &[]).combined_bin()
        );
        for other in [
            key(&site, "http", "a.test", b"/x?y=1", &[]),
            key(&site, "https", "b.test", b"/x?y=1", &[]),
            key(&site, "https", "a.test", b"/x?y=2", &[]),
            key(&site, "https", "a.test", b"/x?y=1", &[b"alice"]),
            // Framing: moving bytes between parts changes the key.
            key(&site, "https", "a.tes", b"t/x?y=1", &[]),
        ] {
            assert_ne!(a.combined_bin(), other.combined_bin());
        }

        let meta = |headers: &[(&str, &str)]| {
            let now = SystemTime::now();
            CacheMeta::new(now, now, 0, 0, resp(200, headers))
        };
        let plain = meta(&[]);
        assert_eq!(variance(&plain, &req(&[])), None);
        let gz = meta(&[("content-encoding", "gzip")]);
        let v1 = variance(&gz, &req(&[("accept-encoding", "gzip, br")]));
        let v2 = variance(&gz, &req(&[("accept-encoding", "br,gzip;q=0.8")]));
        let v3 = variance(&gz, &req(&[]));
        let v4 = variance(&gz, &req(&[("accept-encoding", "gzip")]));
        let v5 = variance(&gz, &req(&[("accept-encoding", "br, gzip;q=0")]));
        assert!(v1.is_some());
        assert_eq!(v1, v2);
        // A gzip body: what else the client takes does not matter.
        assert_eq!(v1, v4);
        assert_ne!(v1, v3);
        assert_eq!(v3, v5);
        // With Vary: Accept-Encoding the upstream chose from the whole list.
        let listed = meta(&[("content-encoding", "gzip"), ("vary", "Accept-Encoding")]);
        let gzip_only = variance(&listed, &req(&[("accept-encoding", "gzip")]));
        assert_ne!(
            variance(&listed, &req(&[("accept-encoding", "gzip, br")])),
            gzip_only
        );
        assert_eq!(
            variance(&listed, &req(&[("accept-encoding", "GZIP")])),
            gzip_only
        );
        let lang = meta(&[("vary", "Accept-Language")]);
        assert_ne!(
            variance(&lang, &req(&[("accept-language", "id")])),
            variance(&lang, &req(&[("accept-language", "en")]))
        );
    }

    #[tokio::test]
    async fn storage_round_trip() {
        let span = pingora::cache::trace::Span::inactive();
        let trace = span.handle();
        let trace = &trace;
        let site: Arc<str> = Arc::from("storage-test");
        let k = key(&site, "https", "s.test", b"/obj", &[]);
        let now = SystemTime::now();
        let m = CacheMeta::new(
            now + Duration::from_secs(60),
            now,
            0,
            0,
            resp(200, &[("content-length", "11")]),
        );
        let store: &'static Store = &STORE;
        let before = stats();
        let mut miss = store.get_miss_handler(&k, &m, trace).await.unwrap();
        let id = miss.entry_id().unwrap();
        miss.write_body(Bytes::from_static(b"hello "), false)
            .await
            .unwrap();
        miss.write_body(Bytes::from_static(b"world"), true)
            .await
            .unwrap();
        let MissFinishType::Created(weight) = miss.finish().await.unwrap() else {
            panic!()
        };
        assert!(weight > 11);
        assert_eq!(stats().objects, before.objects + 1);
        let (meta, mut hit) = store.lookup(&k, trace).await.unwrap().unwrap();
        assert!(meta.is_fresh(SystemTime::now()));
        assert_eq!(hit.entry_id(), Some(id));
        assert_eq!(&hit.read_body().await.unwrap().unwrap()[..], b"hello world");
        assert!(hit.read_body().await.unwrap().is_none());
        hit.seek(6, Some(9)).unwrap();
        assert_eq!(&hit.read_body().await.unwrap().unwrap()[..], b"wor");
        assert!(hit.seek(11, None).is_err());

        // A stale eviction target (an older id) does not remove the current entry.
        let old = tracked(&k, id.get() + 1000);
        let out = store
            .purge(PurgeTarget::Exact(&old), PurgeType::Eviction, trace)
            .await
            .unwrap();
        assert_eq!(out, PurgeOutcome::NotFound);
        assert_eq!(
            store
                .expire(PurgeTarget::Active(&k.to_compact()), trace)
                .await
                .unwrap(),
            PurgeOutcome::Expired
        );
        let (meta, _) = store.lookup(&k, trace).await.unwrap().unwrap();
        assert!(!meta.is_fresh(SystemTime::now()));

        // Purge by site and prefix.
        assert_eq!(purge("storage-test", Some(b"/other")), 0);
        assert_eq!(purge("storage-test", Some(b"/ob")), 1);
        assert!(store.lookup(&k, trace).await.unwrap().is_none());
        assert_eq!(stats().objects, before.objects);
    }
}
