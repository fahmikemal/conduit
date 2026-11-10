# Graph Report - conduit  (2026-11-10)

## Corpus Check
- 76 files · ~146,261 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 14 file(s) not represented in the graph (top: (none) 9, .ico 1, .logrotate 1)

## Summary
- 2643 nodes · 5992 edges · 170 communities (124 shown, 46 thin omitted)
- Extraction: 97% EXTRACTED · 3% INFERRED · 0% AMBIGUOUS · INFERRED: 203 edges (avg confidence: 0.82)
- Token usage: host-agent semantic update not measured; AST extraction is local.

## Graph Freshness
- Built from commit: `3a11d628`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- TLS OpenSSL Bindings
- ACME Key and CSR
- Config Parsing
- Forward and Basic Auth
- End-to-End Tests
- Proxy Features and Architecture
- Per-site Plugin Chain
- Rate and Concurrency Limits
- Proxy Response Status and Headers
- Rule Actions and Header Templates
- HTTP2 Streaming and Tests
- Pingora Fork and Stability Patches
- Runtime Site Compilation
- Proxy Session Hooks
- Planned Management and UI
- Body Replace Plugin
- Cache Storage Globals
- Body Replace Plugin
- Static File Server
- CrowdSec Client
- Access Log File IO
- Main Entry and Listeners
- HTTP2 Streaming and Tests
- Value Formats Docs
- Configuration Reference and nginx Mapping
- E2E HTTP Helpers
- Proxy Comparison Script
- Proxy Comparison Script
- Block Exploits Detector
- Prometheus Metrics Registry
- Upstream Passive Ejection
- Proxy Features and Architecture
- Request Filter and Decide
- Example Plugin Settings
- Example Site Settings
- Pingora Backport Preparation
- Exploit Rule Categories
- Outbound HTTP Client
- CrowdSec Stream Deserializer
- Historical Runtime Recheck
- CrowdSec Decision Storage
- Access Log Writer Shards
- CrowdSec Decision Table Synchronization
- Exploit Scan Engine
- Brand Assets
- WebSocket Soak Client
- Upstream Passive Ejection
- Cache Store and Purge
- Proxy Features and Architecture
- Configuration Reference and nginx Mapping
- Typed Plugin Configuration
- Static File Lookup and Listings
- Typed Plugin Configuration
- Proxy Features and Architecture
- Config Reload Watcher
- Plugin Admission and Concurrency
- Proxy Response Status and Headers
- Cache Store and Purge
- Example Global Configuration
- Parity Script
- Trusted Client IP Resolution
- Cacheability and Variants
- CrowdSec Decision Storage
- Upstream Balancing Configuration
- Example Upstreams and Health Checks
- Audit Evidence and Documentation
- Configuration Reference and nginx Mapping
- Admin API
- CrowdSec Decision Storage
- Example Files and Basic Auth
- Configuration Reference and nginx Mapping
- Configuration Reference and nginx Mapping
- TLS Lifecycle and Hot Reload
- Stability Tests and Soak Evidence
- Historical Static Audit
- Runtime Logger and Filtering
- Proxy Response Status and Headers
- HTTP2 Pool Protocol Selection
- Cache Hit Body Handling
- Outbound HTTP Client
- HTTP2 Pool Protocol Selection
- Access Log Entry Encoding
- Static File Configuration Tests
- Upstream Passive Ejection
- Outbound HTTP Client
- Global Config Sections
- mTLS E2E Tests
- CrowdSec AppSec Plugin
- Static File Lookup and Listings
- RFC 3339 Time Formatting
- Block Exploits Plugin
- Parity Script
- Parity Script
- Parity Stub Backend
- Mixed Traffic Soak Harness
- ACME HTTP Transport
- Parity Script
- Example TLS and ACME
- Example CrowdSec and AppSec
- Example Global Configuration
- Throughput Benchmark Harness
- Example Upstreams and Health Checks
- Configuration Reference and nginx Mapping
- Static Files and Response Compression
- Exploit Rule Parsing
- Static File Lookup and Listings
- Rule Actions and Header Templates
- Upstream Balancing Configuration
- Proxy Features and Architecture
- Example Defaults
- Documentation and Roadmap Links
- HTTP2 Streaming and Tests
- Documentation and Roadmap Links
- Alternating Binary Comparison Harness
- Access Log Fuzz RNG
- Typed Plugin Configuration
- Static File Lookup and Listings
- Cache Storage Round Trip
- CrowdSec AppSec Acquisition
- Audit Evidence and Documentation
- Rule Actions and Header Templates
- CrowdSec IP Bit Masks
- Exploit Scan Engine
- End-to-End Tests
- End-to-End Tests
- HTTP2 Pool Protocol Selection
- Post-fix Validation Memory
- Regression Test Helpers
- Regression Test Helpers
- PGO Training Script
- CrowdSec Log Acquisition
- Audit Evidence and Documentation
- Audit Origin Fixtures
- PGO Build Script
- Fuzz Runner Script
- Cargo Crates
- Release Build Script
- Soak Deploy Script

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
- 2-file cycle: `src/plugin.rs -> src/replace.rs -> src/plugin.rs`
- 2-file cycle: `src/proxy.rs -> src/tls.rs -> src/proxy.rs`
- 2-file cycle: `src/crowdsec.rs -> src/plugin.rs -> src/crowdsec.rs`
- 2-file cycle: `src/limits.rs -> src/plugin.rs -> src/limits.rs`
- 2-file cycle: `src/metrics.rs -> src/runtime.rs -> src/metrics.rs`
- 2-file cycle: `src/proxy.rs -> src/upstream.rs -> src/proxy.rs`
- 3-file cycle: `src/limits.rs -> src/plugin.rs -> src/replace.rs -> src/limits.rs`
- 3-file cycle: `src/proxy.rs -> src/rules.rs -> src/upstream.rs -> src/proxy.rs`
- 3-file cycle: `src/proxy.rs -> src/runtime.rs -> src/tls.rs -> src/proxy.rs`
- 3-file cycle: `src/metrics.rs -> src/tls.rs -> src/proxy.rs -> src/metrics.rs`
- 3-file cycle: `src/plugin.rs -> src/upstream.rs -> src/proxy.rs -> src/plugin.rs`
- 3-file cycle: `src/crowdsec.rs -> src/proxy.rs -> src/plugin.rs -> src/crowdsec.rs`
- 3-file cycle: `src/metrics.rs -> src/runtime.rs -> src/upstream.rs -> src/metrics.rs`
- 3-file cycle: `src/crowdsec.rs -> src/metrics.rs -> src/runtime.rs -> src/crowdsec.rs`
- 3-file cycle: `src/proxy.rs -> src/runtime.rs -> src/upstream.rs -> src/proxy.rs`
- 3-file cycle: `src/crowdsec.rs -> src/proxy.rs -> src/runtime.rs -> src/crowdsec.rs`
- 4-file cycle: `src/plugin.rs -> src/rules.rs -> src/upstream.rs -> src/proxy.rs -> src/plugin.rs`
- 4-file cycle: `src/files.rs -> src/rules.rs -> src/upstream.rs -> src/proxy.rs -> src/files.rs`
- 4-file cycle: `src/cache.rs -> src/rules.rs -> src/upstream.rs -> src/proxy.rs -> src/cache.rs`
- 4-file cycle: `src/proxy.rs -> src/rules.rs -> src/route.rs -> src/upstream.rs -> src/proxy.rs`

