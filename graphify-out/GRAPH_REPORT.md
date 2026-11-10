# Graph Report - conduit  (2026-11-10)

## Corpus Check
- 74 files · ~141,104 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 14 file(s) not represented in the graph (top: (none) 9, .ico 1, .logrotate 1)

## Summary
- 2521 nodes · 5809 edges · 184 communities (133 shown, 51 thin omitted)
- Extraction: 97% EXTRACTED · 3% INFERRED · 0% AMBIGUOUS · INFERRED: 199 edges (avg confidence: 0.82)
- Token usage: host-agent semantic update not measured; AST extraction is local.

## Graph Freshness
- Built from commit: `a6b2bb91`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- End-to-End Tests
- Forward and Basic Auth
- Proxy Features and Architecture
- Rule Actions and Header Templates
- Rate and Concurrency Limits
- Proxy Response Status and Headers
- Pingora Fork and Stability Patches
- HTTP2 Streaming and Tests
- Per-site Plugin Chain
- CrowdSec Client
- Plugin Admission and Concurrency
- Runtime Site Compilation
- Proxy Session Hooks
- Cache Storage Globals
- Static File Server
- Body Replace Plugin
- HTTP End-to-End Scenarios
- Upstream Health Service
- Config Parsing
- Proxy Comparison Script
- Proxy Comparison Script
- Prometheus Metrics Registry
- ACME Certificate Service
- Main Entry and Listeners
- TLS OpenSSL Bindings
- Upstream Passive Ejection
- Block Exploits Detector
- CrowdSec Decision Storage
- Config Reload Watcher
- Access Log Writer Shards
- Typed Plugin Configuration
- Request Filter and Decide
- Proxy Route Rewriting
- Access Log File IO
- Parity Script
- Example Plugin Settings
- Example Site Settings
- Outbound HTTP Client
- Exploit Rule Categories
- CrowdSec Stream Deserializer
- TLS Lifecycle and Hot Reload
- Historical Runtime Recheck
- Brand Assets
- WebSocket Soak Client
- Cacheability and Variants
- Global Config Sections
- Static File Lookup and Listings
- CrowdSec Decision Table Synchronization
- Exploit Scan Engine
- Cache Lookup and Purge Hooks
- Example Global Configuration
- Stability Tests and Soak Evidence
- Cache Store and Purge
- Plugin Configuration Types
- Trusted Client IP Resolution
- E2E HTTP Helpers
- Global Config Sections
- CrowdSec Remediation Lookup
- Health Probes
- Example Upstreams and Health Checks
- Configuration Reference and nginx Mapping
- Proxy Features and Architecture
- Admin API
- Runtime Logger and Filtering
- Example Files and Basic Auth
- Outbound HTTP Client
- Fix Validation and Regressions
- Configuration Reference and nginx Mapping
- Historical Static Audit
- ACME Account Management
- CrowdSec AppSec Plugin
- Block Exploits Plugin
- ACME Certificate Issuance
- Configuration Reference and nginx Mapping
- Configuration Reference and nginx Mapping
- Access Log Entry Encoding
- Upstream Balancing Configuration
- Stability Tests and Soak Evidence
- Static Files and Response Compression
- RFC 3339 Time Formatting
- Proxy Response Decoration
- mTLS E2E Tests
- Static File Lookup and Listings
- ACME Key and CSR
- Configuration Reference and nginx Mapping
- Cache Hit Body Handling
- Parity Script
- End-to-End Tests
- Certificate Store Selection
- Pingora Backport Preparation
- Benchmark and Test Evidence
- CrowdSec and Authentication Limits
- Parity Stub Backend
- Mixed Traffic Soak Harness
- Rule Action Config
- Request Path Normalization
- Upstream Pool Compilation
- ACME HTTP Transport
- Example TLS and ACME
- Example CrowdSec and AppSec
- Example Global Configuration
- Throughput Benchmark Harness
- Certificate Store Selection
- Static File Configuration Tests
- TLS OpenSSL Bindings
- Cache Storage Round Trip
- Request Filter and Decide
- Example Upstreams and Health Checks
- Static File Lookup and Listings
- Parity Script
- Configuration Reference and nginx Mapping
- Configuration Reference and nginx Mapping
- Configuration Reference and nginx Mapping
- Proxy Features and Architecture
- Exploit Rule Parsing
- Rule Actions and Header Templates
- Proxy Response Status and Headers
- Example Defaults
- CrowdSec IP Bit Masks
- Exploit Scan Engine
- TLS OpenSSL Bindings
- E2E Process and Port Helpers
- Alternating Binary Comparison Harness
- Access Log Fuzz RNG
- Typed Plugin Configuration
- Static File Path Templates
- ACME Certificate Service
- CrowdSec AppSec Acquisition
- Fix Validation and Regressions
- CrowdSec Remediation Lookup
- End-to-End Tests
- HTTP2 Pool Protocol Selection
- Post-fix Validation Memory
- Regression Test Helpers
- Regression Test Helpers
- PGO Training Script
- Background Services and Reload State
- Protocol Upgrade Modes
- Proxy Response Decoration
- Client Cert Verification
- Upstream Backend Selection
- CrowdSec Log Acquisition
- Audit Origin Fixtures
- Hidden Files and Ranges
- Command Line Interface
- Client Cert Verification
- PGO Build Script
- Fuzz Runner Script
- Cargo Crates
- Release Build Script
- Soak Deploy Script
- Value Formats Docs

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
- `c4cce19: task_pipe HTTP/1 task channel` --semantically_similar_to--> `JSON access log (per-worker buffers)`  [INFERRED] [semantically similar]
  vendor/README.md → docs/technical.md
