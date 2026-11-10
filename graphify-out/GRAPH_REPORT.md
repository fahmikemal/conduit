# Graph Report - conduit  (2026-11-10)

## Corpus Check
- 76 files · ~145,825 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 14 file(s) not represented in the graph (top: (none) 9, .ico 1, .logrotate 1)

## Summary
- 2638 nodes · 5985 edges · 179 communities (129 shown, 50 thin omitted)
- Extraction: 97% EXTRACTED · 3% INFERRED · 0% AMBIGUOUS · INFERRED: 203 edges (avg confidence: 0.82)
- Token usage: host-agent semantic update not measured; AST extraction is local.

## Graph Freshness
- Built from commit: `543a5d5e`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- Proxy Features and Architecture
- Forward and Basic Auth
- End-to-End Tests
- Body Replace Plugin
- Outbound HTTP Client
- Per-site Plugin Chain
- Proxy Response Status and Headers
- Rate and Concurrency Limits
- HTTP2 Streaming and Tests
- Pingora Fork and Stability Patches
- Rule Actions and Header Templates
- CrowdSec Client
- Config Parsing
- Runtime Site Compilation
- Proxy Session Hooks
- Access Log File IO
- Cache Storage Globals
- HTTP2 Streaming and Tests
- Static File Server
- Pingora Backport Preparation
- Value Formats Docs
- Planned Management and UI
- Upstream Passive Ejection
- Upstream Passive Ejection
- Configuration Reference and nginx Mapping
- Prometheus Metrics Registry
- E2E HTTP Helpers
- Proxy Comparison Script
- ACME Certificate Service
- Proxy Comparison Script
- Proxy Features and Architecture
- Main Entry and Listeners
- Request Filter and Decide
- Block Exploits Detector
- TLS OpenSSL Bindings
- Proxy Features and Architecture
- Example Upstreams and Health Checks
- CrowdSec Decision Storage
- Example Global Configuration
- Static File Lookup and Listings
- Example Plugin Settings
- Audit Evidence and Documentation
- Access Log Writer Shards
- Global Config Sections
- Typed Plugin Configuration
- CrowdSec Stream Deserializer
- Historical Runtime Recheck
- Cache Store and Purge
- Exploit Rule Categories
- Brand Assets
- Config Reload Watcher
- WebSocket Soak Client
- Cacheability and Variants
- Configuration Reference and nginx Mapping
- Typed Plugin Configuration
- Admin API
- CrowdSec Decision Table Synchronization
- Exploit Scan Engine
- HTTP2 Pool Protocol Selection
- Certificate Store Selection
- Trusted Client IP Resolution
- Example Site Settings
- Stability Tests and Soak Evidence
- CrowdSec Decision Storage
- Rule Actions and Header Templates
- Configuration Reference and nginx Mapping
- Runtime Logger and Filtering
- Block Exploits Plugin
- Plugin Admission and Concurrency
- Proxy Response Status and Headers
- Cache Store and Purge
- Example Files and Basic Auth
- Configuration Reference and nginx Mapping
- Configuration Reference and nginx Mapping
- Historical Static Audit
- ACME Account Management
- CrowdSec AppSec Plugin
- Upstream Balancing Configuration
- ACME Key and CSR
- HTTP2 Pool Protocol Selection
- TLS Lifecycle and Hot Reload
- Cache Hit Body Handling
- Global Config Sections
- mTLS E2E Tests
- Access Log Entry Encoding
- Static File Configuration Tests
- ACME Key and CSR
- Static File Lookup and Listings
- Parity Script
- Parity Script
- Parity Stub Backend
- Mixed Traffic Soak Harness
- Rule Action Config
- TLS OpenSSL Bindings
- ACME HTTP Transport
- Parity Script
- Example TLS and ACME
- Example CrowdSec and AppSec
- Parity Script
- Throughput Benchmark Harness
- Typed Plugin Configuration
- Upstream Balancing Configuration
- Certificate Store Selection
- Upstream Passive Ejection
- Cache Storage Round Trip
- Static Files and Response Compression
- RFC 3339 Time Formatting
- Exploit Rule Parsing
- Example Defaults
- Documentation and Roadmap Links
- Proxy Features and Architecture
- CrowdSec IP Bit Masks
- HTTP2 Streaming and Tests
- Documentation and Roadmap Links
- Alternating Binary Comparison Harness
- Access Log Fuzz RNG
- Static File Lookup and Listings
- ACME Key and CSR
- CrowdSec AppSec Acquisition
- Rule Actions and Header Templates
- CrowdSec Remediation Lookup
- Exploit Scan Engine
- Static File Server
- End-to-End Tests
- End-to-End Tests
- Example Site Settings
- Configuration Reference and nginx Mapping
- Post-fix Validation Memory
- Regression Test Helpers
- Regression Test Helpers
- PGO Training Script
- Proxy Response Status and Headers
- Config Parsing
- Client Cert Verification
- Example Global Configuration
- CrowdSec Log Acquisition
- Audit Evidence and Documentation
- Audit Origin Fixtures
- Hidden Files and Ranges
- Command Line Interface
- TLS OpenSSL Bindings
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
- 2-file cycle: `src/proxy.rs -> src/tls.rs -> src/proxy.rs`
- 2-file cycle: `src/plugin.rs -> src/replace.rs -> src/plugin.rs`
- 2-file cycle: `src/metrics.rs -> src/runtime.rs -> src/metrics.rs`
- 2-file cycle: `src/proxy.rs -> src/upstream.rs -> src/proxy.rs`
- 2-file cycle: `src/crowdsec.rs -> src/plugin.rs -> src/crowdsec.rs`
- 2-file cycle: `src/limits.rs -> src/plugin.rs -> src/limits.rs`
- 3-file cycle: `src/proxy.rs -> src/runtime.rs -> src/tls.rs -> src/proxy.rs`
- 3-file cycle: `src/metrics.rs -> src/tls.rs -> src/proxy.rs -> src/metrics.rs`
- 3-file cycle: `src/limits.rs -> src/plugin.rs -> src/replace.rs -> src/limits.rs`
- 3-file cycle: `src/proxy.rs -> src/runtime.rs -> src/upstream.rs -> src/proxy.rs`
- 3-file cycle: `src/metrics.rs -> src/runtime.rs -> src/upstream.rs -> src/metrics.rs`
- 3-file cycle: `src/crowdsec.rs -> src/proxy.rs -> src/runtime.rs -> src/crowdsec.rs`
- 3-file cycle: `src/crowdsec.rs -> src/metrics.rs -> src/runtime.rs -> src/crowdsec.rs`
- 3-file cycle: `src/proxy.rs -> src/rules.rs -> src/upstream.rs -> src/proxy.rs`
- 3-file cycle: `src/plugin.rs -> src/upstream.rs -> src/proxy.rs -> src/plugin.rs`
- 3-file cycle: `src/crowdsec.rs -> src/proxy.rs -> src/plugin.rs -> src/crowdsec.rs`
- 4-file cycle: `src/cache.rs -> src/rules.rs -> src/upstream.rs -> src/proxy.rs -> src/cache.rs`
- 4-file cycle: `src/metrics.rs -> src/tls.rs -> src/proxy.rs -> src/upstream.rs -> src/metrics.rs`
- 4-file cycle: `src/metrics.rs -> src/tls.rs -> src/proxy.rs -> src/runtime.rs -> src/metrics.rs`
- 4-file cycle: `src/metrics.rs -> src/runtime.rs -> src/tls.rs -> src/proxy.rs -> src/metrics.rs`