## Hyperedges (group relationships)
- **Planned consistent management changes** — docs_roadmap_shared_configuration_contract, docs_roadmap_revision_conflict_control, docs_roadmap_yaml_write_ownership, docs_roadmap_durable_configuration_apply [EXTRACTED 1.00]
- **Verified fixes and preserved documented behavior** — docs_audit_fix_validation, docs_audit_fix_validation_f01, docs_audit_fix_validation_f02, docs_audit_fix_validation_f03, docs_audit_fix_validation_f04, docs_audit_fix_validation_f05, docs_audit_fix_validation_f06, docs_audit_fix_validation_f07, docs_audit_fix_validation_f08, docs_audit_fix_validation_f09, docs_audit_fix_validation_f10 [EXTRACTED 1.00]
- **Inputs to the cache key that prevent cache poisoning** — docs_configuration_cache_key, docs_configuration_forward_auth, docs_configuration_mtls, docs_configuration_trusted_proxy_headers, docs_configuration_proxy_action, docs_technical_body_replace [EXTRACTED 1.00]
- **Plugins that can answer (end) the request** — docs_configuration_rules_plugin, docs_technical_ip_filter, docs_configuration_crowdsec_plugin, docs_configuration_rate_limit, docs_configuration_concurrency, docs_technical_basic_auth, docs_configuration_forward_auth, docs_technical_block_exploits [EXTRACTED 1.00]
- **Phase three core stability validation** — docs_technical_upstream_flapping_test, docs_technical_soak_24h_results, docs_technical_fuzzing, docs_technical_panic_audit, docs_technical_open_websockets_upgrade_test, docs_technical_parity_test [EXTRACTED 1.00]
- **Soak harness evidence and failure attribution** — docs_technical_soak_events, docs_technical_soak_samples, docs_technical_soak_report, docs_technical_soak_archives, docs_technical_soak_upgrade_failure_guard [EXTRACTED 1.00]
- **Pingora ten performance patches** — vendor_readme_patch_79bf391, vendor_readme_patch_32d98ca, vendor_readme_patch_ecef83b, vendor_readme_patch_9586c10, vendor_readme_patch_c448eff, vendor_readme_patch_9c64421, vendor_readme_patch_3357c9e, vendor_readme_patch_bb3859d, vendor_readme_patch_c4cce19, vendor_readme_patch_0a6aee4 [EXTRACTED 1.00]
- **Backend failure handling (selection, retries, ejection, health checks)** — docs_configuration_backend_selection, docs_configuration_retries, docs_configuration_passive_ejection, docs_configuration_active_health_checks, docs_configuration_upstream_pools [INFERRED 0.85]

## Communities (170 total, 46 thin omitted)

### Community 0 - "TLS OpenSSL Bindings"
Cohesion: 0.05
Nodes (24): F08: Wildcard certificate override and coverage corrected, CertConfig, ask_client_certs(), ask_for_certificate(), CertKey, CertResolver, CertStore, client_hello_sni() (+16 more)

### Community 1 - "ACME Key and CSR"
Cohesion: 0.06
Nodes (40): acme_err(), Ari, asn1_unix(), at(), backoff(), backoff_grows_and_caps(), bundle(), cert_id() (+32 more)