- `Matriks penutupan` --references--> `request()`  [INFERRED]
  docs/audit/FIX_VALIDATION.md → docs/audit/recheck_runtime.py
- `Upstream HTTP version policy` --conceptually_related_to--> `UpstreamHttpVersion`  [INFERRED]
  docs/configuration.md → src/config.rs

## Import Cycles
- 2-file cycle: `src/proxy.rs -> src/tls.rs -> src/proxy.rs`
- 2-file cycle: `src/crowdsec.rs -> src/plugin.rs -> src/crowdsec.rs`
- 2-file cycle: `src/plugin.rs -> src/replace.rs -> src/plugin.rs`
- 2-file cycle: `src/metrics.rs -> src/runtime.rs -> src/metrics.rs`
- 2-file cycle: `src/limits.rs -> src/plugin.rs -> src/limits.rs`
- 2-file cycle: `src/proxy.rs -> src/upstream.rs -> src/proxy.rs`
- 3-file cycle: `src/proxy.rs -> src/runtime.rs -> src/tls.rs -> src/proxy.rs`
- 3-file cycle: `src/metrics.rs -> src/tls.rs -> src/proxy.rs -> src/metrics.rs`
- 3-file cycle: `src/crowdsec.rs -> src/proxy.rs -> src/plugin.rs -> src/crowdsec.rs`
- 3-file cycle: `src/crowdsec.rs -> src/proxy.rs -> src/runtime.rs -> src/crowdsec.rs`
- 3-file cycle: `src/crowdsec.rs -> src/metrics.rs -> src/runtime.rs -> src/crowdsec.rs`
- 3-file cycle: `src/limits.rs -> src/plugin.rs -> src/replace.rs -> src/limits.rs`
- 3-file cycle: `src/proxy.rs -> src/runtime.rs -> src/upstream.rs -> src/proxy.rs`
- 3-file cycle: `src/metrics.rs -> src/runtime.rs -> src/upstream.rs -> src/metrics.rs`
- 3-file cycle: `src/plugin.rs -> src/upstream.rs -> src/proxy.rs -> src/plugin.rs`
- 3-file cycle: `src/proxy.rs -> src/rules.rs -> src/upstream.rs -> src/proxy.rs`
- 4-file cycle: `src/plugin.rs -> src/route.rs -> src/upstream.rs -> src/proxy.rs -> src/plugin.rs`
- 4-file cycle: `src/proxy.rs -> src/rules.rs -> src/route.rs -> src/upstream.rs -> src/proxy.rs`
- 4-file cycle: `src/metrics.rs -> src/tls.rs -> src/proxy.rs -> src/runtime.rs -> src/metrics.rs`
- 4-file cycle: `src/metrics.rs -> src/tls.rs -> src/proxy.rs -> src/upstream.rs -> src/metrics.rs`

## Hyperedges (group relationships)
- **Verified fixes and preserved documented behavior** — docs_audit_fix_validation, docs_audit_fix_validation_f01, docs_audit_fix_validation_f02, docs_audit_fix_validation_f03, docs_audit_fix_validation_f04, docs_audit_fix_validation_f05, docs_audit_fix_validation_f06, docs_audit_fix_validation_f07, docs_audit_fix_validation_f08, docs_audit_fix_validation_f09, docs_audit_fix_validation_f10 [EXTRACTED 1.00]
- **Inputs to the cache key that prevent cache poisoning** — docs_configuration_cache_key, docs_configuration_forward_auth, docs_configuration_mtls, docs_configuration_trusted_proxy_headers, docs_configuration_proxy_action, docs_technical_body_replace [EXTRACTED 1.00]
- **Plugins that can answer (end) the request** — docs_configuration_rules_plugin, docs_technical_ip_filter, docs_configuration_crowdsec_plugin, docs_configuration_rate_limit, docs_configuration_concurrency, docs_technical_basic_auth, docs_configuration_forward_auth, docs_technical_block_exploits [EXTRACTED 1.00]
- **Phase three core stability validation** — docs_technical_upstream_flapping_test, docs_technical_soak_24h_results, docs_technical_fuzzing, docs_technical_panic_audit, docs_technical_open_websockets_upgrade_test, docs_technical_parity_test [EXTRACTED 1.00]
- **Soak harness evidence and failure attribution** — docs_technical_soak_events, docs_technical_soak_samples, docs_technical_soak_report, docs_technical_soak_archives, docs_technical_soak_upgrade_failure_guard [EXTRACTED 1.00]
- **Pingora ten performance patches** — vendor_readme_patch_79bf391, vendor_readme_patch_32d98ca, vendor_readme_patch_ecef83b, vendor_readme_patch_9586c10, vendor_readme_patch_c448eff, vendor_readme_patch_9c64421, vendor_readme_patch_3357c9e, vendor_readme_patch_bb3859d, vendor_readme_patch_c4cce19, vendor_readme_patch_0a6aee4 [EXTRACTED 1.00]
- **Backend failure handling (selection, retries, ejection, health checks)** — docs_configuration_backend_selection, docs_configuration_retries, docs_configuration_passive_ejection, docs_configuration_active_health_checks, docs_configuration_upstream_pools [INFERRED 0.85]

## Communities (184 total, 51 thin omitted)

### Community 0 - "End-to-End Tests"
Cohesion: 0.06
Nodes (43): Answer, auth_service(), body_exchange(), body_upstream(), BOUNCER_KEY, cache_upstream(), Counts, CROWDSEC_TAG (+35 more)

### Community 1 - "Forward and Basic Auth"
Cohesion: 0.07
Nodes (34): basic(), basic_auth_flow(), basic_auth_settings(), BasicAuth, cache_key(), CACHE_KEY, check(), config() (+26 more)