## Hyperedges (group relationships)
- **Planned consistent management changes** — docs_roadmap_shared_configuration_contract, docs_roadmap_revision_conflict_control, docs_roadmap_yaml_write_ownership, docs_roadmap_durable_configuration_apply [EXTRACTED 1.00]
- **Verified fixes and preserved documented behavior** — docs_audit_fix_validation, docs_audit_fix_validation_f01, docs_audit_fix_validation_f02, docs_audit_fix_validation_f03, docs_audit_fix_validation_f04, docs_audit_fix_validation_f05, docs_audit_fix_validation_f06, docs_audit_fix_validation_f07, docs_audit_fix_validation_f08, docs_audit_fix_validation_f09, docs_audit_fix_validation_f10 [EXTRACTED 1.00]
- **Inputs to the cache key that prevent cache poisoning** — docs_configuration_cache_key, docs_configuration_forward_auth, docs_configuration_mtls, docs_configuration_trusted_proxy_headers, docs_configuration_proxy_action, docs_technical_body_replace [EXTRACTED 1.00]
- **Plugins that can answer (end) the request** — docs_configuration_rules_plugin, docs_technical_ip_filter, docs_configuration_crowdsec_plugin, docs_configuration_rate_limit, docs_configuration_concurrency, docs_technical_basic_auth, docs_configuration_forward_auth, docs_technical_block_exploits [EXTRACTED 1.00]
- **Phase three core stability validation** — docs_technical_upstream_flapping_test, docs_technical_soak_24h_results, docs_technical_fuzzing, docs_technical_panic_audit, docs_technical_open_websockets_upgrade_test, docs_technical_parity_test [EXTRACTED 1.00]
- **Soak harness evidence and failure attribution** — docs_technical_soak_events, docs_technical_soak_samples, docs_technical_soak_report, docs_technical_soak_archives, docs_technical_soak_upgrade_failure_guard [EXTRACTED 1.00]
- **Pingora ten performance patches** — vendor_readme_patch_79bf391, vendor_readme_patch_32d98ca, vendor_readme_patch_ecef83b, vendor_readme_patch_9586c10, vendor_readme_patch_c448eff, vendor_readme_patch_9c64421, vendor_readme_patch_3357c9e, vendor_readme_patch_bb3859d, vendor_readme_patch_c4cce19, vendor_readme_patch_0a6aee4 [EXTRACTED 1.00]
- **Backend failure handling (selection, retries, ejection, health checks)** — docs_configuration_backend_selection, docs_configuration_retries, docs_configuration_passive_ejection, docs_configuration_active_health_checks, docs_configuration_upstream_pools [INFERRED 0.85]

## Communities (179 total, 50 thin omitted)

### Community 0 - "Proxy Features and Architecture"
Cohesion: 0.06
Nodes (44): block_exploits categories and rule ids, scripts/ab.sh A/B harness, Built-in ACME client (HTTP-01, ARI), Authentication tests, block_exploits plugin, block_exploits attack and ordinary-request tests, Stale cache revalidation and stale-if-error, Response-cache tests (+36 more)

### Community 1 - "Forward and Basic Auth"
Cohesion: 0.07
Nodes (34): basic(), basic_auth_flow(), basic_auth_settings(), BasicAuth, cache_key(), CACHE_KEY, check(), config() (+26 more)