### Community 2 - "Config Parsing"
Cohesion: 0.06
Nodes (39): apply(), retain_sites(), AccessLogSection, acme_section(), AppSecSection, CacheSection, Config, CrowdSecSection (+31 more)

### Community 3 - "Forward and Basic Auth"
Cohesion: 0.06
Nodes (34): basic(), basic_auth_flow(), basic_auth_settings(), BasicAuth, cache_key(), CACHE_KEY, check(), config() (+26 more)

### Community 4 - "End-to-End Tests"
Cohesion: 0.06
Nodes (41): Answer, auth_service(), body_exchange(), body_upstream(), BOUNCER_KEY, cache_upstream(), Counts, CROWDSEC_TAG (+33 more)

### Community 5 - "Proxy Features and Architecture"
Cohesion: 0.06
Nodes (41): block_exploits categories and rule ids, scripts/ab.sh A/B harness, Apache httpd, Authentication tests, Cross-proxy cgroup CPU and anonymous-memory comparison, block_exploits plugin, block_exploits attack and ordinary-request tests, Stale cache revalidation and stale-if-error (+33 more)

### Community 6 - "Per-site Plugin Chain"
Cohesion: 0.08
Nodes (16): HeaderOps, HeadersConfig, IpFilterConfig, Chain, chain(), HeadersPlugin, ip_filter(), IpFilter (+8 more)

### Community 7 - "Rate and Concurrency Limits"
Cohesion: 0.08
Nodes (21): F07: Serialized admission for the same limiter key, RateLimitConfig, carried(), CARRIED, chain(), check_status(), client(), Concurrency (+13 more)

### Community 8 - "Proxy Response Status and Headers"
Cohesion: 0.06
Nodes (23): ACME_PREFIX, BASE, CACHE_STATUS, each_worker_thread_has_its_own_pool_group(), header_str(), ip_text(), ip_text_matches_display(), new_request_id() (+15 more)

### Community 9 - "Rule Actions and Header Templates"
Cohesion: 0.09
Nodes (28): HeaderMatch, RuleConfig, Action, Deny, Proxy, Redirect, Respond, bad_rules_are_rejected() (+20 more)

### Community 10 - "HTTP2 Streaming and Tests"
Cohesion: 0.09
Nodes (25): Native H2 pool race regression, Source and binary validation provenance, Default HTTP/1 compatibility benchmark, HTTP/2 implementation and validation results, Final HTTP/2 release soak, 111 unit and 34 e2e passed, Historical upstream HTTP/1 answer, HTTP/2 feature completion evidence (+17 more)

### Community 11 - "Pingora Fork and Stability Patches"
Cohesion: 0.08
Nodes (26): Real-binary end-to-end tests, CasedHeaderName, daemon_wait_for_ready, Docker release build includes vendored Pingora, FlushTrackingMock, TransportConnector idle connection pool, [patch.crates-io] wiring of pingora-* crates, e539402: silence a compiler warning in pingora (+18 more)

### Community 12 - "Runtime Site Compilation"
Cohesion: 0.11
Nodes (26): AcmeSection, UpgradeMode, Off, Websocket, render(), acme_dir(), acme_plan(), acme_site() (+18 more)

### Community 13 - "Proxy Session Hooks"
Cohesion: 0.14
Nodes (13): send(), compressible(), compressible_types(), Conduit, Ctx, decorate(), enable_compression(), final_status() (+5 more)

### Community 14 - "Planned Management and UI"
Cohesion: 0.06
Nodes (14): Authentication and authorization, Conduit management and Web UI roadmap, Current foundation and missing work, Durable apply, rollback and recovery, Enterprise scope, External YAML edits, Generic OIDC, Authentik and ZITADEL, Lightweight UI and observability (+6 more)

### Community 15 - "Body Replace Plugin"
Cohesion: 0.11
Nodes (24): ActionConfig, Deny, File, Proxy, Redirect, Respond, ActionRaw, FileConfig (+16 more)

### Community 16 - "Cache Storage Globals"
Cohesion: 0.09
Nodes (15): ENTRY_OVERHEAD, EVICTION, heuristic(), LOCK_AGE, LOCK, lock(), no_default_ttl(), outcome() (+7 more)

### Community 17 - "Body Replace Plugin"
Cohesion: 0.12
Nodes (20): BodyReplaceConfig, ReplacePair, Gate, Answer, Check, Pass, Replace, Route (+12 more)

### Community 18 - "Static File Server"
Cohesion: 0.09
Nodes (20): CHUNK, conditionals_and_ranges(), content_type(), content_types(), etag_listed(), FileReply, http_date(), LIST_LIMIT (+12 more)

### Community 19 - "CrowdSec Client"
Cohesion: 0.09
Nodes (17): APPSEC_WARNED, BAN_PAGE, base_url(), EPOCH, LAPI_DEADLINE, LAPI_MAX_RESPONSE, LAPI_TIMEOUT, MAX_BODY (+9 more)

### Community 20 - "Access Log File IO"
Cohesion: 0.08
Nodes (14): ESCAPE, FLUSH_EVERY, HEX, HIGHS, int(), needs_escape(), ONES, OpenFile (+6 more)

### Community 21 - "Main Entry and Listeners"
Cohesion: 0.09
Nodes (12): check(), Cli, Cmd, Check, Run, DEFAULT_CONFIG, GLOBAL, huge_pages() (+4 more)