### Community 2 - "Proxy Features and Architecture"
Cohesion: 0.07
Nodes (34): block_exploits categories and rule ids, scripts/ab.sh A/B harness, Authentication tests, block_exploits plugin, block_exploits attack and ordinary-request tests, Stale cache revalidation and stale-if-error, Response-cache tests, clippy -D warnings (+26 more)

### Community 3 - "Rule Actions and Header Templates"
Cohesion: 0.09
Nodes (33): HeaderMatch, MatchConfig, RuleConfig, StrMatch, Exact, Prefix, Regex, StrMatchRaw (+25 more)

### Community 4 - "Rate and Concurrency Limits"
Cohesion: 0.08
Nodes (21): F07: Serialized admission for the same limiter key, ConcurrencyConfig, RateLimitConfig, carried(), CARRIED, chain(), check_status(), client() (+13 more)

### Community 5 - "Proxy Response Status and Headers"
Cohesion: 0.10
Nodes (32): acme_challenge(), acme_challenge_from_memory(), ACME_PREFIX, answer(), BASE, body_limit_and_default_site(), CACHE_STATUS, client_certificates() (+24 more)

### Community 6 - "Pingora Fork and Stability Patches"
Cohesion: 0.08
Nodes (28): Real-binary end-to-end tests, Getting started (build from source), CasedHeaderName, daemon_wait_for_ready, Docker release build includes vendored Pingora, FlushTrackingMock, TransportConnector idle connection pool, [patch.crates-io] wiring of pingora-* crates (+20 more)

### Community 7 - "HTTP2 Streaming and Tests"
Cohesion: 0.09
Nodes (25): Native H2 pool race regression, Source and binary validation provenance, Default HTTP/1 compatibility benchmark, HTTP/2 implementation and validation results, Final HTTP/2 release soak, 111 unit and 34 e2e passed, Historical upstream HTTP/1 answer, HTTP/2 feature completion evidence (+17 more)

### Community 8 - "Per-site Plugin Chain"
Cohesion: 0.09
Nodes (12): HeaderOps, HeadersConfig, IpFilterConfig, chain(), HeadersPlugin, ip_filter(), Ops, order_and_headers() (+4 more)

### Community 9 - "CrowdSec Client"
Cohesion: 0.08
Nodes (19): APPSEC_WARNED, BAN_PAGE, base_url(), EPOCH, go_duration_ms(), go_durations(), LAPI_DEADLINE, LAPI_MAX_RESPONSE (+11 more)

### Community 10 - "Plugin Admission and Concurrency"
Cohesion: 0.11
Nodes (13): Held, Chain, IpFilter, Local, Plugin, Step, Answer, Check (+5 more)

### Community 11 - "Runtime Site Compilation"
Cohesion: 0.12
Nodes (22): AcmeSection, render(), acme_dir(), acme_plan(), acme_site(), acme_sites(), acme_storage_per_directory(), build() (+14 more)

### Community 12 - "Proxy Session Hooks"
Cohesion: 0.11
Nodes (6): HTTP/1 upgrade with upstream policy, appsec(), Conduit, Ctx, forwards_upgrade(), UpgradeLog

### Community 13 - "Cache Storage Globals"
Cohesion: 0.08
Nodes (12): ENTRY_OVERHEAD, EVICTION, heuristic(), LOCK_AGE, LOCK, lock(), no_default_ttl(), outcome() (+4 more)

### Community 14 - "Static File Server"
Cohesion: 0.09
Nodes (20): CHUNK, conditionals_and_ranges(), content_type(), content_types(), etag_listed(), FileReply, http_date(), LIST_LIMIT (+12 more)

### Community 15 - "Body Replace Plugin"
Cohesion: 0.13
Nodes (20): BodyReplaceConfig, ReplacePair, Gate, Answer, Check, Pass, Replace, Route (+12 more)

### Community 16 - "HTTP End-to-End Scenarios"
Cohesion: 0.27
Nodes (29): acme_with_pebble(), auth_plugins(), backend_state(), block_exploits(), crowdsec_bouncer_and_appsec(), echo_upstream(), end_to_end(), file_server() (+21 more)

### Community 17 - "Upstream Health Service"
Cohesion: 0.09
Nodes (6): EPOCH, HealthService, load_balancer(), MAX_ITERATIONS, PROBE_HTTP, PROBE_TCP

### Community 18 - "Config Parsing"
Cohesion: 0.08
Nodes (14): AccessLogSection, acme_section(), example_config_builds(), FailureAction, Allow, Block, LETSENCRYPT, LETSENCRYPT_STAGING (+6 more)

### Community 19 - "Proxy Comparison Script"
Cohesion: 0.13
Nodes (18): configs(), hcm(), listener(), Container, container_name(), docker_rm(), main(), med() (+10 more)

### Community 20 - "Proxy Comparison Script"
Cohesion: 0.19
Nodes (12): main(), launch(), cert(), fingerprint(), main(), reloaded(), origin(), port() (+4 more)

### Community 21 - "Prometheus Metrics Registry"
Cohesion: 0.09
Nodes (8): result_index(), CLASSES, forget_site(), LATENCY_BUCKETS, lazy!, PROCESS, SiteMetrics, status_classes()

### Community 22 - "ACME Certificate Service"
Cohesion: 0.09
Nodes (9): Ari, backoff(), backoff_grows_and_caps(), CHALLENGES, HTTP_TIMEOUT, MAX_RESPONSE, SiteState, SiteStatus (+1 more)

### Community 23 - "Main Entry and Listeners"
Cohesion: 0.10
Nodes (8): check(), DEFAULT_CONFIG, GLOBAL, huge_pages(), listener(), main(), run(), socket_options()