### Community 2 - "End-to-End Tests"
Cohesion: 0.06
Nodes (41): Answer, auth_service(), body_exchange(), body_upstream(), BOUNCER_KEY, cache_upstream(), Counts, CROWDSEC_TAG (+33 more)

### Community 3 - "Body Replace Plugin"
Cohesion: 0.07
Nodes (34): BodyReplaceConfig, ProxyConfig, ReplacePair, RewriteConfig, Gate, Answer, Check, Pass (+26 more)

### Community 4 - "Outbound HTTP Client"
Cohesion: 0.07
Nodes (19): F01: Single percent decoding policy and file lookup, access_log(), config(), ip_backends_only(), path(), route(), ROUTER, ROUTES (+11 more)

### Community 5 - "Per-site Plugin Chain"
Cohesion: 0.07
Nodes (16): HeaderOps, HeadersConfig, IpFilterConfig, Chain, chain(), HeadersPlugin, ip_filter(), IpFilter (+8 more)

### Community 6 - "Proxy Response Status and Headers"
Cohesion: 0.06
Nodes (26): AdminSection, ListenSection, ACME_PREFIX, BASE, CACHE_STATUS, each_worker_thread_has_its_own_pool_group(), final_status(), ip_text() (+18 more)

### Community 7 - "Rate and Concurrency Limits"
Cohesion: 0.08
Nodes (19): F07: Serialized admission for the same limiter key, carried(), CARRIED, chain(), check_status(), client(), Concurrency, concurrency_holds_until_dropped() (+11 more)

### Community 8 - "HTTP2 Streaming and Tests"
Cohesion: 0.09
Nodes (25): Native H2 pool race regression, Source and binary validation provenance, Default HTTP/1 compatibility benchmark, HTTP/2 implementation and validation results, Final HTTP/2 release soak, 111 unit and 34 e2e passed, Historical upstream HTTP/1 answer, HTTP/2 feature completion evidence (+17 more)

### Community 9 - "Pingora Fork and Stability Patches"
Cohesion: 0.08
Nodes (26): Real-binary end-to-end tests, CasedHeaderName, daemon_wait_for_ready, Docker release build includes vendored Pingora, FlushTrackingMock, TransportConnector idle connection pool, [patch.crates-io] wiring of pingora-* crates, e539402: silence a compiler warning in pingora (+18 more)

### Community 10 - "Rule Actions and Header Templates"
Cohesion: 0.12
Nodes (21): RuleConfig, bad_rules_are_rejected(), BytesMatch, Exact, Prefix, Regex, forgejo_browser_redirect(), header_presence() (+13 more)

### Community 11 - "CrowdSec Client"
Cohesion: 0.08
Nodes (19): APPSEC_WARNED, BAN_PAGE, base_url(), EPOCH, go_duration_ms(), go_durations(), LAPI_DEADLINE, LAPI_MAX_RESPONSE (+11 more)

### Community 12 - "Config Parsing"
Cohesion: 0.08
Nodes (18): AccessLogSection, acme_section(), DefaultSite, example_config_builds(), from_map(), LETSENCRYPT, LETSENCRYPT_STAGING, minimal_config() (+10 more)

### Community 13 - "Runtime Site Compilation"
Cohesion: 0.14
Nodes (26): Config, SiteConfig, UpgradeMode, Off, Websocket, acme_dir(), acme_site(), acme_sites() (+18 more)

### Community 14 - "Proxy Session Hooks"
Cohesion: 0.15
Nodes (12): send(), appsec(), compressible(), compressible_types(), Conduit, Ctx, decorate(), enable_compression() (+4 more)

### Community 15 - "Access Log File IO"
Cohesion: 0.07
Nodes (14): ESCAPE, FLUSH_EVERY, HEX, HIGHS, int(), needs_escape(), ONES, OpenFile (+6 more)

### Community 16 - "Cache Storage Globals"
Cohesion: 0.09
Nodes (12): ENTRY_OVERHEAD, EVICTION, heuristic(), LOCK_AGE, LOCK, lock(), no_default_ttl(), outcome() (+4 more)

### Community 17 - "HTTP2 Streaming and Tests"
Cohesion: 0.24
Nodes (31): Validated audit findings closed, acme_with_pebble(), auth_plugins(), block_exploits(), cpu_time(), crowdsec_bouncer_and_appsec(), echo_upstream(), file_descriptors_run_out() (+23 more)

### Community 18 - "Static File Server"
Cohesion: 0.10
Nodes (16): CHUNK, conditionals_and_ranges(), content_type(), content_types(), etag_listed(), FileReply, http_date(), LIST_LIMIT (+8 more)

### Community 19 - "Pingora Backport Preparation"
Cohesion: 0.07
Nodes (26): HTTP/2 upstream: implementasi dan validasi, Integritas dan pemetaan, Pemakaian, Perbaikan dependency yang diperlukan, Performa, Perubahan implementasi, Stabilitas H2 upstream, Uji fungsional dan regresi (+18 more)

### Community 20 - "Value Formats Docs"
Cohesion: 0.07
Nodes (30): 10. Response cache, 12. Headers conduit adds and passes, 13. Protocol upgrades (WebSocket and others), 14. Compression, 15. Responses conduit generates, 16. Access log, 17. Admin endpoints and metrics, 18. Coming from nginx (+22 more)

