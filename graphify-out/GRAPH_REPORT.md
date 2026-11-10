# Graph Report - conduit  (2026-11-11)

## Corpus Check
- 83 files · ~150,166 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 14 file(s) not represented in the graph (top: (none) 9, .ico 1, .logrotate 1)

## Summary
- 2718 nodes · 6147 edges · 174 communities (121 shown, 53 thin omitted)
- Extraction: 97% EXTRACTED · 3% INFERRED · 0% AMBIGUOUS · INFERRED: 206 edges (avg confidence: 0.82)
- Token usage: host-agent semantic update not measured; AST extraction is local.

## Graph Freshness
- Built from commit: `1fa00208`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- fuzzing.rs
- Forward and Basic Auth
- src/config.rs
- Upstream Passive Ejection
- End-to-End Tests
- Proxy Response Status and Headers
- Proxy Features and Architecture
- Audit Evidence and Documentation
- ACME Key and CSR
- Per-site Plugin Chain
- Pingora Fork and Stability Patches
- HTTP2 Streaming and Tests
- limits.rs
- Runtime Site Compilation
- Proxy Session Hooks
- Body Replace Plugin
- TLS OpenSSL Bindings
- Rule Actions and Header Templates
- Cache Storage Globals
- BytesMatch
- HTTP2 Streaming and Tests
- Site
- Upstream Passive Ejection
- Value Formats Docs
- Proxy Comparison Script
- Access Log File IO
- CertStore
- Prometheus Metrics Registry
- read_all
- CrowdSec Client
- Proxy Comparison Script
- Main Entry and Listeners
- UpstreamRef
- Decision
- Block Exploits Detector
- grpc.py
- files.rs
- health_check
- Example Global Configuration
- CrowdSec Client
- Config Reload Watcher
- Configuration Reference and nginx Mapping
- Body Replace Plugin
- Example Plugin Settings
- Configuration Reference and nginx Mapping
- grpc.py
- Net
- Exploit Rule Categories
- Proxy Features and Architecture
- Historical Runtime Recheck
- Cache Store and Purge
- Access Log Writer Shards
- Exploit Scan Engine
- Brand Assets
- WebSocket Soak Client
- Cacheability and Variants
- Parity Script
- Configuration Reference and nginx Mapping
- UpstreamRef
- HTTP2 Pool Protocol Selection
- Proxy Features and Architecture
- Static File Lookup and Listings
- CrowdSec Stream Deserializer
- Stability Tests and Soak Evidence
- Trusted Client IP Resolution
- Example Site Settings
- audit/README.md
- Admin API
- Proxy Features and Architecture
- storage_round_trip
- Example Files and Basic Auth
- Configuration Reference and nginx Mapping
- Historical Static Audit
- Proxy Response Status and Headers
- Conduit management and Web UI roadmap
- Cache Hit Body Handling
- Access Log Entry Encoding
- Static File Lookup and Listings
- HTTP2 Pool Protocol Selection
- Configuration Reference and nginx Mapping
- Static Files and Response Compression
- RFC 3339 Time Formatting
- mTLS E2E Tests
- Mixed Traffic Soak Harness
- ACME Key and CSR
- test_grpc.py
- Block Exploits Plugin
- CertResolver
- ACME Key and CSR
- Parity Script
- Configuration Reference and nginx Mapping
- Proxy Features and Architecture
- ACME Key and CSR
- Example TLS and ACME
- Example CrowdSec and AppSec
- Parity Stub Backend
- Throughput Benchmark Harness
- Static File Configuration Tests
- Cache Storage Round Trip
- README.md
- Table
- Block Exploits Plugin
- Proxy Features and Architecture
- Example Defaults
- HTTP/2 upstream: implementasi dan validasi
- Documentation and Roadmap Links
- Table
- CrowdSec IP Bit Masks
- files.rs
- HTTP2 Streaming and Tests
- HTTP2_UPSTREAM_VALIDATION.md
- Alternating Binary Comparison Harness
- grpc.py
- Access Log Fuzz RNG
- Static File Lookup and Listings
- CrowdSec AppSec Acquisition
- Stability Tests and Soak Evidence
- Exploit Scan Engine
- plan
- End-to-End Tests
- End-to-End Tests
- upstream
- Cacheability and Variants
- Post-fix Validation Memory
- Regression Test Helpers
- Regression Test Helpers
- PGO Training Script
- Native release soak on a disposable host
- Shared
- real_ip
- CrowdSec Log Acquisition
- Audit Evidence and Documentation
- Audit Origin Fixtures
- plan
- Main Entry and Listeners
- PGO Build Script
- Access Log File IO
- Fuzz Runner Script
- Cargo Crates
- Release Build Script
- deploy.sh script

## God Nodes (most connected - your core abstractions)
1. `Pingora fork (vendor/pingora, branch conduit)` - 48 edges
2. `RequestView` - 40 edges
3. `Local` - 39 edges
4. `Pool` - 37 edges
5. `http()` - 35 edges
6. `Ctx` - 34 edges
7. `Shared` - 33 edges
8. `test_dir()` - 33 edges
9. `Dur` - 29 edges
10. `wait_for_port()` - 29 edges

## Surprising Connections (you probably didn't know these)
- `F07: bukan asumsi tentang internal sketch` --references--> `request()`  [INFERRED]
  graphify-out/memory/query_20261110_083252_422cf94f_cek_lagi___apakah_temuan_itu_bener_bener_bug_atau.md → docs/audit/recheck_runtime.py