### Community 25 - "Upstream Passive Ejection"
Cohesion: 0.14
Nodes (6): Lease, least_conn_prefers_idle_backends(), MemberStatus, now_ms(), Pool, Target

### Community 26 - "Block Exploits Detector"
Cohesion: 0.09
Nodes (17): AGENT, Attack, ATTACKS, ENGINE, every_rule_has_an_attack(), H, HANDLERS, HEADER (+9 more)

### Community 27 - "CrowdSec Decision Storage"
Cohesion: 0.15
Nodes (12): d(), Decision, Family, Family<K>, ip(), Map, Slot, Many (+4 more)

### Community 28 - "Config Reload Watcher"
Cohesion: 0.13
Nodes (8): apply(), retain_sites(), load(), record(), Reloader, snapshot(), Stamp, stamp()

### Community 29 - "Access Log Writer Shards"
Cohesion: 0.14
Nodes (6): AccessLog, lock(), OpenFile, Shard, Shards, Writer

### Community 30 - "Typed Plugin Configuration"
Cohesion: 0.11
Nodes (15): Upstream HTTP version policy, BackendConfig, default_h2_streams(), PassiveConfig, PoolConfig, SiteCacheConfig, UpstreamHttpVersion, Auto (+7 more)

### Community 31 - "Request Filter and Decide"
Cohesion: 0.20
Nodes (19): request(), Effects, check_client_cert(), Conn, decide(), files_and_cache(), header_str(), incoming_request_id() (+11 more)

### Community 32 - "Proxy Route Rewriting"
Cohesion: 0.16
Nodes (14): ProxyConfig, RewriteConfig, bad_configs(), decoded_bytes_are_encoded_again(), encode_path(), path_prefix(), regex_rewrite(), Rewrite (+6 more)

### Community 33 - "Access Log File IO"
Cohesion: 0.10
Nodes (12): ESCAPE, FLUSH_EVERY, HEX, HIGHS, int(), needs_escape(), ONES, POLL (+4 more)

### Community 34 - "Parity Script"
Cohesion: 0.13
Nodes (7): fetch(), hosts_from(), main(), run(), read_cases(), causes(), slope()

### Community 35 - "Example Plugin Settings"
Cohesion: 0.09
Nodes (22): except, skip_params, replace, types, max, copy_headers, url, response (+14 more)

### Community 36 - "Example Site Settings"
Cohesion: 0.09
Nodes (22): default_ttl, stale_if_error, ca, sites, acme, cache, compression, force_https (+14 more)

### Community 37 - "Outbound HTTP Client"
Cohesion: 0.18
Nodes (3): Client, IDLE, Reply

### Community 38 - "Exploit Rule Categories"
Cohesion: 0.11
Nodes (16): Category, .ALL, Dotfiles, Php, Rce, Scanners, Spam, Sqli (+8 more)

### Community 39 - "CrowdSec Stream Deserializer"
Cohesion: 0.13
Nodes (4): Apply, List, List<'_, '_, S>, &mut Apply<'_, S>

### Community 40 - "TLS Lifecycle and Hot Reload"
Cohesion: 0.12
Nodes (14): Built-in ACME client (HTTP-01, ARI), conduit check (config validation), Active HTTP/TCP health checks, Hot reload without dropped connections, Load-balancing algorithms (round_robin, random, least_conn, Ketama hash), Production rollout began in November 2026, RFC 9773 (ACME renewal information), Certificate rotation and reload under load test (+6 more)

### Community 41 - "Historical Runtime Recheck"
Cohesion: 0.10
Nodes (20): Answer, Artefak reproduksi, Bukti per temuan, F01: ada kontrol yang membedakan bug dari konfigurasi terbuka, F02: fallback rentan race, tetapi symlink biasa bukan bypass, F03: miss pertama, hit salah authority, dan control baru, F04: perilaku nyata, tetapi klasifikasi bug ditarik, F05: overflow diuji dengan Rust, termasuk mode teroptimisasi (+12 more)

### Community 42 - "Brand Assets"
Cohesion: 0.15
Nodes (15): conduit-app-icon.svg, Conduit brand assets, Ember palette, favicon.svg / favicon.ico, Geist typeface (SemiBold 600, Regular 400, Mono 400), Lanes mark, conduit-logo-*-ember.svg / conduit-mark-ember.svg, conduit-logo-horizontal.svg (+7 more)

### Community 43 - "WebSocket Soak Client"
Cohesion: 0.25
Nodes (11): client(), shorts(), connect(), echo(), frame(), long_lived(), read_frame(), serve() (+3 more)

### Community 44 - "Cacheability and Variants"
Cohesion: 0.16
Nodes (12): eligibility(), keys_and_variants(), req(), resp(), site(), SiteCache, stored(), variance() (+4 more)

### Community 45 - "Global Config Sections"
Cohesion: 0.19
Nodes (12): AppSecSection, CacheSection, Config, CrowdSecSection, Defaults, DefaultSite, five_seconds(), one_second() (+4 more)

### Community 46 - "Static File Lookup and Listings"
Cohesion: 0.17
Nodes (14): Files, Found, File, Listing, Missing, Redirect, Status, html_escape() (+6 more)

### Community 47 - "CrowdSec Decision Table Synchronization"
Cohesion: 0.15
Nodes (8): Applied, CrowdSecPlugin, now(), parse_into(), Plan, pull(), site_plugin_refuses_held_decisions(), sync()

### Community 48 - "Exploit Scan Engine"
Cohesion: 0.16
Nodes (9): after(), contains(), Engine, header_suspect(), Place, Header, Path, Query (+1 more)