### Community 21 - "Planned Management and UI"
Cohesion: 0.07
Nodes (13): Authentication and authorization, Conduit management and Web UI roadmap, Current foundation and missing work, Durable apply, rollback and recovery, Enterprise scope, External YAML edits, Generic OIDC, Authentik and ZITADEL, Lightweight UI and observability (+5 more)

### Community 22 - "Upstream Passive Ejection"
Cohesion: 0.09
Nodes (9): EPOCH, hashing_is_sticky(), HealthService, MAX_ITERATIONS, PROBE_HTTP, PROBE_TCP, resolve(), Resolved (+1 more)

### Community 23 - "Upstream Passive Ejection"
Cohesion: 0.15
Nodes (8): Lease, Member, MemberStatus, now_ms(), passive_ejection_and_panic_mode(), pick(), Pool, Target

### Community 24 - "Configuration Reference and nginx Mapping"
Cohesion: 0.14
Nodes (27): 5. Sites, Authelia / Authentik / oauth2-proxy, Headers conduit adds to client responses, Cloudflare, Response compression (gzip, br, zstd, level 1), `default_site`, defaults section (timeouts, max_body_size, compression), File responses (ETag, conditionals, ranges, MIME) (+19 more)

### Community 25 - "Prometheus Metrics Registry"
Cohesion: 0.09
Nodes (8): result_index(), CLASSES, forget_site(), LATENCY_BUCKETS, lazy!, PROCESS, SiteMetrics, status_classes()

### Community 26 - "E2E HTTP Helpers"
Cohesion: 0.17
Nodes (23): backend_state(), config(), end_to_end(), exchange(), file_server(), graceful_upgrade(), http(), https() (+15 more)

### Community 27 - "Proxy Comparison Script"
Cohesion: 0.13
Nodes (18): configs(), hcm(), listener(), Container, container_name(), docker_rm(), main(), med() (+10 more)

### Community 28 - "ACME Certificate Service"
Cohesion: 0.09
Nodes (9): Ari, backoff(), backoff_grows_and_caps(), CHALLENGES, HTTP_TIMEOUT, MAX_RESPONSE, SiteState, SiteStatus (+1 more)

### Community 29 - "Proxy Comparison Script"
Cohesion: 0.21
Nodes (12): main(), launch(), cert(), fingerprint(), main(), reloaded(), origin(), port() (+4 more)

### Community 30 - "Proxy Features and Architecture"
Cohesion: 0.08
Nodes (21): Brand assets in project documentation, Commands, conduit: technical documentation, Current local validation, Deferred features (session tickets, OCSP, admin API, OpenTelemetry, captcha, DNS-01), Features, HTTP/1.1 and HTTPS throughput analysis, HTTP/1.1 and HTTPS throughput (+13 more)

### Community 31 - "Main Entry and Listeners"
Cohesion: 0.10
Nodes (8): check(), DEFAULT_CONFIG, GLOBAL, huge_pages(), listener(), main(), run(), socket_options()

### Community 32 - "Request Filter and Decide"
Cohesion: 0.17
Nodes (21): request(), Effects, client_cert(), client_cert_headers(), client_scheme(), Conn, decide(), files_and_cache() (+13 more)

### Community 33 - "Block Exploits Detector"
Cohesion: 0.09
Nodes (19): after(), AGENT, Attack, ATTACKS, ENGINE, every_rule_has_an_attack(), H, HANDLERS (+11 more)

### Community 35 - "Proxy Features and Architecture"
Cohesion: 0.09
Nodes (23): write(), Apache httpd, scripts/bench.sh benchmark and regression gate, Benchmark, Cross-proxy cgroup CPU and anonymous-memory comparison, Benchmark gate with baseline-fixed rates and p99 tolerances, Caddy, Comparison with other reverse proxies (+15 more)

### Community 36 - "Example Upstreams and Health Checks"
Cohesion: 0.09
Nodes (25): url, weight, expect, fall, interval, path, rise, timeout (+17 more)

### Community 37 - "CrowdSec Decision Storage"
Cohesion: 0.15
Nodes (12): d(), Decision, Family, Family<K>, ip(), Map, Slot, Many (+4 more)

### Community 38 - "Example Global Configuration"
Cohesion: 0.08
Nodes (23): access_log, path, queue_capacity, admin, listen, cache, lock_timeout, max_object_size (+15 more)

### Community 39 - "Static File Lookup and Listings"
Cohesion: 0.17
Nodes (15): Files, Found, File, Listing, Missing, Redirect, Status, html_escape() (+7 more)

### Community 40 - "Example Plugin Settings"
Cohesion: 0.09
Nodes (22): except, skip_params, replace, types, max, copy_headers, url, response (+14 more)

### Community 41 - "Audit Evidence and Documentation"
Cohesion: 0.09
Nodes (21): F02: Descriptor fallback and pinned directory listing, F03: Cache authority includes the original port, F04: Documented fresh cache retention preserved, F06: Zero or unrepresentable watcher interval rejected, F09: Explicit Vary preserves unknown content codings, F10: README Server header contract aligned, Graph provenance and runtime-proof boundary, Hasil suite dan pemeriksaan (+13 more)

### Community 42 - "Access Log Writer Shards"
Cohesion: 0.16
Nodes (5): AccessLog, lock(), Shard, Shards, Writer

### Community 43 - "Global Config Sections"
Cohesion: 0.15
Nodes (13): AppSecSection, CacheSection, CrowdSecSection, Defaults, FailureAction, Allow, Block, five_seconds() (+5 more)