- `Matriks keputusan akhir` --references--> `request()`  [INFERRED]
  graphify-out/memory/query_20261110_083252_422cf94f_cek_lagi___apakah_temuan_itu_bener_bener_bug_atau.md → docs/audit/recheck_runtime.py
- `One backend configuration contract` --conceptually_related_to--> `Runtime`  [INFERRED]
  docs/roadmap.md → src/runtime.rs
- `Planned management authentication and roles` --conceptually_related_to--> `Admin`  [INFERRED]
  docs/roadmap.md → src/admin.rs
- `c4cce19: task_pipe HTTP/1 task channel` --semantically_similar_to--> `JSON access log (per-worker buffers)`  [INFERRED] [semantically similar]
  vendor/README.md → docs/technical.md

## Import Cycles
- 2-file cycle: `src/limits.rs -> src/plugin.rs -> src/limits.rs`
- 2-file cycle: `src/plugin.rs -> src/replace.rs -> src/plugin.rs`
- 2-file cycle: `src/metrics.rs -> src/runtime.rs -> src/metrics.rs`
- 2-file cycle: `src/crowdsec.rs -> src/plugin.rs -> src/crowdsec.rs`
- 2-file cycle: `src/proxy.rs -> src/tls.rs -> src/proxy.rs`
- 2-file cycle: `src/proxy.rs -> src/upstream.rs -> src/proxy.rs`
- 3-file cycle: `src/limits.rs -> src/plugin.rs -> src/replace.rs -> src/limits.rs`
- 3-file cycle: `src/proxy.rs -> src/rules.rs -> src/upstream.rs -> src/proxy.rs`
- 3-file cycle: `src/metrics.rs -> src/runtime.rs -> src/upstream.rs -> src/metrics.rs`
- 3-file cycle: `src/crowdsec.rs -> src/metrics.rs -> src/runtime.rs -> src/crowdsec.rs`
- 3-file cycle: `src/metrics.rs -> src/tls.rs -> src/proxy.rs -> src/metrics.rs`
- 3-file cycle: `src/proxy.rs -> src/runtime.rs -> src/tls.rs -> src/proxy.rs`
- 3-file cycle: `src/proxy.rs -> src/runtime.rs -> src/upstream.rs -> src/proxy.rs`
- 3-file cycle: `src/crowdsec.rs -> src/proxy.rs -> src/runtime.rs -> src/crowdsec.rs`
- 3-file cycle: `src/plugin.rs -> src/upstream.rs -> src/proxy.rs -> src/plugin.rs`
- 3-file cycle: `src/crowdsec.rs -> src/proxy.rs -> src/plugin.rs -> src/crowdsec.rs`
- 4-file cycle: `src/limits.rs -> src/metrics.rs -> src/runtime.rs -> src/plugin.rs -> src/limits.rs`
- 4-file cycle: `src/proxy.rs -> src/rules.rs -> src/route.rs -> src/upstream.rs -> src/proxy.rs`
- 4-file cycle: `src/plugin.rs -> src/rules.rs -> src/upstream.rs -> src/proxy.rs -> src/plugin.rs`
- 4-file cycle: `src/files.rs -> src/rules.rs -> src/upstream.rs -> src/proxy.rs -> src/files.rs`

## Hyperedges (group relationships)
- **Planned consistent management changes** — docs_roadmap_shared_configuration_contract, docs_roadmap_revision_conflict_control, docs_roadmap_yaml_write_ownership, docs_roadmap_durable_configuration_apply [EXTRACTED 1.00]
- **Verified fixes and preserved documented behavior** — docs_audit_fix_validation, docs_audit_fix_validation_f01, docs_audit_fix_validation_f02, docs_audit_fix_validation_f03, docs_audit_fix_validation_f04, docs_audit_fix_validation_f05, docs_audit_fix_validation_f06, docs_audit_fix_validation_f07, docs_audit_fix_validation_f08, docs_audit_fix_validation_f09, docs_audit_fix_validation_f10 [EXTRACTED 1.00]
- **Inputs to the cache key that prevent cache poisoning** — docs_configuration_cache_key, docs_configuration_forward_auth, docs_configuration_mtls, docs_configuration_trusted_proxy_headers, docs_configuration_proxy_action, docs_technical_body_replace [EXTRACTED 1.00]
- **Plugins that can answer (end) the request** — docs_configuration_rules_plugin, docs_technical_ip_filter, docs_configuration_crowdsec_plugin, docs_configuration_rate_limit, docs_configuration_concurrency, docs_technical_basic_auth, docs_configuration_forward_auth, docs_technical_block_exploits [EXTRACTED 1.00]
- **Phase three core stability validation** — docs_technical_upstream_flapping_test, docs_technical_soak_24h_results, docs_technical_fuzzing, docs_technical_panic_audit, docs_technical_open_websockets_upgrade_test, docs_technical_parity_test [EXTRACTED 1.00]
- **Soak harness evidence and failure attribution** — docs_technical_soak_events, docs_technical_soak_samples, docs_technical_soak_report, docs_technical_soak_archives, docs_technical_soak_upgrade_failure_guard [EXTRACTED 1.00]
- **Pingora ten performance patches** — vendor_readme_patch_79bf391, vendor_readme_patch_32d98ca, vendor_readme_patch_ecef83b, vendor_readme_patch_9586c10, vendor_readme_patch_c448eff, vendor_readme_patch_9c64421, vendor_readme_patch_3357c9e, vendor_readme_patch_bb3859d, vendor_readme_patch_c4cce19, vendor_readme_patch_0a6aee4 [EXTRACTED 1.00]
- **Backend failure handling (selection, retries, ejection, health checks)** — docs_configuration_backend_selection, docs_configuration_retries, docs_configuration_passive_ejection, docs_configuration_active_health_checks, docs_configuration_upstream_pools [INFERRED 0.85]