### Community 22 - "HTTP2 Streaming and Tests"
Cohesion: 0.24
Nodes (31): Validated audit findings closed, acme_with_pebble(), auth_plugins(), block_exploits(), cpu_time(), crowdsec_bouncer_and_appsec(), echo_upstream(), file_descriptors_run_out() (+23 more)

### Community 23 - "Value Formats Docs"
Cohesion: 0.07
Nodes (30): 10. Response cache, 12. Headers conduit adds and passes, 13. Protocol upgrades (WebSocket and others), 14. Compression, 15. Responses conduit generates, 16. Access log, 17. Admin endpoints and metrics, 18. Coming from nginx (+22 more)

### Community 24 - "Configuration Reference and nginx Mapping"
Cohesion: 0.14
Nodes (27): 5. Sites, Authelia / Authentik / oauth2-proxy, Headers conduit adds to client responses, Cloudflare, Response compression (gzip, br, zstd, level 1), `default_site`, defaults section (timeouts, max_body_size, compression), File responses (ETag, conditionals, ranges, MIME) (+19 more)

### Community 25 - "E2E HTTP Helpers"
Cohesion: 0.17
Nodes (23): backend_state(), config(), end_to_end(), exchange(), file_server(), graceful_upgrade(), http(), https() (+15 more)

### Community 26 - "Proxy Comparison Script"
Cohesion: 0.13
Nodes (18): configs(), hcm(), listener(), Container, container_name(), docker_rm(), main(), med() (+10 more)

### Community 27 - "Proxy Comparison Script"
Cohesion: 0.21
Nodes (12): main(), launch(), cert(), fingerprint(), main(), reloaded(), origin(), port() (+4 more)

### Community 28 - "Block Exploits Detector"
Cohesion: 0.09
Nodes (20): AGENT, Attack, ATTACKS, attacks_are_caught_by_their_rule(), detect(), ENGINE, every_rule_has_an_attack(), H (+12 more)

### Community 29 - "Prometheus Metrics Registry"
Cohesion: 0.10
Nodes (7): CLASSES, forget_site(), LATENCY_BUCKETS, lazy!, PROCESS, SiteMetrics, status_classes()

### Community 30 - "Upstream Passive Ejection"
Cohesion: 0.11
Nodes (6): EPOCH, hashing_is_sticky(), load_balancer(), MAX_ITERATIONS, PROBE_HTTP, PROBE_TCP

### Community 31 - "Proxy Features and Architecture"
Cohesion: 0.09
Nodes (15): Built-in ACME client (HTTP-01, ARI), Cloudflare (CDN in front of the origin), CrowdSec AppSec (WAF) checks, Built-in CrowdSec L7 bouncer, CrowdSec parser for conduits access log (deploy/crowdsec), CrowdSec full-loop tests, Active HTTP/TCP health checks, Load-balancing algorithms (round_robin, random, least_conn, Ketama hash) (+7 more)

### Community 32 - "Request Filter and Decide"
Cohesion: 0.22
Nodes (19): request(), Effects, client_cert(), client_cert_headers(), client_scheme(), Conn, decide(), files_and_cache() (+11 more)

### Community 33 - "Example Plugin Settings"
Cohesion: 0.09
Nodes (22): except, skip_params, replace, types, max, copy_headers, url, response (+14 more)

### Community 34 - "Example Site Settings"
Cohesion: 0.09
Nodes (22): default_ttl, stale_if_error, ca, sites, acme, cache, compression, force_https (+14 more)

### Community 35 - "Pingora Backport Preparation"
Cohesion: 0.10
Nodes (19): HTTP/2 upstream: implementasi dan validasi, Integritas dan pemetaan, Pemakaian, Perbaikan dependency yang diperlukan, Performa, Perubahan implementasi, Stabilitas H2 upstream, Uji fungsional dan regresi (+11 more)

### Community 36 - "Exploit Rule Categories"
Cohesion: 0.11
Nodes (16): Category, .ALL, Dotfiles, Php, Rce, Scanners, Spam, Sqli (+8 more)

### Community 37 - "Outbound HTTP Client"
Cohesion: 0.19
Nodes (3): Client, IDLE, Reply

### Community 38 - "CrowdSec Stream Deserializer"
Cohesion: 0.13
Nodes (4): Apply, List, List<'_, '_, S>, &mut Apply<'_, S>

### Community 39 - "Historical Runtime Recheck"
Cohesion: 0.10
Nodes (20): Answer, Artefak reproduksi, Bukti per temuan, F01: ada kontrol yang membedakan bug dari konfigurasi terbuka, F02: fallback rentan race, tetapi symlink biasa bukan bypass, F03: miss pertama, hit salah authority, dan control baru, F04: perilaku nyata, tetapi klasifikasi bug ditarik, F05: overflow diuji dengan Rust, termasuk mode teroptimisasi (+12 more)

### Community 40 - "CrowdSec Decision Storage"
Cohesion: 0.18
Nodes (8): Decision, Family, Family<K>, Map, Slot, Many, One, Table

### Community 41 - "Access Log Writer Shards"
Cohesion: 0.17
Nodes (5): AccessLog, lock(), Shard, Shards, Writer

### Community 42 - "CrowdSec Decision Table Synchronization"
Cohesion: 0.14
Nodes (9): Applied, AppSec, CrowdSecPlugin, now(), parse_into(), Plan, pull(), site_plugin_refuses_held_decisions() (+1 more)