### Community 44 - "Typed Plugin Configuration"
Cohesion: 0.10
Nodes (21): BlockExploitsConfig, ConcurrencyConfig, CrowdSecPluginConfig, ExploitException, ExploitMode, Block, Log, HeaderMatch (+13 more)

### Community 45 - "CrowdSec Stream Deserializer"
Cohesion: 0.13
Nodes (4): Apply, List, List<'_, '_, S>, &mut Apply<'_, S>

### Community 46 - "Historical Runtime Recheck"
Cohesion: 0.10
Nodes (20): Answer, Artefak reproduksi, Bukti per temuan, F01: ada kontrol yang membedakan bug dari konfigurasi terbuka, F02: fallback rentan race, tetapi symlink biasa bukan bypass, F03: miss pertama, hit salah authority, dan control baru, F04: perilaku nyata, tetapi klasifikasi bug ditarik, F05: overflow diuji dengan Rust, termasuk mode teroptimisasi (+12 more)

### Community 47 - "Cache Store and Purge"
Cohesion: 0.25
Nodes (4): lock_shard(), Object, Shard, Store

### Community 48 - "Exploit Rule Categories"
Cohesion: 0.11
Nodes (16): Category, .ALL, Dotfiles, Php, Rce, Scanners, Spam, Sqli (+8 more)

### Community 49 - "Brand Assets"
Cohesion: 0.15
Nodes (15): conduit-app-icon.svg, Conduit brand assets, Ember palette, favicon.svg / favicon.ico, Geist typeface (SemiBold 600, Regular 400, Mono 400), Lanes mark, conduit-logo-*-ember.svg / conduit-mark-ember.svg, conduit-logo-horizontal.svg (+7 more)

### Community 50 - "Config Reload Watcher"
Cohesion: 0.15
Nodes (4): Reloader, snapshot(), Stamp, stamp()

### Community 51 - "WebSocket Soak Client"
Cohesion: 0.25
Nodes (11): client(), shorts(), connect(), echo(), frame(), long_lived(), read_frame(), serve() (+3 more)

### Community 52 - "Cacheability and Variants"
Cohesion: 0.16
Nodes (12): meta, eligibility(), keys_and_variants(), req(), resp(), site(), SiteCache, stored() (+4 more)

### Community 53 - "Configuration Reference and nginx Mapping"
Cohesion: 0.16
Nodes (18): 11. TLS, ACME and client certificates, `access_log`, ACME, Admin endpoints (/metrics, /healthz, /status, /cache/purge), Cache-Status header (RFC 9211), certbot, Certificate selection, conduit check command (+10 more)

### Community 54 - "Typed Plugin Configuration"
Cohesion: 0.12
Nodes (12): Upstream HTTP version policy, Upstream protocol behavior, BackendConfig, UpstreamHttpVersion, Auto, Http1, Http2, UpstreamRef (+4 more)

### Community 55 - "Admin API"
Cohesion: 0.21
Nodes (6): Admin, bad(), reply(), service(), unescape(), unix()

### Community 56 - "CrowdSec Decision Table Synchronization"
Cohesion: 0.15
Nodes (8): Applied, CrowdSecPlugin, now(), parse_into(), Plan, pull(), site_plugin_refuses_held_decisions(), sync()

### Community 57 - "Exploit Scan Engine"
Cohesion: 0.20
Nodes (12): contains(), decode_query(), decoding(), Engine, fuzz_check(), has_escape(), needles_never_hide_a_match(), percent_decode() (+4 more)

### Community 58 - "HTTP2 Pool Protocol Selection"
Cohesion: 0.15
Nodes (8): Protocol isolation and HTTP/2 health checks, HTTP/2 connection stream limit, Cleartext HTTP/2 prior knowledge, HealthCheckConfig, ProbeType, Http, Tcp, Probe

### Community 59 - "Certificate Store Selection"
Cohesion: 0.17
Nodes (4): F08: Wildcard certificate override and coverage corrected, CertKey, CertResolver, CertStore

### Community 60 - "Trusted Client IP Resolution"
Cohesion: 0.26
Nodes (9): canonical(), Cidr, cidr_contains(), CLOUDFLARE, ip(), RealIp, recursive_skips_trusted_hops(), trusted_peer_header_used() (+1 more)

### Community 61 - "Example Site Settings"
Cohesion: 0.12
Nodes (17): default_ttl, stale_if_error, ca, sites, acme, cache, compression, force_https (+9 more)

### Community 62 - "Stability Tests and Soak Evidence"
Cohesion: 0.14
Nodes (16): Daemon readiness smoke (2026-11-10: 255,136 requests, 2 upgrades), Graceful binary upgrade (socket handover), Graceful upgrade test, upgrade_with_open_websockets test, Panic and misbehaving-client audit, 24-hour soak (2026-11-08/09: 56.2 million requests), Unique soak archives preserve prior output and runtime files, Soak reload, backend flap, certificate and access-log rotation schedule (+8 more)

### Community 63 - "CrowdSec Decision Storage"
Cohesion: 0.18
Nodes (10): Apply<'_, S>, Delta, lookup_cost(), Net, V4, V6, nets(), parse_net() (+2 more)

### Community 64 - "Rule Actions and Header Templates"
Cohesion: 0.14
Nodes (13): Action, Deny, Proxy, Redirect, Respond, Part, Host, Lit (+5 more)