### Community 49 - "Cache Lookup and Purge Hooks"
Cohesion: 0.16
Nodes (7): key(), purge(), push_framed(), Stats, stats(), storage_round_trip(), tracked()

### Community 50 - "Example Global Configuration"
Cohesion: 0.11
Nodes (17): access_log, path, queue_capacity, admin, listen, cache, lock_timeout, max_object_size (+9 more)

### Community 51 - "Stability Tests and Soak Evidence"
Cohesion: 0.12
Nodes (15): Daemon readiness smoke (2026-11-10: 255,136 requests, 2 upgrades), Fuzz properties: config, path, access_log, request, route, body_replace, block_exploits, Fuzzing (cargo-fuzz, src/fuzzing.rs), Graceful binary upgrade (socket handover), Graceful upgrade test, upgrade_with_open_websockets test, Panic and misbehaving-client audit, PGO release build (scripts/pgo.sh) (+7 more)

### Community 52 - "Cache Store and Purge"
Cohesion: 0.30
Nodes (4): lock_shard(), Object, Shard, Store

### Community 53 - "Plugin Configuration Types"
Cohesion: 0.11
Nodes (17): BlockExploitsConfig, CrowdSecPluginConfig, ExploitException, ExploitMode, Block, Log, PluginConfig, BasicAuth (+9 more)

### Community 54 - "Trusted Client IP Resolution"
Cohesion: 0.26
Nodes (9): canonical(), Cidr, cidr_contains(), CLOUDFLARE, ip(), RealIp, recursive_skips_trusted_hops(), trusted_peer_header_used() (+1 more)

### Community 55 - "E2E HTTP Helpers"
Cohesion: 0.25
Nodes (17): cpu_time(), exchange(), file_descriptors_run_out(), http(), https(), load_until(), open_fds(), parse() (+9 more)

### Community 56 - "Global Config Sections"
Cohesion: 0.15
Nodes (12): F05: Duration multiplication overflow rejected, from_map(), or_default(), yaml(), durations(), parse_duration(), parse_size(), Raw (+4 more)

### Community 57 - "CrowdSec Remediation Lookup"
Cohesion: 0.18
Nodes (10): Apply<'_, S>, Delta, lookup_cost(), Net, V4, V6, nets(), parse_net() (+2 more)

### Community 58 - "Health Probes"
Cohesion: 0.19
Nodes (6): HealthCheckConfig, ProbeType, Http, Tcp, Member, Probe

### Community 59 - "Example Upstreams and Health Checks"
Cohesion: 0.12
Nodes (16): url, weight, eject, failures, backends, backends, balance, hash_key (+8 more)

### Community 60 - "Configuration Reference and nginx Mapping"
Cohesion: 0.17
Nodes (15): ACME automatic certificates (HTTP-01, ARI), Admin endpoints (/metrics, /healthz, /status, /cache/purge), certbot, SNI certificate selection, conduit check command, Atomic configuration reload (SIGHUP / file change), file action (in-memory file response), Watched files (mtime, inode, size, follows symlinks) (+7 more)

### Community 61 - "Proxy Features and Architecture"
Cohesion: 0.13
Nodes (11): Commands, conduit: technical documentation, Deferred features (session tickets, OCSP, admin API, OpenTelemetry, captcha, DNS-01), HTTP/1.1 and HTTPS throughput analysis, HTTP/1.1 and HTTPS throughput, Phase 2 (ACME, CrowdSec, limits, mTLS, auth), Phase 1 targets, Roadmap (+3 more)

### Community 62 - "Admin API"
Cohesion: 0.28
Nodes (6): Admin, bad(), reply(), service(), unescape(), unix()

### Community 63 - "Runtime Logger and Filtering"
Cohesion: 0.22
Nodes (4): benign(), chatty(), Logger, only_the_pool_race_is_benign()

### Community 64 - "Example Files and Basic Auth"
Cohesion: 0.13
Nodes (15): htpasswd, match, match, root, strip_prefix, try_files, method, path (+7 more)

### Community 65 - "Outbound HTTP Client"
Cohesion: 0.15
Nodes (6): config(), ip_backends_only(), path(), route(), ROUTER, ROUTES

### Community 66 - "Fix Validation and Regressions"
Cohesion: 0.13
Nodes (14): F01: Single percent decoding policy and file lookup, F03: Cache authority includes the original port, F04: Documented fresh cache retention preserved, F06: Zero or unrepresentable watcher interval rejected, F09: Explicit Vary preserves unknown content codings, F10: README Server header contract aligned, Graph provenance and runtime-proof boundary, Local 600-second stability target (+6 more)

### Community 67 - "Configuration Reference and nginx Mapping"
Cohesion: 0.23
Nodes (13): deny action, Responses conduit generates (status table, HTML status page), Header conditions (present, equals, regex), Match conditions (match: block), proxy action (routing and path rewrite), redirect action and placeholders, respond action, RFC 3986 URI syntax (+5 more)

### Community 68 - "Historical Static Audit"
Cohesion: 0.13
Nodes (14): Answer, Arsitektur yang terbentuk, Audit mendalam Conduit, Cakupan dan batas kejujuran, Cara membaca paket audit, Invarian yang benar-benar penting, Outcome, Pemeriksaan dokumentasi terkini (+6 more)

### Community 69 - "ACME Account Management"
Cohesion: 0.27
Nodes (11): at(), Connected, Due, At, Now, Manager, random_unit(), unix() (+3 more)

### Community 70 - "CrowdSec AppSec Plugin"
Cohesion: 0.17
Nodes (9): AppSec, appsec_request_shape(), forwarded(), http_version(), Verdict, Allow, Block, verdict_status() (+1 more)