### Community 43 - "Exploit Scan Engine"
Cohesion: 0.18
Nodes (14): after(), contains(), decode_query(), decoding(), Engine, fuzz_check(), has_escape(), header_suspect() (+6 more)

### Community 44 - "Brand Assets"
Cohesion: 0.15
Nodes (15): conduit-app-icon.svg, Conduit brand assets, Ember palette, favicon.svg / favicon.ico, Geist typeface (SemiBold 600, Regular 400, Mono 400), Lanes mark, conduit-logo-*-ember.svg / conduit-mark-ember.svg, conduit-logo-horizontal.svg (+7 more)

### Community 45 - "WebSocket Soak Client"
Cohesion: 0.25
Nodes (11): client(), shorts(), connect(), echo(), frame(), long_lived(), read_frame(), serve() (+3 more)

### Community 46 - "Upstream Passive Ejection"
Cohesion: 0.15
Nodes (6): LoadBalancer<S>, Member, MemberStatus, now_ms(), Pool, Select

### Community 47 - "Cache Store and Purge"
Cohesion: 0.16
Nodes (3): purge(), storage_round_trip(), tracked()

### Community 48 - "Proxy Features and Architecture"
Cohesion: 0.11
Nodes (17): write(), scripts/bench.sh benchmark and regression gate, Benchmark, Benchmark gate with baseline-fixed rates and p99 tolerances, Comparison with other reverse proxies, CPU profile, Daemon readiness smoke (2026-11-10: 255,136 requests, 2 upgrades), Fuzz properties: config, path, access_log, request, route, body_replace, block_exploits (+9 more)

### Community 49 - "Configuration Reference and nginx Mapping"
Cohesion: 0.16
Nodes (18): 11. TLS, ACME and client certificates, `access_log`, ACME, Admin endpoints (/metrics, /healthz, /status, /cache/purge), Cache-Status header (RFC 9211), certbot, Certificate selection, conduit check command (+10 more)

### Community 50 - "Typed Plugin Configuration"
Cohesion: 0.11
Nodes (19): BlockExploitsConfig, ConcurrencyConfig, CrowdSecPluginConfig, ExploitException, ExploitMode, Block, Log, MatchConfig (+11 more)

### Community 51 - "Static File Lookup and Listings"
Cohesion: 0.17
Nodes (14): Files, Found, File, Listing, Missing, Redirect, Status, html_escape() (+6 more)

### Community 52 - "Typed Plugin Configuration"
Cohesion: 0.12
Nodes (12): Upstream HTTP version policy, Upstream protocol behavior, BackendConfig, UpstreamHttpVersion, Auto, Http1, Http2, UpstreamRef (+4 more)

### Community 53 - "Proxy Features and Architecture"
Cohesion: 0.11
Nodes (14): Commands, conduit: technical documentation, Deferred features (session tickets, OCSP, admin API, OpenTelemetry, captcha, DNS-01), Features, HTTP/1.1 and HTTPS throughput analysis, HTTP/1.1 and HTTPS throughput, Phase 2 (ACME, CrowdSec, limits, mTLS, auth), Phase 1 targets (+6 more)

### Community 54 - "Config Reload Watcher"
Cohesion: 0.16
Nodes (4): Reloader, snapshot(), Stamp, stamp()

### Community 55 - "Plugin Admission and Concurrency"
Cohesion: 0.18
Nodes (9): Local, Step, Answer, Check, Pass, acme_response(), appsec(), check_client_cert() (+1 more)

### Community 56 - "Proxy Response Status and Headers"
Cohesion: 0.12
Nodes (6): AcmeService, AdminSection, ListenSection, ReloadStatus, Shared, HealthService

### Community 57 - "Cache Store and Purge"
Cohesion: 0.26
Nodes (5): lock_shard(), Object, Shard, Store, variance()

### Community 58 - "Example Global Configuration"
Cohesion: 0.11
Nodes (17): access_log, path, queue_capacity, admin, listen, cache, lock_timeout, max_object_size (+9 more)

### Community 59 - "Parity Script"
Cohesion: 0.13
Nodes (10): Audit dan validasi Conduit, Batas hasil, Cara membaca laporan, Graphify, Context7 dan provenance, Matriks penutupan, Reproduksi pada checkout baru, Status saat ini, accepted_bits() (+2 more)

### Community 60 - "Trusted Client IP Resolution"
Cohesion: 0.26
Nodes (9): canonical(), Cidr, cidr_contains(), CLOUDFLARE, ip(), RealIp, recursive_skips_trusted_hops(), trusted_peer_header_used() (+1 more)

### Community 61 - "Cacheability and Variants"
Cohesion: 0.18
Nodes (9): eligibility(), req(), resp(), site(), SiteCache, stored(), vary_names(), what_is_stored() (+1 more)

### Community 62 - "CrowdSec Decision Storage"
Cohesion: 0.15
Nodes (12): renewal_from_lifetime(), d(), ip(), lookup_cost(), V4, Remedy, Ban, Captcha (+4 more)

### Community 63 - "Upstream Balancing Configuration"
Cohesion: 0.18
Nodes (14): Balance, Hash, LeastConn, Random, RoundRobin, PoolConfig, least_conn_prefers_idle_backends(), passive_ejection_and_panic_mode() (+6 more)