### Community 65 - "Configuration Reference and nginx Mapping"
Cohesion: 0.20
Nodes (15): 6. Match conditions, deny action, file action (in-memory file response), Responses conduit generates (status table, HTML status page), Header conditions, Path normalization, proxy action (routing and path rewrite), redirect action and placeholders (+7 more)

### Community 66 - "Runtime Logger and Filtering"
Cohesion: 0.22
Nodes (4): benign(), chatty(), Logger, only_the_pool_race_is_benign()

### Community 67 - "Block Exploits Plugin"
Cohesion: 0.19
Nodes (10): attacks_are_caught_by_their_rule(), block_and_log_modes(), BlockExploits, detect(), detect_with(), Hit, may_log(), ordinary_requests_pass() (+2 more)

### Community 68 - "Plugin Admission and Concurrency"
Cohesion: 0.23
Nodes (9): Held, Local, Step, Answer, Check, Pass, acme_response(), check_client_cert() (+1 more)

### Community 69 - "Proxy Response Status and Headers"
Cohesion: 0.25
Nodes (15): acme_challenge(), acme_challenge_from_memory(), answer(), body_limit_and_default_site(), client_certificates(), host_parsing(), https_redirect_runs_before_plugins(), plugins_see_normalized_path_and_client() (+7 more)

### Community 70 - "Cache Store and Purge"
Cohesion: 0.17
Nodes (8): key(), purge(), push_framed(), Stats, stats(), storage_round_trip(), tracked(), render()

### Community 71 - "Example Files and Basic Auth"
Cohesion: 0.13
Nodes (15): htpasswd, match, match, root, strip_prefix, try_files, method, path (+7 more)

### Community 72 - "Configuration Reference and nginx Mapping"
Cohesion: 0.14
Nodes (14): description, filter, name, onsuccess, 8. The other plugins, `basic_auth`, `block_exploits`, `body_replace` (+6 more)

### Community 73 - "Configuration Reference and nginx Mapping"
Cohesion: 0.26
Nodes (13): Cache purge endpoint, Freshness, revalidation, stale serving and cache lock, Cache storage rules (RFC 9111, stricter), `concurrency`, Fixed-size counting sketch, Match conditions (match: block), Coming from nginx: directive equivalents, Plugin chain and suggested order (+5 more)

### Community 74 - "Historical Static Audit"
Cohesion: 0.13
Nodes (14): Answer, Arsitektur yang terbentuk, Audit mendalam Conduit, Cakupan dan batas kejujuran, Cara membaca paket audit, Invarian yang benar-benar penting, Outcome, Pemeriksaan dokumentasi terkini (+6 more)

### Community 75 - "ACME Account Management"
Cohesion: 0.27
Nodes (11): at(), Connected, Due, At, Now, Manager, random_unit(), unix() (+3 more)

### Community 76 - "CrowdSec AppSec Plugin"
Cohesion: 0.17
Nodes (9): AppSec, appsec_request_shape(), forwarded(), http_version(), Verdict, Allow, Block, verdict_status() (+1 more)

### Community 77 - "Upstream Balancing Configuration"
Cohesion: 0.18
Nodes (11): HashKey, ClientIp, Cookie, Header, None, Path, Uri, least_conn_prefers_idle_backends() (+3 more)

### Community 78 - "ACME Key and CSR"
Cohesion: 0.15
Nodes (9): acme_err(), challenge_answer(), challenge_tokens_are_withdrawn(), failure_detail(), order(), Published, storage_lock_is_exclusive(), StorageLock (+1 more)

### Community 79 - "HTTP2 Pool Protocol Selection"
Cohesion: 0.20
Nodes (13): 4. Upstream pools, Active health checks, Backend DNS resolution at load time, Backend selection with fallback, Backends and DNS, Balancing, Choosing a backend, Passive ejection (+5 more)

### Community 80 - "TLS Lifecycle and Hot Reload"
Cohesion: 0.18
Nodes (9): Cache purge endpoint (POST /cache/purge), conduit check (config validation), Hot reload without dropped connections, Production rollout began in November 2026, Prometheus metrics, Certificate rotation and reload under load test, Apache License 2.0, conduit reverse proxy (+1 more)

### Community 82 - "Global Config Sections"
Cohesion: 0.21
Nodes (8): F05: Duration multiplication overflow rejected, durations(), parse_duration(), parse_size(), Raw, Num, Str, sizes()

### Community 83 - "mTLS E2E Tests"
Cohesion: 0.31
Nodes (5): Ca, cert_builder(), ec_key(), mtls_get(), MtlsReply

### Community 84 - "Access Log Entry Encoding"
Cohesion: 0.26
Nodes (9): Entry, entry(), full_shard_drops_and_counts(), hostile_values_stay_one_line(), ip(), json_line(), line(), millis() (+1 more)

### Community 85 - "Static File Configuration Tests"
Cohesion: 0.22
Nodes (9): FilesConfig, body(), Dir, files(), lookup(), precompressed_try_files_browse_prefix(), serving_and_confinement(), settings_are_checked() (+1 more)

### Community 86 - "ACME Key and CSR"
Cohesion: 0.29
Nodes (8): asn1_unix(), bundle(), csr(), csr_names_every_host(), inspect(), issued(), new_key(), stored_bundle_loads_and_identifies()

### Community 87 - "Static File Lookup and Listings"
Cohesion: 0.21
Nodes (5): Accepted, Encoding, Br, Gzip, Zstd