### Community 71 - "Block Exploits Plugin"
Cohesion: 0.21
Nodes (10): attacks_are_caught_by_their_rule(), block_and_log_modes(), BlockExploits, detect(), detect_with(), Hit, may_log(), ordinary_requests_pass() (+2 more)

### Community 72 - "ACME Certificate Issuance"
Cohesion: 0.15
Nodes (9): acme_err(), challenge_answer(), challenge_tokens_are_withdrawn(), failure_detail(), order(), Published, storage_lock_is_exclusive(), StorageLock (+1 more)

### Community 73 - "Configuration Reference and nginx Mapping"
Cohesion: 0.23
Nodes (14): conduit configuration reference (docs/configuration.md), Access log (JSON lines, background writer), Cache purge endpoint, Freshness, revalidation, stale serving and cache lock, Cache-Status header (RFC 9211), Cache storage rules (RFC 9111, stricter), default_site page for unknown hosts, Protocol upgrades (websocket, any, off) (+6 more)

### Community 74 - "Configuration Reference and nginx Mapping"
Cohesion: 0.24
Nodes (13): Authelia / Authentik / oauth2-proxy, Headers conduit adds to client responses, defaults section (timeouts, max_body_size, compression), force_https and hsts, forward_auth plugin, headers plugin (request/response set/remove), Client certificates (mTLS), Request id (x-request-id) (+5 more)

### Community 75 - "Access Log Entry Encoding"
Cohesion: 0.23
Nodes (9): Entry, entry(), full_shard_drops_and_counts(), hostile_values_stay_one_line(), ip(), json_line(), line(), millis() (+1 more)

### Community 76 - "Upstream Balancing Configuration"
Cohesion: 0.16
Nodes (12): Balance, Hash, LeastConn, Random, RoundRobin, HashKey, ClientIp, Cookie (+4 more)

### Community 77 - "Stability Tests and Soak Evidence"
Cohesion: 0.18
Nodes (10): Cache purge endpoint (POST /cache/purge), Prometheus metrics, Unique soak archives preserve prior output and runtime files, Soak reload, backend flap, certificate and access-log rotation schedule, Soak report: hourly medians, per-process RSS and disruption attribution, Soak process sampling every 30 seconds, Seven-minute soak smoke configuration (scale 40, upgrades 1 and 3), Soak test (scripts/soak, conduit-soak unit) (+2 more)

### Community 78 - "Static Files and Response Compression"
Cohesion: 0.15
Nodes (8): Static-file conditional requests and single byte ranges, Static file server (files:), Static file-server tests, rules proxy action (strip_prefix, add_prefix, rewrite), RFC 9110 (HTTP semantics), routes_and_body_replace end-to-end test, rules plugin, Upstream Brotli finalization (b5dea5a, PR 1021, issue 1013)

### Community 79 - "RFC 3339 Time Formatting"
Cohesion: 0.23
Nodes (7): ser_ts(), civil_from_days(), known_instants(), rfc3339(), rfc3339_millis(), seconds_part(), write_rfc3339_millis()

### Community 80 - "Proxy Response Decoration"
Cohesion: 0.19
Nodes (8): compressible(), compressible_types(), decorate(), final_status(), respond(), status_page(), upstream_error_status(), upstream_timeouts_are_504()

### Community 81 - "mTLS E2E Tests"
Cohesion: 0.31
Nodes (5): Ca, cert_builder(), ec_key(), mtls_get(), MtlsReply

### Community 82 - "Static File Lookup and Listings"
Cohesion: 0.19
Nodes (5): Accepted, Encoding, Br, Gzip, Zstd

### Community 83 - "ACME Key and CSR"
Cohesion: 0.29
Nodes (8): asn1_unix(), bundle(), csr(), csr_names_every_host(), inspect(), issued(), new_key(), stored_bundle_loads_and_identifies()

### Community 84 - "Configuration Reference and nginx Mapping"
Cohesion: 0.17
Nodes (11): description, filter, name, onsuccess, statics, expression, meta, parsed (+3 more)

### Community 86 - "Parity Script"
Cohesion: 0.31
Nodes (4): category(), main(), make_browser(), run()

### Community 87 - "End-to-End Tests"
Cohesion: 0.18
Nodes (7): config(), graceful_upgrade(), panic_text(), Ports, Proxy, tally(), wait_exit()

### Community 89 - "Pingora Backport Preparation"
Cohesion: 0.18
Nodes (9): HTTP/2 upstream: implementasi dan validasi, Integritas dan pemetaan, Pemakaian, Perbaikan dependency yang diperlukan, Performa, Perubahan implementasi, Stabilitas H2 upstream, Uji fungsional dan regresi (+1 more)

### Community 90 - "Benchmark and Test Evidence"
Cohesion: 0.20
Nodes (11): Apache httpd, scripts/bench.sh benchmark and regression gate, Cross-proxy cgroup CPU and anonymous-memory comparison, Benchmark gate with baseline-fixed rates and p99 tolerances, Caddy, Envoy, HAProxy, Comparison with other reverse proxies (scripts/compare.py) (+3 more)

### Community 91 - "CrowdSec and Authentication Limits"
Cohesion: 0.18
Nodes (8): Cloudflare (CDN in front of the origin), CrowdSec AppSec (WAF) checks, Built-in CrowdSec L7 bouncer, CrowdSec parser for conduits access log (deploy/crowdsec), CrowdSec full-loop tests, Pebble ACME test CA, ACME end-to-end test with Pebble, rate_limit plugin

### Community 93 - "Mixed Traffic Soak Harness"
Cohesion: 0.42
Nodes (10): cert(), event(), every(), flap(), keep_log(), rotate_log(), sample(), soak.sh script (+2 more)