### Community 64 - "Example Upstreams and Health Checks"
Cohesion: 0.12
Nodes (16): url, weight, eject, failures, backends, backends, balance, hash_key (+8 more)

### Community 65 - "Audit Evidence and Documentation"
Cohesion: 0.12
Nodes (15): F02: Descriptor fallback and pinned directory listing, F03: Cache authority includes the original port, F04: Documented fresh cache retention preserved, F06: Zero or unrepresentable watcher interval rejected, F09: Explicit Vary preserves unknown content codings, F10: README Server header contract aligned, Graph provenance and runtime-proof boundary, Local 600-second stability target (+7 more)

### Community 66 - "Configuration Reference and nginx Mapping"
Cohesion: 0.20
Nodes (15): 6. Match conditions, deny action, file action (in-memory file response), Responses conduit generates (status table, HTML status page), Header conditions, Path normalization, proxy action (routing and path rewrite), redirect action and placeholders (+7 more)

### Community 67 - "Admin API"
Cohesion: 0.28
Nodes (6): Admin, bad(), reply(), service(), unescape(), unix()

### Community 68 - "CrowdSec Decision Storage"
Cohesion: 0.19
Nodes (10): Apply<'_, S>, Delta, go_duration_ms(), go_durations(), Net, V6, nets(), parse_net() (+2 more)

### Community 69 - "Example Files and Basic Auth"
Cohesion: 0.13
Nodes (15): htpasswd, match, match, root, strip_prefix, try_files, method, path (+7 more)

### Community 70 - "Configuration Reference and nginx Mapping"
Cohesion: 0.14
Nodes (14): description, filter, name, onsuccess, 8. The other plugins, `basic_auth`, `block_exploits`, `body_replace` (+6 more)

### Community 71 - "Configuration Reference and nginx Mapping"
Cohesion: 0.26
Nodes (13): Cache purge endpoint, Freshness, revalidation, stale serving and cache lock, Cache storage rules (RFC 9111, stricter), `concurrency`, Fixed-size counting sketch, Match conditions (match: block), Coming from nginx: directive equivalents, Plugin chain and suggested order (+5 more)

### Community 72 - "TLS Lifecycle and Hot Reload"
Cohesion: 0.16
Nodes (10): Cache purge endpoint (POST /cache/purge), conduit check (config validation), Hot reload without dropped connections, Production rollout began in November 2026, Prometheus metrics, Certificate rotation and reload under load test, Abort soak when graceful handover or admin PID verification fails, Apache License 2.0 (+2 more)

### Community 73 - "Stability Tests and Soak Evidence"
Cohesion: 0.15
Nodes (14): Graceful binary upgrade (socket handover), Graceful upgrade test, upgrade_with_open_websockets test, Panic and misbehaving-client audit, 24-hour soak (2026-11-08/09: 56.2 million requests), Unique soak archives preserve prior output and runtime files, Soak reload, backend flap, certificate and access-log rotation schedule, Old soak final-process RSS step remains inconclusive (+6 more)

### Community 74 - "Historical Static Audit"
Cohesion: 0.13
Nodes (14): Answer, Arsitektur yang terbentuk, Audit mendalam Conduit, Cakupan dan batas kejujuran, Cara membaca paket audit, Invarian yang benar-benar penting, Outcome, Pemeriksaan dokumentasi terkini (+6 more)

### Community 75 - "Runtime Logger and Filtering"
Cohesion: 0.24
Nodes (4): benign(), chatty(), Logger, only_the_pool_race_is_benign()

### Community 76 - "Proxy Response Status and Headers"
Cohesion: 0.27
Nodes (14): acme_challenge(), acme_challenge_from_memory(), answer(), body_limit_and_default_site(), client_certificates(), host_parsing(), https_redirect_runs_before_plugins(), plugins_see_normalized_path_and_client() (+6 more)

### Community 77 - "HTTP2 Pool Protocol Selection"
Cohesion: 0.21
Nodes (5): HealthCheckConfig, ProbeType, Http, Tcp, Probe

### Community 79 - "Outbound HTTP Client"
Cohesion: 0.23
Nodes (12): F01: Single percent decoding policy and file lookup, access_log(), path(), route(), n(), normalize(), normalize_decoded(), PathError (+4 more)

### Community 80 - "HTTP2 Pool Protocol Selection"
Cohesion: 0.20
Nodes (13): 4. Upstream pools, Active health checks, Backend DNS resolution at load time, Backend selection with fallback, Backends and DNS, Balancing, Choosing a backend, Passive ejection (+5 more)

### Community 81 - "Access Log Entry Encoding"
Cohesion: 0.23
Nodes (9): Entry, entry(), full_shard_drops_and_counts(), hostile_values_stay_one_line(), ip(), json_line(), line(), millis() (+1 more)

### Community 82 - "Static File Configuration Tests"
Cohesion: 0.21
Nodes (11): FilesConfig, body(), Dir, files(), Hide, Name, Suffix, precompressed_try_files_browse_prefix() (+3 more)

### Community 83 - "Upstream Passive Ejection"
Cohesion: 0.23
Nodes (3): Lease, Resolved, Target

### Community 84 - "Outbound HTTP Client"
Cohesion: 0.18
Nodes (4): config(), ip_backends_only(), ROUTER, ROUTES

### Community 85 - "Global Config Sections"
Cohesion: 0.21
Nodes (8): F05: Duration multiplication overflow rejected, durations(), parse_duration(), parse_size(), Raw, Num, Str, sizes()