### Community 88 - "Parity Script"
Cohesion: 0.31
Nodes (4): category(), main(), make_browser(), run()

### Community 89 - "Parity Script"
Cohesion: 0.29
Nodes (5): fetch(), hosts_from(), main(), run(), read_cases()

### Community 91 - "Mixed Traffic Soak Harness"
Cohesion: 0.42
Nodes (10): cert(), event(), every(), flap(), keep_log(), rotate_log(), sample(), soak.sh script (+2 more)

### Community 92 - "Rule Action Config"
Cohesion: 0.31
Nodes (10): ActionConfig, Deny, File, Proxy, Redirect, Respond, ActionRaw, FileConfig (+2 more)

### Community 93 - "TLS OpenSSL Bindings"
Cohesion: 0.22
Nodes (5): ask_client_certs(), ask_for_certificate(), client_hello_sni(), sni_extension(), sni_from_extension()

### Community 96 - "Example TLS and ACME"
Cohesion: 0.20
Nodes (10): directory, email, storage, cert, key, name, tls, acme (+2 more)

### Community 97 - "Example CrowdSec and AppSec"
Cohesion: 0.20
Nodes (10): body_limit, failure_action, timeout, url, crowdsec, api_key_file, appsec, lapi_url (+2 more)

### Community 98 - "Parity Script"
Cohesion: 0.27
Nodes (3): accepted_bits(), main(), normalize()

### Community 99 - "Throughput Benchmark Harness"
Cohesion: 0.36
Nodes (8): baseline_rate(), cpu_ticks(), load(), peak_rss_kb(), pids_of(), run(), bench.sh script, wait_port()

### Community 100 - "Typed Plugin Configuration"
Cohesion: 0.24
Nodes (5): AcmeSection, ServerHeader, Off, Value, acme_plan()

### Community 101 - "Upstream Balancing Configuration"
Cohesion: 0.22
Nodes (8): Balance, Hash, LeastConn, Random, RoundRobin, default_h2_streams(), PassiveConfig, PoolConfig

### Community 102 - "Certificate Store Selection"
Cohesion: 0.31
Nodes (6): CertConfig, client_trust_from_file(), mismatched_key_rejected(), selection_order(), self_signed(), write_pair()

### Community 103 - "Upstream Passive Ejection"
Cohesion: 0.22
Nodes (3): load_balancer(), LoadBalancer<S>, Select

### Community 105 - "Static Files and Response Compression"
Cohesion: 0.22
Nodes (5): Static-file conditional requests and single byte ranges, Static file server (files:), Static file-server tests, RFC 9110 (HTTP semantics), Upstream Brotli finalization (b5dea5a, PR 1021, issue 1013)

### Community 106 - "RFC 3339 Time Formatting"
Cohesion: 0.39
Nodes (6): civil_from_days(), known_instants(), rfc3339(), rfc3339_millis(), seconds_part(), write_rfc3339_millis()

### Community 107 - "Exploit Rule Parsing"
Cohesion: 0.28
Nodes (5): all_rules(), categories_mask(), cost(), tautologies(), tautology()

### Community 108 - "Example Defaults"
Cohesion: 0.25
Nodes (8): defaults, compression, connect_timeout, keepalive_timeout, max_body_size, read_timeout, upstream_idle_timeout, write_timeout

### Community 109 - "Documentation and Roadmap Links"
Cohesion: 0.25
Nodes (8): 7. The `rules` plugin, Actions, `deny`, Examples, `file`, `proxy`, `redirect`, `respond`

### Community 110 - "Proxy Features and Architecture"
Cohesion: 0.25
Nodes (5): headscale / Tailscale control protocol, Idle, slow, and malformed client tests, RFC 8441 (HTTP/2 WebSocket), Protocol upgrade modes (upgrade: websocket | any | off), WebSocket and protocol-upgrade tests

### Community 111 - "CrowdSec IP Bit Masks"
Cohesion: 0.25
Nodes (6): Bits, .WIDTH, u128, .WIDTH, u32, .WIDTH

### Community 112 - "HTTP2 Streaming and Tests"
Cohesion: 0.29
Nodes (3): CrowdSec, docker(), Pebble

### Community 113 - "Documentation and Roadmap Links"
Cohesion: 0.29
Nodes (7): 3. Global sections, `admin`, `cache`, `defaults`, `listen`, `server`, `tls`

### Community 114 - "Alternating Binary Comparison Harness"
Cohesion: 0.48
Nodes (5): config(), cpu_ticks(), load(), ab.sh script, wait_port()

### Community 116 - "Static File Lookup and Listings"
Cohesion: 0.33
Nodes (4): Candidate, Part, Lit, Path

### Community 117 - "ACME Key and CSR"
Cohesion: 0.33
Nodes (4): cert_id(), lifetime_renewal(), renewal_from_lifetime(), Stored

### Community 118 - "CrowdSec AppSec Acquisition"
Cohesion: 0.33
Nodes (5): appsec_config, labels, type, listen_addr, source

### Community 119 - "Rule Actions and Header Templates"
Cohesion: 0.60
Nodes (5): StrMatch, Exact, Prefix, Regex, StrMatchRaw

### Community 120 - "CrowdSec Remediation Lookup"
Cohesion: 0.47
Nodes (3): Remedy, Ban, Captcha

### Community 121 - "Exploit Scan Engine"
Cohesion: 0.33
Nodes (3): Place, Path, Query