## Communities (174 total, 53 thin omitted)

### Community 0 - "fuzzing.rs"
Cohesion: 0.05
Nodes (33): F01: Single percent decoding policy and file lookup, BodyReplaceConfig, ReplacePair, access_log(), config(), ip_backends_only(), path(), route() (+25 more)

### Community 1 - "Forward and Basic Auth"
Cohesion: 0.07
Nodes (34): basic(), basic_auth_flow(), basic_auth_settings(), BasicAuth, cache_key(), CACHE_KEY, check(), config() (+26 more)

### Community 2 - "src/config.rs"
Cohesion: 0.05
Nodes (33): AccessLogSection, acme_section(), AcmeSection, AppSecSection, CacheSection, CrowdSecSection, default_h2_streams(), Defaults (+25 more)

### Community 3 - "Upstream Passive Ejection"
Cohesion: 0.06
Nodes (29): Balance, Hash, LeastConn, Random, RoundRobin, PoolConfig, EPOCH, hashing_is_sticky() (+21 more)

### Community 4 - "End-to-End Tests"
Cohesion: 0.06
Nodes (41): Answer, auth_service(), body_exchange(), body_upstream(), BOUNCER_KEY, cache_upstream(), Counts, CROWDSEC_TAG (+33 more)

### Community 5 - "Proxy Response Status and Headers"
Cohesion: 0.06
Nodes (27): AdminSection, ListenSection, ACME_PREFIX, BASE, CACHE_STATUS, each_worker_thread_has_its_own_pool_group(), header_str(), ip_text() (+19 more)

### Community 6 - "Proxy Features and Architecture"
Cohesion: 0.06
Nodes (37): scripts/ab.sh A/B harness, Apache httpd, Authentication tests, Cross-proxy cgroup CPU and anonymous-memory comparison, block_exploits attack and ordinary-request tests, Stale cache revalidation and stale-if-error, Response-cache tests, Caddy (+29 more)

### Community 7 - "Audit Evidence and Documentation"
Cohesion: 0.05
Nodes (34): F02: Descriptor fallback and pinned directory listing, F03: Cache authority includes the original port, F04: Documented fresh cache retention preserved, F05: Duration multiplication overflow rejected, F06: Zero or unrepresentable watcher interval rejected, F07: Serialized admission for the same limiter key, F09: Explicit Vary preserves unknown content codings, F10: README Server header contract aligned (+26 more)

### Community 8 - "ACME Key and CSR"
Cohesion: 0.06
Nodes (26): Ari, at(), backoff(), backoff_grows_and_caps(), cert_id(), challenge_answer(), challenge_tokens_are_withdrawn(), CHALLENGES (+18 more)

### Community 9 - "Per-site Plugin Chain"
Cohesion: 0.08
Nodes (14): HeaderOps, HeadersConfig, IpFilterConfig, Chain, chain(), HeadersPlugin, ip_filter(), IpFilter (+6 more)

### Community 10 - "Pingora Fork and Stability Patches"
Cohesion: 0.08
Nodes (29): Real-binary end-to-end tests, prepare-vendor.sh script, CasedHeaderName, daemon_wait_for_ready, Docker release build includes vendored Pingora, FlushTrackingMock, H2 backport preparation and native validation, TransportConnector idle connection pool (+21 more)

### Community 11 - "HTTP2 Streaming and Tests"
Cohesion: 0.09
Nodes (25): Native H2 pool race regression, Source and binary validation provenance, Default HTTP/1 compatibility benchmark, HTTP/2 implementation and validation results, Final HTTP/2 release soak, 111 unit and 34 e2e passed, Historical upstream HTTP/1 answer, HTTP/2 feature completion evidence (+17 more)

### Community 12 - "limits.rs"
Cohesion: 0.09
Nodes (16): CARRIED, chain(), client(), Concurrency, concurrency_holds_until_dropped(), Hasher, InFlight, Key (+8 more)

### Community 13 - "Runtime Site Compilation"
Cohesion: 0.13
Nodes (28): Config, SiteConfig, UpgradeMode, Off, Websocket, yaml(), acme_dir(), acme_plan() (+20 more)

### Community 14 - "Proxy Session Hooks"
Cohesion: 0.14
Nodes (13): send(), compressible(), compressible_types(), Conduit, Ctx, decorate(), enable_compression(), final_status() (+5 more)

### Community 15 - "Body Replace Plugin"
Cohesion: 0.11
Nodes (24): ActionConfig, Deny, File, Proxy, Redirect, Respond, ActionRaw, FileConfig (+16 more)

### Community 16 - "TLS OpenSSL Bindings"
Cohesion: 0.09
Nodes (8): ask_client_certs(), ask_for_certificate(), client_hello_sni(), ClientTrust, sni_extension(), sni_from_extension(), SSL_set_session_id_context(), TRUST_INDEX