### Community 94 - "Rule Action Config"
Cohesion: 0.31
Nodes (10): ActionConfig, Deny, File, Proxy, Redirect, Respond, ActionRaw, FileConfig (+2 more)

### Community 95 - "Request Path Normalization"
Cohesion: 0.33
Nodes (9): access_log(), n(), normalize(), normalize_decoded(), PathError, Invalid, percent_decode(), rejects_bad_input() (+1 more)

### Community 96 - "Upstream Pool Compilation"
Cohesion: 0.31
Nodes (9): hashing_is_sticky(), passive_ejection_and_panic_mode(), pick(), pool_cfg(), resolve(), Resolved, round_robin_spreads_and_skips_excluded(), unchanged_pool_is_reused() (+1 more)

### Community 98 - "Example TLS and ACME"
Cohesion: 0.20
Nodes (10): directory, email, storage, cert, key, name, tls, acme (+2 more)

### Community 99 - "Example CrowdSec and AppSec"
Cohesion: 0.20
Nodes (10): body_limit, failure_action, timeout, url, crowdsec, api_key_file, appsec, lapi_url (+2 more)

### Community 100 - "Example Global Configuration"
Cohesion: 0.20
Nodes (10): server, pid_file, shutdown_grace, shutdown_timeout, threads, transparent_huge_pages, upgrade_sock, upstream_keepalive_pool_size (+2 more)

### Community 101 - "Throughput Benchmark Harness"
Cohesion: 0.36
Nodes (8): baseline_rate(), cpu_ticks(), load(), peak_rss_kb(), pids_of(), run(), bench.sh script, wait_port()

### Community 102 - "Certificate Store Selection"
Cohesion: 0.29
Nodes (7): CertConfig, client_trust_from_file(), mismatched_key_rejected(), selection_order(), self_signed(), subject_as_rfc4514(), write_pair()

### Community 103 - "Static File Configuration Tests"
Cohesion: 0.29
Nodes (8): FilesConfig, body(), Dir, files(), precompressed_try_files_browse_prefix(), serving_and_confinement(), settings_are_checked(), tree()

### Community 104 - "TLS OpenSSL Bindings"
Cohesion: 0.22
Nodes (4): client_hello_sni(), distinguished_name(), sni_extension(), sni_from_extension()

### Community 106 - "Request Filter and Decide"
Cohesion: 0.28
Nodes (6): client_cert(), client_cert_headers(), client_scheme(), ip_text(), ip_text_matches_display(), ClientCert

### Community 107 - "Example Upstreams and Health Checks"
Cohesion: 0.25
Nodes (9): expect, fall, interval, path, rise, timeout, type, health_check (+1 more)

### Community 108 - "Static File Lookup and Listings"
Cohesion: 0.31
Nodes (5): F02: Descriptor fallback and pinned directory listing, open_beneath(), read_exact_at(), reply(), show()

### Community 109 - "Parity Script"
Cohesion: 0.28
Nodes (3): accepted_bits(), main(), normalize()

### Community 110 - "Configuration Reference and nginx Mapping"
Cohesion: 0.31
Nodes (8): Active health checks (http/tcp probes), Backend DNS resolution at load time, Backend selection with fallback, Load balancing (round_robin, random, least_conn, ketama hash), Passive ejection, Connect-failure retries, gRPC streaming and trailers, Upstream pools (named, URL, or inline)

### Community 111 - "Configuration Reference and nginx Mapping"
Cohesion: 0.33
Nodes (8): Cloudflare, Response compression (gzip, br, zstd, level 1), File responses (ETag, conditionals, ranges, MIME), nginx, Coming from nginx: directive equivalents, real_ip client address resolution, RFC 9110 HTTP Semantics, ip_filter plugin

### Community 112 - "Configuration Reference and nginx Mapping"
Cohesion: 0.39
Nodes (8): concurrency plugin, Fixed-size counting sketch, CrowdSec AppSec (WAF) inspection, CrowdSec bouncer section (stream-mode decisions), crowdsec plugin, Plugin chain and suggested order, rate_limit plugin (sliding window), State that survives a reload

### Community 113 - "Proxy Features and Architecture"
Cohesion: 0.22
Nodes (7): Benchmark, Comparison with other reverse proxies, samply CPU profile, Latency under load (fixed-rate p99 overhead), Memory notes, samply CPU profiler, Threads and runtime

### Community 114 - "Exploit Rule Parsing"
Cohesion: 0.28
Nodes (5): all_rules(), categories_mask(), cost(), tautologies(), tautology()

### Community 115 - "Rule Actions and Header Templates"
Cohesion: 0.25
Nodes (7): Part, Host, Lit, Path, RequestUri, Scheme, Template

### Community 116 - "Proxy Response Status and Headers"
Cohesion: 0.25
Nodes (4): AdminSection, ListenSection, ReloadStatus, Shared

### Community 117 - "Example Defaults"
Cohesion: 0.25
Nodes (8): defaults, compression, connect_timeout, keepalive_timeout, max_body_size, read_timeout, upstream_idle_timeout, write_timeout

### Community 118 - "CrowdSec IP Bit Masks"
Cohesion: 0.25
Nodes (6): Bits, .WIDTH, u128, .WIDTH, u32, .WIDTH

### Community 119 - "Exploit Scan Engine"
Cohesion: 0.36
Nodes (8): decode_query(), decoding(), fuzz_check(), has_escape(), needles_never_hide_a_match(), percent_decode(), strip_comments(), block_exploits()

### Community 120 - "TLS OpenSSL Bindings"
Cohesion: 0.29
Nodes (3): ask_client_certs(), ask_for_certificate(), ClientTrust

### Community 121 - "E2E Process and Port Helpers"
Cohesion: 0.29
Nodes (3): CrowdSec, docker(), Pebble