### Community 122 - "Static File Server"
Cohesion: 0.33
Nodes (6): Plan, Full, NotModified, Partial, PreconditionFailed, Unsatisfiable

### Community 125 - "Example Site Settings"
Cohesion: 0.40
Nodes (5): upstream, backends, balance, hash_key, verify_tls

### Community 126 - "Configuration Reference and nginx Mapping"
Cohesion: 0.40
Nodes (5): statics, expression, parsed, target, value

### Community 127 - "Post-fix Validation Memory"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: ok languns perbaiki semua temuan ya,, dan setiap temuan fix languns di uji e2e ya,, dan kalo udah semua uji e2e semunaya dan uji stabilitas dan performasnya dan uji sekali algi apakah sduah tertutup, Source Nodes

### Community 128 - "Regression Test Helpers"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: ini conduit dukunga ke upstream ini hanay di http/1 ya? http/2 belum?, Source Nodes

### Community 129 - "Regression Test Helpers"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: loh kerjakan bro,, karena ada tool internal butuh http/2 upstream,, semua fitur di pingora kalo bisa ngapain gk di kerjakan di conduit bro, Source Nodes

### Community 130 - "PGO Training Script"
Cohesion: 0.60
Nodes (3): fetch(), pgo-train.sh script, urls()

### Community 132 - "Config Parsing"
Cohesion: 0.40
Nodes (4): apply(), retain_sites(), load(), record()

### Community 133 - "Client Cert Verification"
Cohesion: 0.40
Nodes (4): ClientStatus, Invalid, Missing, Valid

### Community 134 - "Example Global Configuration"
Cohesion: 0.50
Nodes (4): real_ip, header, recursive, trusted

### Community 135 - "CrowdSec Log Acquisition"
Cohesion: 0.50
Nodes (3): filenames, labels, type

### Community 136 - "Audit Evidence and Documentation"
Cohesion: 0.50
Nodes (4): Current audit and validation index, Validation workload and deployment limits, Documentation and source provenance, Native checkout reproduction

### Community 138 - "Hidden Files and Ranges"
Cohesion: 0.83
Nodes (3): Hide, Name, Suffix

### Community 139 - "Command Line Interface"
Cohesion: 0.50
Nodes (4): Cli, Cmd, Check, Run

## Knowledge Gaps
- **138 isolated node(s):** `Off`, `Websocket`, `Path`, `Query`, `BasicAuth` (+133 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 746 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **50 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Pool` connect `Upstream Passive Ejection` to `Request Filter and Decide`, `Body Replace Plugin`, `Per-site Plugin Chain`, `Proxy Response Status and Headers`, `Upstream Balancing Configuration`, `Upstream Passive Ejection`, `Rule Actions and Header Templates`, `Runtime Site Compilation`, `Upstream Balancing Configuration`, `Typed Plugin Configuration`, `Upstream Passive Ejection`?**
  _High betweenness centrality (0.036) - this node is a cross-community bridge._
- **Are the 11 inferred relationships involving `http()` (e.g. with `file_descriptors_run_out()` and `idle_and_malformed_clients()`) actually correct?**
  _`http()` has 11 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Off`, `Websocket`, `Path` to the rest of the system?**
  _138 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Proxy Features and Architecture` be split into smaller, more focused modules?**
  _Cohesion score 0.05803571428571429 - nodes in this community are weakly interconnected._
- **Why does `conduit configuration reference` connect `Value Formats Docs` to `Configuration Reference and nginx Mapping`, `Configuration Reference and nginx Mapping`, `Documentation and Roadmap Links`, `HTTP2 Pool Protocol Selection`, `Documentation and Roadmap Links`, `Configuration Reference and nginx Mapping`, `Configuration Reference and nginx Mapping`?**
  _High betweenness centrality (0.026) - this node is a cross-community bridge._
- **Should `Forward and Basic Auth` be split into smaller, more focused modules?**
  _Cohesion score 0.06656426011264721 - nodes in this community are weakly interconnected._
- **Why does `RequestView` connect `Rule Actions and Header Templates` to `Forward and Basic Auth`, `Block Exploits Detector`, `Block Exploits Plugin`, `Plugin Admission and Concurrency`, `Per-site Plugin Chain`, `Proxy Response Status and Headers`, `Rate and Concurrency Limits`, `Body Replace Plugin`, `CrowdSec Client`, `Cache Storage Globals`, `Static File Server`, `Cacheability and Variants`, `CrowdSec Decision Table Synchronization`, `Exploit Scan Engine`?**
  _High betweenness centrality (0.026) - this node is a cross-community bridge._
## Planned management roadmap

The graph now includes [docs/roadmap.md](../docs/roadmap.md) and its links from README,
the technical guide and configuration reference. Management/UI/OIDC/SQLite capabilities
are marked **planned**, not implemented. The recorded requirements include headless by
default, optional Hallmark-designed UI, shared backend configuration contracts, expected
revisions, exclusive live-file write ownership for strict concurrency guarantees, journalled
apply/recovery, local auth and generic OIDC with Authentik/ZITADEL, and production acceptance
tests. Multi-node HA remains a separate design.


This update describes the working tree after commit `543a5d5`; it does not deploy code or
change the Krynox soak. Current code remains the tested proxy implementation. Earlier
validation JSON files retain their original snapshot hashes. See
[roadmap-validation.json](../docs/audit/roadmap-validation.json) for this documentation
update's link checks, source hashes and graph health. Historical audit snapshots and vendored
Pingora AST remain outside the configured graph scope.