### Community 17 - "Rule Actions and Header Templates"
Cohesion: 0.11
Nodes (26): RuleConfig, Action, Deny, Proxy, Redirect, Respond, bad_rules_are_rejected(), compile_headers() (+18 more)

### Community 18 - "Cache Storage Globals"
Cohesion: 0.09
Nodes (12): ENTRY_OVERHEAD, EVICTION, heuristic(), LOCK_AGE, LOCK, lock(), no_default_ttl(), outcome() (+4 more)

### Community 19 - "BytesMatch"
Cohesion: 0.10
Nodes (21): BlockExploitsConfig, ExploitException, ExploitMode, Block, Log, HeaderMatch, MatchConfig, StrMatch (+13 more)

### Community 20 - "HTTP2 Streaming and Tests"
Cohesion: 0.24
Nodes (31): Validated audit findings closed, acme_with_pebble(), auth_plugins(), block_exploits(), cpu_time(), crowdsec_bouncer_and_appsec(), echo_upstream(), file_descriptors_run_out() (+23 more)

### Community 21 - "Site"
Cohesion: 0.16
Nodes (27): request(), Effects, Local, Step, Pass, acme_response(), appsec(), check_client_cert() (+19 more)

### Community 22 - "Upstream Passive Ejection"
Cohesion: 0.13
Nodes (7): Lease, LoadBalancer<S>, Member, now_ms(), Pool, Select, Target

### Community 23 - "Value Formats Docs"
Cohesion: 0.07
Nodes (30): 10. Response cache, 12. Headers conduit adds and passes, 13. Protocol upgrades (WebSocket and others), 14. Compression, 15. Responses conduit generates, 16. Access log, 17. Admin endpoints and metrics, 18. Coming from nginx (+22 more)

### Community 24 - "Proxy Comparison Script"
Cohesion: 0.13
Nodes (18): configs(), hcm(), listener(), Container, container_name(), docker_rm(), main(), med() (+10 more)

### Community 25 - "Access Log File IO"
Cohesion: 0.08
Nodes (11): ESCAPE, FLUSH_EVERY, HEX, HIGHS, int(), needs_escape(), ONES, OpenFile (+3 more)

### Community 26 - "CertStore"
Cohesion: 0.12
Nodes (11): F08: Wildcard certificate override and coverage corrected, CertConfig, CertKey, CertResolver, CertStore, client_trust_from_file(), mismatched_key_rejected(), selection_order() (+3 more)

### Community 27 - "Prometheus Metrics Registry"
Cohesion: 0.09
Nodes (8): result_index(), CLASSES, forget_site(), LATENCY_BUCKETS, lazy!, PROCESS, SiteMetrics, status_classes()

### Community 28 - "read_all"
Cohesion: 0.17
Nodes (23): backend_state(), config(), end_to_end(), exchange(), file_server(), graceful_upgrade(), http(), https() (+15 more)

### Community 29 - "CrowdSec Client"
Cohesion: 0.10
Nodes (16): AppSec, appsec_request_shape(), base_url(), CrowdSecPlugin, forwarded(), http_version(), Plan, plan() (+8 more)

### Community 30 - "Proxy Comparison Script"
Cohesion: 0.21
Nodes (12): main(), launch(), cert(), fingerprint(), main(), reloaded(), origin(), port() (+4 more)

### Community 31 - "Main Entry and Listeners"
Cohesion: 0.10
Nodes (8): check(), DEFAULT_CONFIG, GLOBAL, huge_pages(), listener(), main(), run(), socket_options()

### Community 32 - "UpstreamRef"
Cohesion: 0.09
Nodes (16): Upstream HTTP version policy, Upstream protocol behavior, BackendConfig, ServerHeader, Off, Value, UpstreamHttpVersion, Auto (+8 more)

### Community 33 - "Decision"
Cohesion: 0.15
Nodes (11): Decision, Delta, Family, Family<K>, Map, Net, Sink, Slot (+3 more)

### Community 34 - "Block Exploits Detector"
Cohesion: 0.09
Nodes (20): AGENT, Attack, ATTACKS, attacks_are_caught_by_their_rule(), detect(), ENGINE, every_rule_has_an_attack(), H (+12 more)

### Community 36 - "files.rs"
Cohesion: 0.12
Nodes (14): CHUNK, conditionals_and_ranges(), content_type(), content_types(), etag_listed(), FileReply, http_date(), LIST_LIMIT (+6 more)

### Community 37 - "health_check"
Cohesion: 0.09
Nodes (25): url, weight, expect, fall, interval, path, rise, timeout (+17 more)

### Community 38 - "Example Global Configuration"
Cohesion: 0.08
Nodes (23): access_log, path, queue_capacity, admin, listen, cache, lock_timeout, max_object_size (+15 more)

### Community 39 - "CrowdSec Client"
Cohesion: 0.11
Nodes (13): APPSEC_WARNED, BAN_PAGE, EPOCH, LAPI_DEADLINE, LAPI_MAX_RESPONSE, LAPI_TIMEOUT, MAX_BODY, PRUNE_EVERY (+5 more)

### Community 40 - "Config Reload Watcher"
Cohesion: 0.13
Nodes (8): apply(), retain_sites(), load(), record(), Reloader, snapshot(), Stamp, stamp()