### Community 86 - "mTLS E2E Tests"
Cohesion: 0.31
Nodes (5): Ca, cert_builder(), ec_key(), mtls_get(), MtlsReply

### Community 87 - "CrowdSec AppSec Plugin"
Cohesion: 0.19
Nodes (8): appsec_request_shape(), forwarded(), http_version(), Verdict, Allow, Block, verdict_status(), X_API_KEY

### Community 88 - "Static File Lookup and Listings"
Cohesion: 0.19
Nodes (5): Accepted, Encoding, Br, Gzip, Zstd

### Community 89 - "RFC 3339 Time Formatting"
Cohesion: 0.26
Nodes (6): civil_from_days(), known_instants(), rfc3339(), rfc3339_millis(), seconds_part(), write_rfc3339_millis()

### Community 90 - "Block Exploits Plugin"
Cohesion: 0.27
Nodes (7): block_and_log_modes(), BlockExploits, detect_with(), Hit, may_log(), plugin(), settings()

### Community 91 - "Parity Script"
Cohesion: 0.31
Nodes (4): category(), main(), make_browser(), run()

### Community 92 - "Parity Script"
Cohesion: 0.29
Nodes (5): fetch(), hosts_from(), main(), run(), read_cases()

### Community 94 - "Mixed Traffic Soak Harness"
Cohesion: 0.42
Nodes (10): cert(), event(), every(), flap(), keep_log(), rotate_log(), sample(), soak.sh script (+2 more)

### Community 97 - "Example TLS and ACME"
Cohesion: 0.20
Nodes (10): directory, email, storage, cert, key, name, tls, acme (+2 more)

### Community 98 - "Example CrowdSec and AppSec"
Cohesion: 0.20
Nodes (10): body_limit, failure_action, timeout, url, crowdsec, api_key_file, appsec, lapi_url (+2 more)

### Community 99 - "Example Global Configuration"
Cohesion: 0.20
Nodes (10): server, pid_file, shutdown_grace, shutdown_timeout, threads, transparent_huge_pages, upgrade_sock, upstream_keepalive_pool_size (+2 more)

### Community 100 - "Throughput Benchmark Harness"
Cohesion: 0.36
Nodes (8): baseline_rate(), cpu_ticks(), load(), peak_rss_kb(), pids_of(), run(), bench.sh script, wait_port()

### Community 101 - "Example Upstreams and Health Checks"
Cohesion: 0.25
Nodes (9): expect, fall, interval, path, rise, timeout, type, health_check (+1 more)

### Community 102 - "Configuration Reference and nginx Mapping"
Cohesion: 0.22
Nodes (9): statics, expression, meta, parsed, target, value, key(), keys_and_variants() (+1 more)

### Community 103 - "Static Files and Response Compression"
Cohesion: 0.22
Nodes (5): Static-file conditional requests and single byte ranges, Static file server (files:), Static file-server tests, RFC 9110 (HTTP semantics), Upstream Brotli finalization (b5dea5a, PR 1021, issue 1013)

### Community 104 - "Exploit Rule Parsing"
Cohesion: 0.28
Nodes (5): all_rules(), categories_mask(), cost(), tautologies(), tautology()

### Community 105 - "Static File Lookup and Listings"
Cohesion: 0.31
Nodes (4): open_beneath(), read_exact_at(), reply(), show()

### Community 106 - "Rule Actions and Header Templates"
Cohesion: 0.25
Nodes (7): Part, Host, Lit, Path, RequestUri, Scheme, Template

### Community 107 - "Upstream Balancing Configuration"
Cohesion: 0.28
Nodes (7): HashKey, ClientIp, Cookie, Header, None, Path, Uri

### Community 108 - "Proxy Features and Architecture"
Cohesion: 0.25
Nodes (7): Brand assets in project documentation, Current local validation, server_header end-to-end test, Local test results, 111 application unit tests passed, Final validation and documentation navigation, Project README overview

### Community 109 - "Example Defaults"
Cohesion: 0.25
Nodes (8): defaults, compression, connect_timeout, keepalive_timeout, max_body_size, read_timeout, upstream_idle_timeout, write_timeout

### Community 110 - "Documentation and Roadmap Links"
Cohesion: 0.25
Nodes (8): 7. The `rules` plugin, Actions, `deny`, Examples, `file`, `proxy`, `redirect`, `respond`

### Community 111 - "HTTP2 Streaming and Tests"
Cohesion: 0.29
Nodes (3): CrowdSec, docker(), Pebble

### Community 112 - "Documentation and Roadmap Links"
Cohesion: 0.29
Nodes (7): 3. Global sections, `admin`, `cache`, `defaults`, `listen`, `server`, `tls`

### Community 113 - "Alternating Binary Comparison Harness"
Cohesion: 0.48
Nodes (5): config(), cpu_ticks(), load(), ab.sh script, wait_port()

### Community 115 - "Typed Plugin Configuration"
Cohesion: 0.38
Nodes (3): ServerHeader, Off, Value

### Community 116 - "Static File Lookup and Listings"
Cohesion: 0.33
Nodes (4): Candidate, Part, Lit, Path

### Community 118 - "CrowdSec AppSec Acquisition"
Cohesion: 0.33
Nodes (5): appsec_config, labels, type, listen_addr, source