### Community 122 - "Alternating Binary Comparison Harness"
Cohesion: 0.48
Nodes (5): config(), cpu_ticks(), load(), ab.sh script, wait_port()

### Community 124 - "Typed Plugin Configuration"
Cohesion: 0.38
Nodes (3): ServerHeader, Off, Value

### Community 125 - "Static File Path Templates"
Cohesion: 0.33
Nodes (4): Candidate, Part, Lit, Path

### Community 126 - "ACME Certificate Service"
Cohesion: 0.33
Nodes (4): cert_id(), lifetime_renewal(), renewal_from_lifetime(), Stored

### Community 127 - "CrowdSec AppSec Acquisition"
Cohesion: 0.33
Nodes (5): appsec_config, labels, type, listen_addr, source

### Community 128 - "Fix Validation and Regressions"
Cohesion: 0.33
Nodes (6): Hasil suite dan pemeriksaan, Integritas dan batas kesimpulan, Matriks penutupan, Perbaikan dan validasi penutupan temuan, Performa release sebelum/sesudah, Stabilitas berkelanjutan

### Community 129 - "CrowdSec Remediation Lookup"
Cohesion: 0.47
Nodes (3): Remedy, Ban, Captcha

### Community 131 - "HTTP2 Pool Protocol Selection"
Cohesion: 0.40
Nodes (3): Protocol isolation and HTTP/2 health checks, HTTP/2 connection stream limit, Cleartext HTTP/2 prior knowledge

### Community 132 - "Post-fix Validation Memory"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: ok languns perbaiki semua temuan ya,, dan setiap temuan fix languns di uji e2e ya,, dan kalo udah semua uji e2e semunaya dan uji stabilitas dan performasnya dan uji sekali algi apakah sduah tertutup, Source Nodes

### Community 133 - "Regression Test Helpers"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: ini conduit dukunga ke upstream ini hanay di http/1 ya? http/2 belum?, Source Nodes

### Community 134 - "Regression Test Helpers"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: loh kerjakan bro,, karena ada tool internal butuh http/2 upstream,, semua fitur di pingora kalo bisa ngapain gk di kerjakan di conduit bro, Source Nodes

### Community 135 - "PGO Training Script"
Cohesion: 0.60
Nodes (3): fetch(), pgo-train.sh script, urls()

### Community 137 - "Protocol Upgrade Modes"
Cohesion: 0.40
Nodes (5): MtlsConfig, SiteConfig, UpgradeMode, Off, Websocket

### Community 139 - "Client Cert Verification"
Cohesion: 0.40
Nodes (4): ClientStatus, Invalid, Missing, Valid

### Community 141 - "CrowdSec Log Acquisition"
Cohesion: 0.50
Nodes (3): filenames, labels, type

### Community 143 - "Hidden Files and Ranges"
Cohesion: 0.83
Nodes (3): Hide, Name, Suffix

### Community 144 - "Command Line Interface"
Cohesion: 0.50
Nodes (4): Cli, Cmd, Check, Run

## Knowledge Gaps
- **135 isolated node(s):** `Pass`, `Host`, `Path`, `RequestUri`, `Scheme` (+130 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 661 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **51 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `conduit configuration reference (docs/configuration.md)` connect `Configuration Reference and nginx Mapping` to `Fix Validation and Regressions`, `Configuration Reference and nginx Mapping`, `Proxy Features and Architecture`, `Configuration Reference and nginx Mapping`, `Configuration Reference and nginx Mapping`, `Configuration Reference and nginx Mapping`, `Configuration Reference and nginx Mapping`, `Example Global Configuration`, `Configuration Reference and nginx Mapping`?**
  _High betweenness centrality (0.190) - this node is a cross-community bridge._
- **Are the 11 inferred relationships involving `http()` (e.g. with `file_descriptors_run_out()` and `idle_and_malformed_clients()`) actually correct?**
  _`http()` has 11 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Pass`, `Host`, `Path` to the rest of the system?**
  _135 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `End-to-End Tests` be split into smaller, more focused modules?**
  _Cohesion score 0.05683060109289618 - nodes in this community are weakly interconnected._
- **Why does `RequestView` connect `Plugin Admission and Concurrency` to `Forward and Basic Auth`, `Rule Actions and Header Templates`, `Rate and Concurrency Limits`, `Proxy Response Status and Headers`, `Block Exploits Plugin`, `Per-site Plugin Chain`, `CrowdSec Client`, `Cacheability and Variants`, `Cache Storage Globals`, `Static File Server`, `CrowdSec Decision Table Synchronization`, `Exploit Scan Engine`, `Static File Lookup and Listings`, `Body Replace Plugin`, `Block Exploits Detector`?**
  _High betweenness centrality (0.058) - this node is a cross-community bridge._
- **Should `Forward and Basic Auth` be split into smaller, more focused modules?**
  _Cohesion score 0.06994535519125683 - nodes in this community are weakly interconnected._
- **Why does `sites` connect `Example Site Settings` to `Example Files and Basic Auth`, `Example Global Configuration`, `Example Plugin Settings`?**
  _High betweenness centrality (0.032) - this node is a cross-community bridge._
## Resynchronization scope

Updated after the security fixes and HTTP/2 upstream implementation. Includes application AST, HTTP policy documentation, six H2 test cases and the documented Pingora backport. The graph describes the working tree, including uncommitted patches. Historical audit snapshots and raw results are excluded by .graphifyignore; vendored code is documented rather than AST-indexed. Earlier session memories are marked historical. Source hashes and graph health are in docs/audit/http2-graphify-resync.json. The user-reported 24-hour Krynox run is not verified by these local tests.