### Community 41 - "Configuration Reference and nginx Mapping"
Cohesion: 0.18
Nodes (19): Authelia / Authentik / oauth2-proxy, Freshness, revalidation, stale serving and cache lock, Cache storage rules (RFC 9111, stricter), Cloudflare, `concurrency`, Fixed-size counting sketch, `forward_auth`, Load balancing (round_robin, random, least_conn, ketama hash) (+11 more)

### Community 42 - "Body Replace Plugin"
Cohesion: 0.13
Nodes (10): Held, Gate, Answer, Check, Pass, Replace, Route, Answer (+2 more)

### Community 43 - "Example Plugin Settings"
Cohesion: 0.09
Nodes (22): except, skip_params, replace, types, max, copy_headers, url, response (+14 more)

### Community 44 - "Configuration Reference and nginx Mapping"
Cohesion: 0.10
Nodes (21): description, filter, name, onsuccess, 3. Global sections, 8. The other plugins, `admin`, `basic_auth` (+13 more)

### Community 45 - "grpc.py"
Cohesion: 0.15
Nodes (9): atomic_json(), client(), main(), publish(), serve(), connection(), unpack_frames(), get() (+1 more)

### Community 46 - "Net"
Cohesion: 0.13
Nodes (15): Apply<'_, S>, d(), go_duration_ms(), go_durations(), ip(), lookup_cost(), V4, V6 (+7 more)

### Community 47 - "Exploit Rule Categories"
Cohesion: 0.11
Nodes (16): Category, .ALL, Dotfiles, Php, Rce, Scanners, Spam, Sqli (+8 more)

### Community 49 - "Proxy Features and Architecture"
Cohesion: 0.12
Nodes (14): Built-in ACME client (HTTP-01, ARI), conduit check (config validation), Active HTTP/TCP health checks, Hot reload without dropped connections, Load-balancing algorithms (round_robin, random, least_conn, Ketama hash), Production rollout began in November 2026, RFC 9773 (ACME renewal information), Certificate rotation and reload under load test (+6 more)

### Community 50 - "Historical Runtime Recheck"
Cohesion: 0.10
Nodes (20): Answer, Artefak reproduksi, Bukti per temuan, F01: ada kontrol yang membedakan bug dari konfigurasi terbuka, F02: fallback rentan race, tetapi symlink biasa bukan bypass, F03: miss pertama, hit salah authority, dan control baru, F04: perilaku nyata, tetapi klasifikasi bug ditarik, F05: overflow diuji dengan Rust, termasuk mode teroptimisasi (+12 more)

### Community 51 - "Cache Store and Purge"
Cohesion: 0.25
Nodes (4): lock_shard(), Object, Shard, Store

### Community 52 - "Access Log Writer Shards"
Cohesion: 0.17
Nodes (5): AccessLog, lock(), Shard, Shards, Writer

### Community 53 - "Exploit Scan Engine"
Cohesion: 0.18
Nodes (14): after(), contains(), decode_query(), decoding(), Engine, fuzz_check(), has_escape(), header_suspect() (+6 more)

### Community 54 - "Brand Assets"
Cohesion: 0.15
Nodes (15): conduit-app-icon.svg, Conduit brand assets, Ember palette, favicon.svg / favicon.ico, Geist typeface (SemiBold 600, Regular 400, Mono 400), Lanes mark, conduit-logo-*-ember.svg / conduit-mark-ember.svg, conduit-logo-horizontal.svg (+7 more)

### Community 55 - "WebSocket Soak Client"
Cohesion: 0.25
Nodes (11): client(), shorts(), connect(), echo(), frame(), long_lived(), read_frame(), serve() (+3 more)

### Community 56 - "Cacheability and Variants"
Cohesion: 0.16
Nodes (12): meta, eligibility(), keys_and_variants(), req(), resp(), site(), SiteCache, stored() (+4 more)

### Community 57 - "Parity Script"
Cohesion: 0.14
Nodes (7): fetch(), hosts_from(), main(), run(), read_cases(), causes(), slope()

### Community 58 - "Configuration Reference and nginx Mapping"
Cohesion: 0.21
Nodes (18): 5. Sites, Headers conduit adds to client responses, Response compression (gzip, br, zstd, level 1), conduit check command, `default_site`, defaults section (timeouts, max_body_size, compression), force_https and hsts, headers plugin (request/response set/remove) (+10 more)

### Community 59 - "UpstreamRef"
Cohesion: 0.12
Nodes (16): ConcurrencyConfig, CrowdSecPluginConfig, PluginConfig, BasicAuth, BlockExploits, BodyReplace, Concurrency, CrowdSec (+8 more)

### Community 60 - "HTTP2 Pool Protocol Selection"
Cohesion: 0.14
Nodes (8): Protocol isolation and HTTP/2 health checks, HTTP/2 connection stream limit, Cleartext HTTP/2 prior knowledge, HealthCheckConfig, ProbeType, Http, Tcp, Probe

### Community 61 - "Proxy Features and Architecture"
Cohesion: 0.12
Nodes (16): write(), scripts/bench.sh benchmark and regression gate, Benchmark, Benchmark gate with baseline-fixed rates and p99 tolerances, Comparison with other reverse proxies, CPU profile, Fuzz properties: config, path, access_log, request, route, body_replace, block_exploits, Fuzzing (cargo-fuzz, src/fuzzing.rs) (+8 more)