### Community 119 - "Audit Evidence and Documentation"
Cohesion: 0.33
Nodes (6): Hasil suite dan pemeriksaan, Integritas dan batas kesimpulan, Matriks penutupan, Perbaikan dan validasi penutupan temuan, Performa release sebelum/sesudah, Stabilitas berkelanjutan

### Community 120 - "Rule Actions and Header Templates"
Cohesion: 0.60
Nodes (5): StrMatch, Exact, Prefix, Regex, StrMatchRaw

### Community 121 - "CrowdSec IP Bit Masks"
Cohesion: 0.33
Nodes (4): Bits, .WIDTH, u32, .WIDTH

### Community 122 - "Exploit Scan Engine"
Cohesion: 0.33
Nodes (3): Place, Path, Query

### Community 125 - "HTTP2 Pool Protocol Selection"
Cohesion: 0.40
Nodes (3): Protocol isolation and HTTP/2 health checks, HTTP/2 connection stream limit, Cleartext HTTP/2 prior knowledge

### Community 126 - "Post-fix Validation Memory"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: ok languns perbaiki semua temuan ya,, dan setiap temuan fix languns di uji e2e ya,, dan kalo udah semua uji e2e semunaya dan uji stabilitas dan performasnya dan uji sekali algi apakah sduah tertutup, Source Nodes

### Community 127 - "Regression Test Helpers"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: ini conduit dukunga ke upstream ini hanay di http/1 ya? http/2 belum?, Source Nodes

### Community 128 - "Regression Test Helpers"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: loh kerjakan bro,, karena ada tool internal butuh http/2 upstream,, semua fitur di pingora kalo bisa ngapain gk di kerjakan di conduit bro, Source Nodes

### Community 129 - "PGO Training Script"
Cohesion: 0.60
Nodes (3): fetch(), pgo-train.sh script, urls()

### Community 130 - "CrowdSec Log Acquisition"
Cohesion: 0.50
Nodes (3): filenames, labels, type

### Community 131 - "Audit Evidence and Documentation"
Cohesion: 0.50
Nodes (4): Current audit and validation index, Validation workload and deployment limits, Documentation and source provenance, Native checkout reproduction

## Knowledge Gaps
- **138 isolated node(s):** `Regex`, `Off`, `Hash`, `LeastConn`, `Random` (+133 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 748 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **46 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `conduit configuration reference` connect `Value Formats Docs` to `Configuration Reference and nginx Mapping`, `Configuration Reference and nginx Mapping`, `Documentation and Roadmap Links`, `Documentation and Roadmap Links`, `Configuration Reference and nginx Mapping`, `HTTP2 Pool Protocol Selection`, `Configuration Reference and nginx Mapping`?**
  _High betweenness centrality (0.045) - this node is a cross-community bridge._
- **Are the 11 inferred relationships involving `http()` (e.g. with `file_descriptors_run_out()` and `idle_and_malformed_clients()`) actually correct?**
  _`http()` has 11 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Regex`, `Off`, `Hash` to the rest of the system?**
  _138 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `TLS OpenSSL Bindings` be split into smaller, more focused modules?**
  _Cohesion score 0.04900900900900901 - nodes in this community are weakly interconnected._
- **Why does `RequestView` connect `Rule Actions and Header Templates` to `Forward and Basic Auth`, `Per-site Plugin Chain`, `Rate and Concurrency Limits`, `Proxy Response Status and Headers`, `CrowdSec Decision Table Synchronization`, `Exploit Scan Engine`, `Cache Storage Globals`, `Body Replace Plugin`, `Static File Server`, `CrowdSec Client`, `Static File Lookup and Listings`, `Plugin Admission and Concurrency`, `Block Exploits Plugin`, `Block Exploits Detector`, `Cacheability and Variants`?**
  _High betweenness centrality (0.034) - this node is a cross-community bridge._
- **Should `ACME Key and CSR` be split into smaller, more focused modules?**
  _Cohesion score 0.0578386605783866 - nodes in this community are weakly interconnected._
- **Why does `request()` connect `Proxy Comparison Script` to `Historical Runtime Recheck`, `Audit Evidence and Documentation`?**
  _High betweenness centrality (0.026) - this node is a cross-community bridge._
## Planned UI quality requirements

The graph includes the quality release gates in [docs/roadmap.md](../docs/roadmap.md), including resource budgets, capability coverage, real UX workflow tests, WCAG 2.2 AA and applicable OWASP ASVS 5.0.0 verification targets. These targets are planned and not current conformance claims. The roadmap remains linked from README,
the technical guide and configuration reference. Management/UI/OIDC/SQLite capabilities
are marked **planned**, not implemented. The recorded requirements include headless by
default, optional Hallmark-designed UI, shared backend configuration contracts, expected
revisions, exclusive live-file write ownership for strict concurrency guarantees, journalled
apply/recovery, local auth and generic OIDC with Authentik/ZITADEL, and production acceptance
tests. Multi-node HA remains a separate design.


This update describes the working tree after commit `3a11d62`; it does not deploy code or
change the Krynox soak. Current code remains the tested proxy implementation. Earlier
validation JSON files retain their original snapshot hashes. See
[ui-quality-validation.json](../docs/audit/ui-quality-validation.json) for this documentation
update's link checks, source hashes and graph health. Historical audit snapshots and vendored
Pingora AST remain outside the configured graph scope.