### Community 62 - "Static File Lookup and Listings"
Cohesion: 0.19
Nodes (14): Files, Found, File, Listing, Missing, Redirect, Status, html_escape() (+6 more)

### Community 64 - "Stability Tests and Soak Evidence"
Cohesion: 0.13
Nodes (17): Daemon readiness smoke (2026-11-10: 255,136 requests, 2 upgrades), Graceful binary upgrade (socket handover), Graceful upgrade test, Nginx Proxy Manager, upgrade_with_open_websockets test, Panic and misbehaving-client audit, Parity test against nginx (Nginx Proxy Manager), 24-hour soak (2026-11-08/09: 56.2 million requests) (+9 more)

### Community 65 - "Trusted Client IP Resolution"
Cohesion: 0.26
Nodes (9): canonical(), Cidr, cidr_contains(), CLOUDFLARE, ip(), RealIp, recursive_skips_trusted_hops(), trusted_peer_header_used() (+1 more)

### Community 66 - "Example Site Settings"
Cohesion: 0.12
Nodes (17): default_ttl, stale_if_error, ca, sites, acme, cache, compression, force_https (+9 more)

### Community 67 - "audit/README.md"
Cohesion: 0.13
Nodes (10): Audit dan validasi Conduit, Batas hasil, Cara membaca laporan, Graphify, Context7 dan provenance, Matriks penutupan, Reproduksi pada checkout baru, Status saat ini, accepted_bits() (+2 more)

### Community 68 - "Admin API"
Cohesion: 0.25
Nodes (6): Admin, bad(), reply(), service(), unescape(), unix()

### Community 69 - "Proxy Features and Architecture"
Cohesion: 0.13
Nodes (11): Commands, conduit: technical documentation, Deferred features (session tickets, OCSP, admin API, OpenTelemetry, captcha, DNS-01), HTTP/1.1 and HTTPS throughput analysis, HTTP/1.1 and HTTPS throughput, Phase 2 (ACME, CrowdSec, limits, mTLS, auth), Phase 1 targets, Roadmap (+3 more)

### Community 70 - "storage_round_trip"
Cohesion: 0.17
Nodes (8): key(), purge(), push_framed(), Stats, stats(), storage_round_trip(), tracked(), render()

### Community 71 - "Example Files and Basic Auth"
Cohesion: 0.13
Nodes (15): htpasswd, match, match, root, strip_prefix, try_files, method, path (+7 more)

### Community 72 - "Configuration Reference and nginx Mapping"
Cohesion: 0.18
Nodes (15): 11. TLS, ACME and client certificates, ACME, certbot, Certificate selection, deny action, file action (in-memory file response), Watched files (mtime, inode, size, follows symlinks), Responses conduit generates (status table, HTML status page) (+7 more)

### Community 73 - "Historical Static Audit"
Cohesion: 0.13
Nodes (14): Answer, Arsitektur yang terbentuk, Audit mendalam Conduit, Cakupan dan batas kejujuran, Cara membaca paket audit, Invarian yang benar-benar penting, Outcome, Pemeriksaan dokumentasi terkini (+6 more)

### Community 74 - "Proxy Response Status and Headers"
Cohesion: 0.27
Nodes (14): acme_challenge(), acme_challenge_from_memory(), answer(), body_limit_and_default_site(), client_certificates(), host_parsing(), https_redirect_runs_before_plugins(), plugins_see_normalized_path_and_client() (+6 more)

### Community 75 - "Conduit management and Web UI roadmap"
Cohesion: 0.14
Nodes (14): Authentication and authorization, Conduit management and Web UI roadmap, Current foundation and missing work, Durable apply, rollback and recovery, Enterprise scope, External YAML edits, Generic OIDC, Authentik and ZITADEL, Lightweight UI and observability (+6 more)

### Community 77 - "Access Log Entry Encoding"
Cohesion: 0.23
Nodes (9): Entry, entry(), full_shard_drops_and_counts(), hostile_values_stay_one_line(), ip(), json_line(), line(), millis() (+1 more)

### Community 78 - "Static File Lookup and Listings"
Cohesion: 0.18
Nodes (5): Accepted, Encoding, Br, Gzip, Zstd

### Community 79 - "HTTP2 Pool Protocol Selection"
Cohesion: 0.22
Nodes (13): 4. Upstream pools, Active health checks, Backend DNS resolution at load time, Backend selection with fallback, Backends and DNS, Balancing, Choosing a backend, Passive ejection (+5 more)

### Community 80 - "Configuration Reference and nginx Mapping"
Cohesion: 0.23
Nodes (12): 6. Match conditions, File responses (ETag, conditionals, ranges, MIME), Header conditions, Path normalization, proxy action (routing and path rewrite), redirect action and placeholders, RFC 3986 URI syntax, RFC 9110 HTTP Semantics (+4 more)

### Community 81 - "Static Files and Response Compression"
Cohesion: 0.15
Nodes (8): Static-file conditional requests and single byte ranges, Static file server (files:), Static file-server tests, rules proxy action (strip_prefix, add_prefix, rewrite), RFC 9110 (HTTP semantics), routes_and_body_replace end-to-end test, rules plugin, Upstream Brotli finalization (b5dea5a, PR 1021, issue 1013)

### Community 82 - "RFC 3339 Time Formatting"
Cohesion: 0.23
Nodes (7): ser_ts(), civil_from_days(), known_instants(), rfc3339(), rfc3339_millis(), seconds_part(), write_rfc3339_millis()

### Community 83 - "mTLS E2E Tests"
Cohesion: 0.31
Nodes (5): Ca, cert_builder(), ec_key(), mtls_get(), MtlsReply

### Community 84 - "Mixed Traffic Soak Harness"
Cohesion: 0.36
Nodes (12): cert(), cleanup(), event(), every(), flap(), keep_log(), probe_protocol(), rotate_log() (+4 more)

### Community 85 - "ACME Key and CSR"
Cohesion: 0.29
Nodes (8): asn1_unix(), bundle(), csr(), csr_names_every_host(), inspect(), issued(), new_key(), stored_bundle_loads_and_identifies()

### Community 87 - "Block Exploits Plugin"
Cohesion: 0.27
Nodes (7): block_and_log_modes(), BlockExploits, detect_with(), Hit, may_log(), plugin(), settings()

### Community 88 - "CertResolver"
Cohesion: 0.18
Nodes (5): ClientStatus, Invalid, Missing, Valid, distinguished_name()

### Community 89 - "ACME Key and CSR"
Cohesion: 0.33
Nodes (5): acme_err(), Connected, Manager, order(), write_private()

### Community 90 - "Parity Script"
Cohesion: 0.31
Nodes (4): category(), main(), make_browser(), run()

### Community 91 - "Configuration Reference and nginx Mapping"
Cohesion: 0.24
Nodes (11): `access_log`, Admin endpoints (/metrics, /healthz, /status, /cache/purge), block_exploits categories and rule ids, Cache purge endpoint, Cache-Status header (RFC 9211), Runtime snapshots on configuration reload, Graceful upgrade (conduit run -d -u), Prometheus metrics (conduit_*) (+3 more)

### Community 92 - "Proxy Features and Architecture"
Cohesion: 0.18
Nodes (8): Cloudflare (CDN in front of the origin), CrowdSec AppSec (WAF) checks, Built-in CrowdSec L7 bouncer, CrowdSec parser for conduits access log (deploy/crowdsec), CrowdSec full-loop tests, Pebble ACME test CA, ACME end-to-end test with Pebble, rate_limit plugin

### Community 94 - "Example TLS and ACME"
Cohesion: 0.20
Nodes (10): directory, email, storage, cert, key, name, tls, acme (+2 more)

### Community 95 - "Example CrowdSec and AppSec"
Cohesion: 0.20
Nodes (10): body_limit, failure_action, timeout, url, crowdsec, api_key_file, appsec, lapi_url (+2 more)

### Community 97 - "Throughput Benchmark Harness"
Cohesion: 0.36
Nodes (8): baseline_rate(), cpu_ticks(), load(), peak_rss_kb(), pids_of(), run(), bench.sh script, wait_port()

### Community 98 - "Static File Configuration Tests"
Cohesion: 0.29
Nodes (8): FilesConfig, body(), Dir, files(), precompressed_try_files_browse_prefix(), serving_and_confinement(), settings_are_checked(), tree()

### Community 100 - "README.md"
Cohesion: 0.22
Nodes (8): Development, Documentation, Features, Getting started, HTTP/2 upstream, License, Performance, Pingora fork

### Community 102 - "Block Exploits Plugin"
Cohesion: 0.28
Nodes (5): all_rules(), categories_mask(), cost(), tautologies(), tautology()

### Community 103 - "Proxy Features and Architecture"
Cohesion: 0.25
Nodes (7): Brand assets in project documentation, Current local validation, server_header end-to-end test, Local test results, 111 application unit tests passed, Final validation and documentation navigation, Project README overview

### Community 104 - "Example Defaults"
Cohesion: 0.25
Nodes (8): defaults, compression, connect_timeout, keepalive_timeout, max_body_size, read_timeout, upstream_idle_timeout, write_timeout

### Community 105 - "HTTP/2 upstream: implementasi dan validasi"
Cohesion: 0.25
Nodes (8): HTTP/2 upstream: implementasi dan validasi, Integritas dan pemetaan, Pemakaian, Perbaikan dependency yang diperlukan, Performa, Perubahan implementasi, Stabilitas H2 upstream, Uji fungsional dan regresi

### Community 106 - "Documentation and Roadmap Links"
Cohesion: 0.25
Nodes (8): 7. The `rules` plugin, Actions, `deny`, Examples, `file`, `proxy`, `redirect`, `respond`

### Community 107 - "Table"
Cohesion: 0.36
Nodes (6): Applied, Apply, List, parse_into(), stream_full_then_delta(), sync()

### Community 108 - "CrowdSec IP Bit Masks"
Cohesion: 0.25
Nodes (6): Bits, .WIDTH, u128, .WIDTH, u32, .WIDTH

### Community 109 - "files.rs"
Cohesion: 0.36
Nodes (4): open_beneath(), read_exact_at(), reply(), show()

### Community 110 - "HTTP2 Streaming and Tests"
Cohesion: 0.29
Nodes (3): CrowdSec, docker(), Pebble

### Community 111 - "HTTP2_UPSTREAM_VALIDATION.md"
Cohesion: 0.29
Nodes (4): Completed older run, Expanded smoke outcome, Krynox soak revalidation — 11 November 2026, Latest release and harness checks

### Community 112 - "Alternating Binary Comparison Harness"
Cohesion: 0.48
Nodes (5): config(), cpu_ticks(), load(), ab.sh script, wait_port()

### Community 115 - "Static File Lookup and Listings"
Cohesion: 0.33
Nodes (4): Candidate, Part, Lit, Path

### Community 116 - "CrowdSec AppSec Acquisition"
Cohesion: 0.33
Nodes (5): appsec_config, labels, type, listen_addr, source

### Community 117 - "Stability Tests and Soak Evidence"
Cohesion: 0.40
Nodes (3): Cache purge endpoint (POST /cache/purge), Prometheus metrics, Abort soak when graceful handover or admin PID verification fails

### Community 118 - "Exploit Scan Engine"
Cohesion: 0.33
Nodes (3): Place, Path, Query

### Community 119 - "plan"
Cohesion: 0.33
Nodes (6): Plan, Full, NotModified, Partial, PreconditionFailed, Unsatisfiable

### Community 122 - "upstream"
Cohesion: 0.40
Nodes (5): upstream, backends, balance, hash_key, verify_tls

### Community 123 - "Cacheability and Variants"
Cohesion: 0.40
Nodes (5): statics, expression, parsed, target, value

### Community 124 - "Post-fix Validation Memory"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: ok languns perbaiki semua temuan ya,, dan setiap temuan fix languns di uji e2e ya,, dan kalo udah semua uji e2e semunaya dan uji stabilitas dan performasnya dan uji sekali algi apakah sduah tertutup, Source Nodes

### Community 125 - "Regression Test Helpers"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: ini conduit dukunga ke upstream ini hanay di http/1 ya? http/2 belum?, Source Nodes

### Community 126 - "Regression Test Helpers"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: loh kerjakan bro,, karena ada tool internal butuh http/2 upstream,, semua fitur di pingora kalo bisa ngapain gk di kerjakan di conduit bro, Source Nodes

### Community 127 - "PGO Training Script"
Cohesion: 0.60
Nodes (3): fetch(), pgo-train.sh script, urls()

### Community 128 - "Native release soak on a disposable host"
Cohesion: 0.40
Nodes (5): Native release soak on a disposable host, Prepare the artifacts, Read results, Smoke before the full run, What is checked

### Community 130 - "real_ip"
Cohesion: 0.50
Nodes (4): real_ip, header, recursive, trusted

### Community 131 - "CrowdSec Log Acquisition"
Cohesion: 0.50
Nodes (3): filenames, labels, type

### Community 132 - "Audit Evidence and Documentation"
Cohesion: 0.50
Nodes (4): Current audit and validation index, Validation workload and deployment limits, Documentation and source provenance, Native checkout reproduction

### Community 134 - "plan"
Cohesion: 0.83
Nodes (3): Hide, Name, Suffix

### Community 135 - "Main Entry and Listeners"
Cohesion: 0.50
Nodes (4): Cli, Cmd, Check, Run

## Knowledge Gaps
- **138 isolated node(s):** `Off`, `Path`, `Br`, `Gzip`, `Zstd` (+133 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 780 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **53 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `conduit configuration reference` connect `Value Formats Docs` to `Configuration Reference and nginx Mapping`, `Documentation and Roadmap Links`, `Configuration Reference and nginx Mapping`, `HTTP2 Pool Protocol Selection`, `Configuration Reference and nginx Mapping`, `Configuration Reference and nginx Mapping`?**
  _High betweenness centrality (0.044) - this node is a cross-community bridge._
- **Are the 11 inferred relationships involving `http()` (e.g. with `file_descriptors_run_out()` and `idle_and_malformed_clients()`) actually correct?**
  _`http()` has 11 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Off`, `Path`, `Br` to the rest of the system?**
  _138 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `fuzzing.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.050721954831543875 - nodes in this community are weakly interconnected._
- **Why does `Shared` connect `Proxy Response Status and Headers` to `Shared`, `Upstream Passive Ejection`, `Admin API`, `CrowdSec Client`, `ACME Key and CSR`, `Config Reload Watcher`, `Runtime Site Compilation`, `Proxy Session Hooks`, `TLS OpenSSL Bindings`, `Access Log Writer Shards`, `ACME Key and CSR`, `CertStore`?**
  _High betweenness centrality (0.039) - this node is a cross-community bridge._
- **Should `Forward and Basic Auth` be split into smaller, more focused modules?**
  _Cohesion score 0.06656426011264721 - nodes in this community are weakly interconnected._
- **Why does `Pingora fork (vendor/pingora, branch conduit)` connect `Pingora Fork and Stability Patches` to `README.md`, `Proxy Features and Architecture`, `Static Files and Response Compression`, `Proxy Features and Architecture`, `Proxy Features and Architecture`?**
  _High betweenness centrality (0.024) - this node is a cross-community bridge._
## Native soak harness update

Working tree based on `1fa00208`: native h2c/TLS H2 and gRPC fixtures, safe deployment/archive guards and strict result checks. See [soak guide](../scripts/soak/README.md) and [revalidation](../docs/audit/SOAK_REVALIDATION_20261111.md). Previous 24-hour evidence uses an older binary; the next full run remains pending. Existing management/UI/OIDC/SQLite graph concepts remain planned. Historical validation artifacts retain their original snapshot hashes.
