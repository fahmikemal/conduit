# Graph Report - conduit  (2026-11-10)

## Corpus Check
- 75 files · ~143,724 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 14 file(s) not represented in the graph (top: (none) 9, .ico 1, .logrotate 1)

## Summary
- 2541 nodes · 5851 edges · 166 communities (118 shown, 48 thin omitted)
- Extraction: 97% EXTRACTED · 3% INFERRED · 0% AMBIGUOUS · INFERRED: 200 edges (avg confidence: 0.82)
- Token usage: host-agent semantic update not measured; AST extraction is local.

## Graph Freshness
- Built from commit: `03cae2bc`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- Upstream Passive Ejection
- End-to-End Tests
- Proxy Features and Architecture
- Forward and Basic Auth
- Pingora Fork and Stability Patches
- Outbound HTTP Client
- Rate and Concurrency Limits
- Config Parsing
- HTTP2 Streaming and Tests
- TLS OpenSSL Bindings
- HTTP2 Streaming and Tests
- Runtime Site Compilation
- Typed Plugin Configuration
- Global Config Sections
- Per-site Plugin Chain
- Static File Server
- Proxy Session Hooks
- ACME Certificate Service
- Access Log File IO
- Proxy Response Status and Headers
- Cache Storage Globals
- Body Replace Plugin
- Upstream Balancing Configuration
- Proxy Comparison Script
- CrowdSec Decision Storage
- Plugin Admission and Concurrency
- Proxy Comparison Script
- Main Entry and Listeners
- Prometheus Metrics Registry
- CrowdSec AppSec Plugin
- Block Exploits Detector
- Cache Store and Purge
- Example Upstreams and Health Checks
- Audit Evidence and Documentation
- CrowdSec Remediation Lookup
- Example Global Configuration
- CrowdSec Client
- ACME Key and CSR
- Parity Script
- Request Filter and Decide
- Example Plugin Settings
- Certificate Store Selection
- Access Log Writer Shards
- Proxy Route Rewriting
- Exploit Rule Categories
- Cacheability and Variants
- TLS Lifecycle and Hot Reload
- Historical Runtime Recheck
- Exploit Scan Engine
- Brand Assets
- WebSocket Soak Client
- Config Reload Watcher
- Cache Hit Body Handling
- Forward and Basic Auth
- Rule Actions and Header Templates
- Proxy Response Status and Headers
- CrowdSec Stream Deserializer
- Trusted Client IP Resolution
- Rule Actions and Header Templates
- ACME Account Management
- Example Site Settings
- Static File Lookup and Listings
- Configuration Reference and nginx Mapping
- Stability Tests and Soak Evidence
- Admin API
- Example Files and Basic Auth
- Configuration Reference and nginx Mapping
- Configuration Reference and nginx Mapping
- Configuration Reference and nginx Mapping
- Configuration Reference and nginx Mapping
- Proxy Features and Architecture
- Historical Static Audit
- Runtime Logger and Filtering
- Static File Lookup and Listings
- Rule Actions and Header Templates
- HTTP2 Pool Protocol Selection
- Proxy Response Decoration
- mTLS E2E Tests
- Proxy Response Status and Headers
- Access Log Entry Encoding
- Static File Lookup and Listings
- CrowdSec Decision Table Synchronization
- Block Exploits Plugin
- Proxy Features and Architecture
- CrowdSec and Authentication Limits
- Static Files and Response Compression
- E2E HTTP Helpers
- Mixed Traffic Soak Harness
- Rule Action Config
- Parity Script
- Example TLS and ACME
- Example CrowdSec and AppSec
- Parity Stub Backend
- RFC 3339 Time Formatting
- Throughput Benchmark Harness
- Static File Configuration Tests
- ACME HTTP Transport
- Parity Script
- Configuration Reference and nginx Mapping
- Proxy Features and Architecture
- Exploit Rule Parsing
- Example Defaults
- Pingora Backport Preparation
- CrowdSec IP Bit Masks
- Configuration Reference and nginx Mapping
- Documentation Validation and Provenance
- Stability Tests and Soak Evidence
- Alternating Binary Comparison Harness
- Access Log Fuzz RNG
- Cache Storage Round Trip
- CrowdSec AppSec Acquisition
- Fix Validation and Regressions
- Rule Actions and Header Templates
- Exploit Scan Engine
- End-to-End Tests
- Cache Lookup and Purge Hooks
- End-to-End Tests
- Example Site Settings
- Post-fix Validation Memory
- Regression Test Helpers
- Regression Test Helpers
- PGO Training Script
- Client Cert Verification
- Example Global Configuration
- CrowdSec Log Acquisition
- Audit Origin Fixtures
- Hidden Files and Ranges
- Command Line Interface
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
- `Upstream protocol and transport matrix` --conceptually_related_to--> `UpstreamHttpVersion`  [INFERRED]
  docs/configuration.md → src/config.rs
- `c4cce19: task_pipe HTTP/1 task channel` --semantically_similar_to--> `JSON access log (per-worker buffers)`  [INFERRED] [semantically similar]
  vendor/README.md → docs/technical.md
- `Matriks penutupan` --references--> `request()`  [INFERRED]
  docs/audit/FIX_VALIDATION.md → docs/audit/recheck_runtime.py

## Import Cycles
- 2-file cycle: `src/plugin.rs -> src/replace.rs -> src/plugin.rs`
- 2-file cycle: `src/crowdsec.rs -> src/plugin.rs -> src/crowdsec.rs`
- 2-file cycle: `src/limits.rs -> src/plugin.rs -> src/limits.rs`
- 2-file cycle: `src/metrics.rs -> src/runtime.rs -> src/metrics.rs`
- 2-file cycle: `src/proxy.rs -> src/tls.rs -> src/proxy.rs`
- 2-file cycle: `src/proxy.rs -> src/upstream.rs -> src/proxy.rs`
- 3-file cycle: `src/plugin.rs -> src/upstream.rs -> src/proxy.rs -> src/plugin.rs`
- 3-file cycle: `src/limits.rs -> src/plugin.rs -> src/replace.rs -> src/limits.rs`
- 3-file cycle: `src/crowdsec.rs -> src/proxy.rs -> src/plugin.rs -> src/crowdsec.rs`
- 3-file cycle: `src/proxy.rs -> src/runtime.rs -> src/tls.rs -> src/proxy.rs`
- 3-file cycle: `src/proxy.rs -> src/runtime.rs -> src/upstream.rs -> src/proxy.rs`
- 3-file cycle: `src/metrics.rs -> src/runtime.rs -> src/upstream.rs -> src/metrics.rs`
- 3-file cycle: `src/crowdsec.rs -> src/proxy.rs -> src/runtime.rs -> src/crowdsec.rs`
- 3-file cycle: `src/crowdsec.rs -> src/metrics.rs -> src/runtime.rs -> src/crowdsec.rs`
- 3-file cycle: `src/proxy.rs -> src/rules.rs -> src/upstream.rs -> src/proxy.rs`
- 3-file cycle: `src/metrics.rs -> src/tls.rs -> src/proxy.rs -> src/metrics.rs`
- 4-file cycle: `src/plugin.rs -> src/route.rs -> src/upstream.rs -> src/proxy.rs -> src/plugin.rs`
- 4-file cycle: `src/proxy.rs -> src/rules.rs -> src/route.rs -> src/upstream.rs -> src/proxy.rs`
- 4-file cycle: `src/plugin.rs -> src/rules.rs -> src/upstream.rs -> src/proxy.rs -> src/plugin.rs`
- 4-file cycle: `src/plugin.rs -> src/upstream.rs -> src/proxy.rs -> src/runtime.rs -> src/plugin.rs`

## Hyperedges (group relationships)
- **Verified fixes and preserved documented behavior** — docs_audit_fix_validation, docs_audit_fix_validation_f01, docs_audit_fix_validation_f02, docs_audit_fix_validation_f03, docs_audit_fix_validation_f04, docs_audit_fix_validation_f05, docs_audit_fix_validation_f06, docs_audit_fix_validation_f07, docs_audit_fix_validation_f08, docs_audit_fix_validation_f09, docs_audit_fix_validation_f10 [EXTRACTED 1.00]
- **Inputs to the cache key that prevent cache poisoning** — docs_configuration_cache_key, docs_configuration_forward_auth, docs_configuration_mtls, docs_configuration_trusted_proxy_headers, docs_configuration_proxy_action, docs_technical_body_replace [EXTRACTED 1.00]
- **Plugins that can answer (end) the request** — docs_configuration_rules_plugin, docs_technical_ip_filter, docs_configuration_crowdsec_plugin, docs_configuration_rate_limit, docs_configuration_concurrency, docs_technical_basic_auth, docs_configuration_forward_auth, docs_technical_block_exploits [EXTRACTED 1.00]
- **Phase three core stability validation** — docs_technical_upstream_flapping_test, docs_technical_soak_24h_results, docs_technical_fuzzing, docs_technical_panic_audit, docs_technical_open_websockets_upgrade_test, docs_technical_parity_test [EXTRACTED 1.00]
- **Soak harness evidence and failure attribution** — docs_technical_soak_events, docs_technical_soak_samples, docs_technical_soak_report, docs_technical_soak_archives, docs_technical_soak_upgrade_failure_guard [EXTRACTED 1.00]
- **Pingora ten performance patches** — vendor_readme_patch_79bf391, vendor_readme_patch_32d98ca, vendor_readme_patch_ecef83b, vendor_readme_patch_9586c10, vendor_readme_patch_c448eff, vendor_readme_patch_9c64421, vendor_readme_patch_3357c9e, vendor_readme_patch_bb3859d, vendor_readme_patch_c4cce19, vendor_readme_patch_0a6aee4 [EXTRACTED 1.00]
- **Backend failure handling (selection, retries, ejection, health checks)** — docs_configuration_backend_selection, docs_configuration_retries, docs_configuration_passive_ejection, docs_configuration_active_health_checks, docs_configuration_upstream_pools [INFERRED 0.85]

## Communities (166 total, 48 thin omitted)

### Community 0 - "Upstream Passive Ejection"
Cohesion: 0.06
Nodes (20): EPOCH, hashing_is_sticky(), Lease, load_balancer(), LoadBalancer<S>, MAX_ITERATIONS, Member, MemberStatus (+12 more)

### Community 1 - "End-to-End Tests"
Cohesion: 0.05
Nodes (46): Answer, auth_service(), body_exchange(), body_upstream(), BOUNCER_KEY, cache_upstream(), Counts, cpu_time() (+38 more)

### Community 2 - "Proxy Features and Architecture"
Cohesion: 0.07
Nodes (43): scripts/ab.sh A/B harness, Apache httpd, scripts/bench.sh benchmark and regression gate, Benchmark, Cross-proxy cgroup CPU and anonymous-memory comparison, Benchmark gate with baseline-fixed rates and p99 tolerances, Caddy, clippy -D warnings (+35 more)

### Community 3 - "Forward and Basic Auth"
Cohesion: 0.07
Nodes (29): basic(), basic_auth_flow(), basic_auth_settings(), BasicAuth, CACHE_KEY, check(), config(), Counters (+21 more)

### Community 4 - "Pingora Fork and Stability Patches"
Cohesion: 0.06
Nodes (33): Real-binary end-to-end tests, HTTP/1.1 and HTTPS throughput analysis, Stage A small optimizations, Getting started (build from source), prepare-vendor.sh script, CasedHeaderName, daemon_wait_for_ready, Docker release build includes vendored Pingora (+25 more)

### Community 5 - "Outbound HTTP Client"
Cohesion: 0.07
Nodes (19): F01: Single percent decoding policy and file lookup, access_log(), config(), ip_backends_only(), path(), route(), ROUTER, ROUTES (+11 more)

### Community 6 - "Rate and Concurrency Limits"
Cohesion: 0.08
Nodes (20): ConcurrencyConfig, RateLimitConfig, carried(), CARRIED, chain(), check_status(), client(), Concurrency (+12 more)

### Community 7 - "Config Parsing"
Cohesion: 0.06
Nodes (25): retain_sites(), AccessLogSection, acme_section(), BlockExploitsConfig, DefaultSite, example_config_builds(), ExploitException, ExploitMode (+17 more)

### Community 8 - "HTTP2 Streaming and Tests"
Cohesion: 0.08
Nodes (27): Brand assets in project documentation, Native H2 pool race regression, Source and binary validation provenance, Default HTTP/1 compatibility benchmark, HTTP/2 implementation and validation results, Final HTTP/2 release soak, 111 unit and 34 e2e passed, Current implementation validation (+19 more)

### Community 9 - "TLS OpenSSL Bindings"
Cohesion: 0.07
Nodes (10): ask_client_certs(), ask_for_certificate(), CertResolver, client_hello_sni(), distinguished_name(), sni_extension(), sni_from_extension(), SSL_set_session_id_context() (+2 more)

### Community 10 - "HTTP2 Streaming and Tests"
Cohesion: 0.21
Nodes (37): acme_with_pebble(), auth_plugins(), backend_state(), block_exploits(), config(), CrowdSec, crowdsec_bouncer_and_appsec(), docker() (+29 more)

### Community 11 - "Runtime Site Compilation"
Cohesion: 0.12
Nodes (31): Config, SiteConfig, UpgradeMode, Off, Websocket, render(), acme_dir(), acme_plan() (+23 more)

### Community 12 - "Typed Plugin Configuration"
Cohesion: 0.06
Nodes (27): Upstream HTTP version policy, Upstream protocol and transport matrix, BackendConfig, CrowdSecPluginConfig, PluginConfig, BasicAuth, BlockExploits, BodyReplace (+19 more)

### Community 13 - "Global Config Sections"
Cohesion: 0.09
Nodes (24): F05: Duration multiplication overflow rejected, apply(), AcmeSection, AppSecSection, CacheSection, CrowdSecSection, Defaults, FailureAction (+16 more)

### Community 14 - "Per-site Plugin Chain"
Cohesion: 0.09
Nodes (12): HeaderOps, HeadersConfig, IpFilterConfig, chain(), HeadersPlugin, ip_filter(), Ops, order_and_headers() (+4 more)

### Community 15 - "Static File Server"
Cohesion: 0.09
Nodes (21): CHUNK, conditionals_and_ranges(), content_type(), content_types(), etag_listed(), FileReply, http_date(), LIST_LIMIT (+13 more)

### Community 16 - "Proxy Session Hooks"
Cohesion: 0.12
Nodes (10): Conduit, Ctx, enable_compression(), forwards_upgrade(), header_str(), ip_text(), ip_text_matches_display(), pool_of() (+2 more)

### Community 17 - "ACME Certificate Service"
Cohesion: 0.08
Nodes (12): backoff(), backoff_grows_and_caps(), challenge_answer(), challenge_tokens_are_withdrawn(), CHALLENGES, HTTP_TIMEOUT, MAX_RESPONSE, SiteStatus (+4 more)

### Community 18 - "Access Log File IO"
Cohesion: 0.07
Nodes (14): ESCAPE, FLUSH_EVERY, HEX, HIGHS, int(), needs_escape(), ONES, OpenFile (+6 more)

### Community 19 - "Proxy Response Status and Headers"
Cohesion: 0.09
Nodes (20): ACME_PREFIX, BASE, CACHE_STATUS, each_worker_thread_has_its_own_pool_group(), final_status(), new_request_id(), request_ids(), split_target() (+12 more)

### Community 20 - "Cache Storage Globals"
Cohesion: 0.09
Nodes (12): ENTRY_OVERHEAD, EVICTION, heuristic(), LOCK_AGE, LOCK, lock(), no_default_ttl(), outcome() (+4 more)

### Community 21 - "Body Replace Plugin"
Cohesion: 0.12
Nodes (20): BodyReplaceConfig, ReplacePair, Gate, Answer, Check, Pass, Replace, Route (+12 more)

### Community 22 - "Upstream Balancing Configuration"
Cohesion: 0.09
Nodes (23): Balance, Hash, LeastConn, Random, RoundRobin, default_h2_streams(), HealthCheckConfig, PassiveConfig (+15 more)

### Community 23 - "Proxy Comparison Script"
Cohesion: 0.13
Nodes (18): configs(), hcm(), listener(), Container, container_name(), docker_rm(), main(), med() (+10 more)

### Community 24 - "CrowdSec Decision Storage"
Cohesion: 0.14
Nodes (11): Decision, Delta, Family, Family<K>, Map, Net, Sink, Slot (+3 more)

### Community 25 - "Plugin Admission and Concurrency"
Cohesion: 0.12
Nodes (9): Held, Chain, IpFilter, Plugin, Step, Answer, Check, Pass (+1 more)

### Community 26 - "Proxy Comparison Script"
Cohesion: 0.19
Nodes (12): main(), launch(), cert(), fingerprint(), main(), reloaded(), origin(), port() (+4 more)

### Community 27 - "Main Entry and Listeners"
Cohesion: 0.10
Nodes (8): check(), DEFAULT_CONFIG, GLOBAL, huge_pages(), listener(), main(), run(), socket_options()

### Community 28 - "Prometheus Metrics Registry"
Cohesion: 0.09
Nodes (8): result_index(), CLASSES, forget_site(), LATENCY_BUCKETS, lazy!, PROCESS, SiteMetrics, status_classes()

### Community 29 - "CrowdSec AppSec Plugin"
Cohesion: 0.10
Nodes (16): AppSec, appsec_request_shape(), base_url(), CrowdSecPlugin, forwarded(), http_version(), Plan, plan() (+8 more)

### Community 30 - "Block Exploits Detector"
Cohesion: 0.09
Nodes (20): AGENT, Attack, ATTACKS, attacks_are_caught_by_their_rule(), detect(), ENGINE, every_rule_has_an_attack(), H (+12 more)

### Community 31 - "Cache Store and Purge"
Cohesion: 0.19
Nodes (4): lock_shard(), Object, Shard, Store

### Community 32 - "Example Upstreams and Health Checks"
Cohesion: 0.09
Nodes (25): url, weight, expect, fall, interval, path, rise, timeout (+17 more)

### Community 33 - "Audit Evidence and Documentation"
Cohesion: 0.09
Nodes (24): F02: Descriptor fallback and pinned directory listing, F03: Cache authority includes the original port, F06: Zero or unrepresentable watcher interval rejected, F07: Serialized admission for the same limiter key, F09: Explicit Vary preserves unknown content codings, Graph provenance and runtime-proof boundary, Local 600-second stability target, Audit closure before the H2 feature (+16 more)

### Community 34 - "CrowdSec Remediation Lookup"
Cohesion: 0.12
Nodes (18): renewal_from_lifetime(), Apply<'_, S>, d(), go_duration_ms(), go_durations(), ip(), lookup_cost(), V4 (+10 more)

### Community 35 - "Example Global Configuration"
Cohesion: 0.08
Nodes (23): access_log, path, queue_capacity, admin, listen, cache, lock_timeout, max_object_size (+15 more)

### Community 36 - "CrowdSec Client"
Cohesion: 0.11
Nodes (13): APPSEC_WARNED, BAN_PAGE, EPOCH, LAPI_DEADLINE, LAPI_MAX_RESPONSE, LAPI_TIMEOUT, MAX_BODY, PRUNE_EVERY (+5 more)

### Community 37 - "ACME Key and CSR"
Cohesion: 0.15
Nodes (14): asn1_unix(), bundle(), cert_id(), csr(), csr_names_every_host(), failure_detail(), inspect(), issued() (+6 more)

### Community 38 - "Parity Script"
Cohesion: 0.15
Nodes (9): category(), main(), make_browser(), run(), fetch(), hosts_from(), main(), run() (+1 more)

### Community 39 - "Request Filter and Decide"
Cohesion: 0.22
Nodes (19): request(), Effects, appsec(), client_cert(), client_cert_headers(), client_scheme(), Conn, decide() (+11 more)

### Community 40 - "Example Plugin Settings"
Cohesion: 0.09
Nodes (22): except, skip_params, replace, types, max, copy_headers, url, response (+14 more)

### Community 41 - "Certificate Store Selection"
Cohesion: 0.13
Nodes (8): F08: Wildcard certificate override and coverage corrected, CertConfig, CertKey, client_trust_from_file(), mismatched_key_rejected(), selection_order(), self_signed(), write_pair()

### Community 42 - "Access Log Writer Shards"
Cohesion: 0.16
Nodes (5): AccessLog, lock(), Shard, Shards, Writer

### Community 43 - "Proxy Route Rewriting"
Cohesion: 0.17
Nodes (14): ProxyConfig, RewriteConfig, bad_configs(), decoded_bytes_are_encoded_again(), encode_path(), path_prefix(), regex_rewrite(), Rewrite (+6 more)

### Community 44 - "Exploit Rule Categories"
Cohesion: 0.11
Nodes (16): Category, .ALL, Dotfiles, Php, Rce, Scanners, Spam, Sqli (+8 more)

### Community 45 - "Cacheability and Variants"
Cohesion: 0.15
Nodes (13): eligibility(), key(), keys_and_variants(), push_framed(), req(), resp(), site(), SiteCache (+5 more)

### Community 46 - "TLS Lifecycle and Hot Reload"
Cohesion: 0.12
Nodes (14): Built-in ACME client (HTTP-01, ARI), conduit check (config validation), Active HTTP/TCP health checks, Hot reload without dropped connections, Load-balancing algorithms (round_robin, random, least_conn, Ketama hash), Production rollout began in November 2026, RFC 9773 (ACME renewal information), Certificate rotation and reload under load test (+6 more)

### Community 47 - "Historical Runtime Recheck"
Cohesion: 0.10
Nodes (20): Answer, Artefak reproduksi, Bukti per temuan, F01: ada kontrol yang membedakan bug dari konfigurasi terbuka, F02: fallback rentan race, tetapi symlink biasa bukan bypass, F03: miss pertama, hit salah authority, dan control baru, F04: perilaku nyata, tetapi klasifikasi bug ditarik, F05: overflow diuji dengan Rust, termasuk mode teroptimisasi (+12 more)

### Community 48 - "Exploit Scan Engine"
Cohesion: 0.18
Nodes (14): after(), contains(), decode_query(), decoding(), Engine, fuzz_check(), has_escape(), header_suspect() (+6 more)

### Community 49 - "Brand Assets"
Cohesion: 0.15
Nodes (15): conduit-app-icon.svg, Conduit brand assets, Ember palette, favicon.svg / favicon.ico, Geist typeface (SemiBold 600, Regular 400, Mono 400), Lanes mark, conduit-logo-*-ember.svg / conduit-mark-ember.svg, conduit-logo-horizontal.svg (+7 more)

### Community 50 - "WebSocket Soak Client"
Cohesion: 0.25
Nodes (11): client(), shorts(), connect(), echo(), frame(), long_lived(), read_frame(), serve() (+3 more)

### Community 51 - "Config Reload Watcher"
Cohesion: 0.16
Nodes (4): Reloader, snapshot(), Stamp, stamp()

### Community 52 - "Cache Hit Body Handling"
Cohesion: 0.13
Nodes (6): Hit, purge(), Stats, stats(), storage_round_trip(), Any

### Community 53 - "Forward and Basic Auth"
Cohesion: 0.18
Nodes (9): cache_key(), refused(), verify(), CheckRequest, Local, UpstreamEdits, acme_response(), check_client_cert() (+1 more)

### Community 54 - "Rule Actions and Header Templates"
Cohesion: 0.23
Nodes (14): HeaderMatch, MatchConfig, RuleConfig, bad_rules_are_rejected(), forgejo_browser_redirect(), header_presence(), Matcher, path_regex_on_normalized_path() (+6 more)

### Community 55 - "Proxy Response Status and Headers"
Cohesion: 0.12
Nodes (6): AcmeService, AdminSection, ListenSection, ReloadStatus, Shared, HealthService

### Community 57 - "Trusted Client IP Resolution"
Cohesion: 0.26
Nodes (9): canonical(), Cidr, cidr_contains(), CLOUDFLARE, ip(), RealIp, recursive_skips_trusted_hops(), trusted_peer_header_used() (+1 more)

### Community 58 - "Rule Actions and Header Templates"
Cohesion: 0.13
Nodes (14): Action, Deny, Proxy, Redirect, Respond, compile_headers(), Part, Host (+6 more)

### Community 59 - "ACME Account Management"
Cohesion: 0.16
Nodes (12): acme_err(), Ari, at(), Connected, Due, At, Now, lifetime_renewal() (+4 more)

### Community 60 - "Example Site Settings"
Cohesion: 0.12
Nodes (17): default_ttl, stale_if_error, ca, sites, acme, cache, compression, force_https (+9 more)

### Community 61 - "Static File Lookup and Listings"
Cohesion: 0.18
Nodes (14): Found, File, Listing, Missing, Redirect, Status, html_escape(), human_size() (+6 more)

### Community 62 - "Configuration Reference and nginx Mapping"
Cohesion: 0.26
Nodes (14): Authelia / Authentik / oauth2-proxy, Cloudflare, forward_auth plugin, headers plugin (request/response set/remove), Client certificates (mTLS), nginx, Coming from nginx: directive equivalents, Plugin chain and suggested order (+6 more)

### Community 63 - "Stability Tests and Soak Evidence"
Cohesion: 0.15
Nodes (15): Daemon readiness smoke (2026-11-10: 255,136 requests, 2 upgrades), Graceful binary upgrade (socket handover), Graceful upgrade test, upgrade_with_open_websockets test, Panic and misbehaving-client audit, 24-hour soak (2026-11-08/09: 56.2 million requests), Unique soak archives preserve prior output and runtime files, Soak reload, backend flap, certificate and access-log rotation schedule (+7 more)

### Community 64 - "Admin API"
Cohesion: 0.28
Nodes (6): Admin, bad(), reply(), service(), unescape(), unix()

### Community 65 - "Example Files and Basic Auth"
Cohesion: 0.13
Nodes (15): htpasswd, match, match, root, strip_prefix, try_files, method, path (+7 more)

### Community 66 - "Configuration Reference and nginx Mapping"
Cohesion: 0.14
Nodes (14): description, filter, name, onsuccess, statics, expression, meta, parsed (+6 more)

### Community 67 - "Configuration Reference and nginx Mapping"
Cohesion: 0.18
Nodes (15): F04: Documented fresh cache retention preserved, Admin endpoints (/metrics, /healthz, /status, /cache/purge), Cache purge endpoint, Freshness, revalidation, stale serving and cache lock, Cache storage rules (RFC 9111, stricter), concurrency plugin, Runtime snapshots on configuration reload, Fixed-size counting sketch (+7 more)

### Community 68 - "Configuration Reference and nginx Mapping"
Cohesion: 0.23
Nodes (14): conduit configuration reference (docs/configuration.md), Access log (JSON lines, background writer), Response compression (gzip, br, zstd, level 1), conduit check command, default_site page for unknown hosts, defaults section (timeouts, max_body_size, compression), File responses (ETag, conditionals, ranges, MIME), force_https and hsts (+6 more)

### Community 69 - "Configuration Reference and nginx Mapping"
Cohesion: 0.23
Nodes (13): deny action, Responses conduit generates (status table, HTML status page), Header conditions (present, equals, regex), Match conditions (match: block), proxy action (routing and path rewrite), redirect action and placeholders, respond action, RFC 3986 URI syntax (+5 more)

### Community 70 - "Proxy Features and Architecture"
Cohesion: 0.14
Nodes (11): Cache purge endpoint (POST /cache/purge), Stale cache revalidation and stale-if-error, Response-cache tests, pingora-cache, Prometheus metrics, rules proxy action (strip_prefix, add_prefix, rewrite), HTTP response cache (RFC 9111), RFC 9111 (shared cache) (+3 more)

### Community 71 - "Historical Static Audit"
Cohesion: 0.13
Nodes (14): Answer, Arsitektur yang terbentuk, Audit mendalam Conduit, Cakupan dan batas kejujuran, Cara membaca paket audit, Invarian yang benar-benar penting, Outcome, Pemeriksaan dokumentasi terkini (+6 more)

### Community 72 - "Runtime Logger and Filtering"
Cohesion: 0.24
Nodes (4): benign(), chatty(), Logger, only_the_pool_race_is_benign()

### Community 73 - "Static File Lookup and Listings"
Cohesion: 0.22
Nodes (8): Candidate, Files, Lookup, lookup(), Part, Lit, Path, with_query()

### Community 74 - "Rule Actions and Header Templates"
Cohesion: 0.20
Nodes (8): BytesMatch, Exact, Prefix, Regex, HeaderCond, Equals, Present, Regex

### Community 75 - "HTTP2 Pool Protocol Selection"
Cohesion: 0.20
Nodes (12): Active health checks (http/tcp probes), Backend DNS resolution at load time, Backend selection with fallback, Load balancing (round_robin, random, least_conn, ketama hash), Passive ejection, Connect-failure retries, gRPC streaming and trailers, Protocol isolation and HTTP/2 health checks (+4 more)

### Community 76 - "Proxy Response Decoration"
Cohesion: 0.18
Nodes (6): compressible(), compressible_types(), decorate(), has_body(), respond(), status_page()

### Community 77 - "mTLS E2E Tests"
Cohesion: 0.30
Nodes (6): Ca, cert_builder(), ec_key(), mtls_get(), MtlsReply, mutual_tls()

### Community 78 - "Proxy Response Status and Headers"
Cohesion: 0.30
Nodes (14): acme_challenge(), acme_challenge_from_memory(), answer(), body_limit_and_default_site(), client_certificates(), host_parsing(), https_redirect_runs_before_plugins(), plugins_see_normalized_path_and_client() (+6 more)

### Community 79 - "Access Log Entry Encoding"
Cohesion: 0.26
Nodes (9): Entry, entry(), full_shard_drops_and_counts(), hostile_values_stay_one_line(), ip(), json_line(), line(), millis() (+1 more)

### Community 80 - "Static File Lookup and Listings"
Cohesion: 0.19
Nodes (5): Accepted, Encoding, Br, Gzip, Zstd

### Community 81 - "CrowdSec Decision Table Synchronization"
Cohesion: 0.21
Nodes (6): Applied, Apply, List, now(), parse_into(), sync()

### Community 82 - "Block Exploits Plugin"
Cohesion: 0.27
Nodes (7): block_and_log_modes(), BlockExploits, detect_with(), Hit, may_log(), plugin(), settings()

### Community 83 - "Proxy Features and Architecture"
Cohesion: 0.22
Nodes (5): Authentication tests, Per-site mutual TLS (mtls), mTLS handshake and resumption tests, OpenSSL, RFC 4514 (certificate subject)

### Community 84 - "CrowdSec and Authentication Limits"
Cohesion: 0.18
Nodes (8): Cloudflare (CDN in front of the origin), CrowdSec AppSec (WAF) checks, Built-in CrowdSec L7 bouncer, CrowdSec parser for conduits access log (deploy/crowdsec), CrowdSec full-loop tests, Pebble ACME test CA, ACME end-to-end test with Pebble, rate_limit plugin

### Community 85 - "Static Files and Response Compression"
Cohesion: 0.18
Nodes (6): Static-file conditional requests and single byte ranges, Static file server (files:), Static file-server tests, RFC 9110 (HTTP semantics), rules plugin, Upstream Brotli finalization (b5dea5a, PR 1021, issue 1013)

### Community 86 - "E2E HTTP Helpers"
Cohesion: 0.33
Nodes (9): exchange(), https(), load_until(), parse(), raw_http(), raw_request(), read_all(), read_response() (+1 more)

### Community 87 - "Mixed Traffic Soak Harness"
Cohesion: 0.42
Nodes (10): cert(), event(), every(), flap(), keep_log(), rotate_log(), sample(), soak.sh script (+2 more)

### Community 88 - "Rule Action Config"
Cohesion: 0.31
Nodes (10): ActionConfig, Deny, File, Proxy, Redirect, Respond, ActionRaw, FileConfig (+2 more)

### Community 90 - "Example TLS and ACME"
Cohesion: 0.20
Nodes (10): directory, email, storage, cert, key, name, tls, acme (+2 more)

### Community 91 - "Example CrowdSec and AppSec"
Cohesion: 0.20
Nodes (10): body_limit, failure_action, timeout, url, crowdsec, api_key_file, appsec, lapi_url (+2 more)

### Community 93 - "RFC 3339 Time Formatting"
Cohesion: 0.33
Nodes (6): civil_from_days(), known_instants(), rfc3339(), rfc3339_millis(), seconds_part(), write_rfc3339_millis()

### Community 94 - "Throughput Benchmark Harness"
Cohesion: 0.36
Nodes (8): baseline_rate(), cpu_ticks(), load(), peak_rss_kb(), pids_of(), run(), bench.sh script, wait_port()

### Community 95 - "Static File Configuration Tests"
Cohesion: 0.29
Nodes (8): FilesConfig, body(), Dir, files(), precompressed_try_files_browse_prefix(), serving_and_confinement(), settings_are_checked(), tree()

### Community 97 - "Parity Script"
Cohesion: 0.28
Nodes (3): accepted_bits(), main(), normalize()

### Community 98 - "Configuration Reference and nginx Mapping"
Cohesion: 0.31
Nodes (9): ACME automatic certificates (HTTP-01, ARI), certbot, SNI certificate selection, file action (in-memory file response), Watched files (mtime, inode, size, follows symlinks), Let's Encrypt, listen section (http/https addresses), RFC 9773 ACME Renewal Information (ARI) (+1 more)

### Community 99 - "Proxy Features and Architecture"
Cohesion: 0.22
Nodes (7): block_exploits categories and rule ids, block_exploits plugin, block_exploits attack and ordinary-request tests, Exploit categories (traversal, dotfiles, sqli, xss, php, rce, ssrf, scanners, spam), Nginx Proxy Manager, Parity test against nginx (Nginx Proxy Manager), Exploit replay on 1,139,833 production requests

### Community 100 - "Exploit Rule Parsing"
Cohesion: 0.28
Nodes (5): all_rules(), categories_mask(), cost(), tautologies(), tautology()

### Community 101 - "Example Defaults"
Cohesion: 0.25
Nodes (8): defaults, compression, connect_timeout, keepalive_timeout, max_body_size, read_timeout, upstream_idle_timeout, write_timeout

### Community 102 - "Pingora Backport Preparation"
Cohesion: 0.25
Nodes (8): HTTP/2 upstream: implementasi dan validasi, Integritas dan pemetaan, Pemakaian, Perbaikan dependency yang diperlukan, Performa, Perubahan implementasi, Stabilitas H2 upstream, Uji fungsional dan regresi

### Community 103 - "CrowdSec IP Bit Masks"
Cohesion: 0.25
Nodes (6): Bits, .WIDTH, u128, .WIDTH, u32, .WIDTH

### Community 104 - "Configuration Reference and nginx Mapping"
Cohesion: 0.29
Nodes (6): F10: README Server header contract aligned, Cache-Status header (RFC 9211), Headers conduit adds to client responses, RFC 9211 Cache-Status, server_header option (Server: conduit), Server header off preserves origin header

### Community 105 - "Documentation Validation and Provenance"
Cohesion: 0.29
Nodes (7): Audit dan validasi Conduit, Batas hasil, Cara membaca laporan, Graphify, Context7 dan provenance, Matriks penutupan, Reproduksi pada checkout baru, Status saat ini

### Community 106 - "Stability Tests and Soak Evidence"
Cohesion: 0.29
Nodes (6): Fuzz properties: config, path, access_log, request, route, body_replace, block_exploits, Fuzzing (cargo-fuzz, src/fuzzing.rs), server_header end-to-end test, Local test results, 111 application unit tests passed, mise development tasks

### Community 107 - "Alternating Binary Comparison Harness"
Cohesion: 0.48
Nodes (5): config(), cpu_ticks(), load(), ab.sh script, wait_port()

### Community 110 - "CrowdSec AppSec Acquisition"
Cohesion: 0.33
Nodes (5): appsec_config, labels, type, listen_addr, source

### Community 111 - "Fix Validation and Regressions"
Cohesion: 0.33
Nodes (6): Hasil suite dan pemeriksaan, Integritas dan batas kesimpulan, Matriks penutupan, Perbaikan dan validasi penutupan temuan, Performa release sebelum/sesudah, Stabilitas berkelanjutan

### Community 112 - "Rule Actions and Header Templates"
Cohesion: 0.60
Nodes (5): StrMatch, Exact, Prefix, Regex, StrMatchRaw

### Community 113 - "Exploit Scan Engine"
Cohesion: 0.33
Nodes (3): Place, Path, Query

### Community 117 - "Example Site Settings"
Cohesion: 0.40
Nodes (5): upstream, backends, balance, hash_key, verify_tls

### Community 118 - "Post-fix Validation Memory"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: ok languns perbaiki semua temuan ya,, dan setiap temuan fix languns di uji e2e ya,, dan kalo udah semua uji e2e semunaya dan uji stabilitas dan performasnya dan uji sekali algi apakah sduah tertutup, Source Nodes

### Community 119 - "Regression Test Helpers"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: ini conduit dukunga ke upstream ini hanay di http/1 ya? http/2 belum?, Source Nodes

### Community 120 - "Regression Test Helpers"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: loh kerjakan bro,, karena ada tool internal butuh http/2 upstream,, semua fitur di pingora kalo bisa ngapain gk di kerjakan di conduit bro, Source Nodes

### Community 121 - "PGO Training Script"
Cohesion: 0.60
Nodes (3): fetch(), pgo-train.sh script, urls()

### Community 122 - "Client Cert Verification"
Cohesion: 0.40
Nodes (4): ClientStatus, Invalid, Missing, Valid

### Community 123 - "Example Global Configuration"
Cohesion: 0.50
Nodes (4): real_ip, header, recursive, trusted

### Community 124 - "CrowdSec Log Acquisition"
Cohesion: 0.50
Nodes (3): filenames, labels, type

### Community 126 - "Hidden Files and Ranges"
Cohesion: 0.83
Nodes (3): Hide, Name, Suffix

### Community 127 - "Command Line Interface"
Cohesion: 0.50
Nodes (4): Cli, Cmd, Check, Run

## Knowledge Gaps
- **140 isolated node(s):** `Off`, `Websocket`, `Hash`, `LeastConn`, `Random` (+135 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 672 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **48 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `conduit configuration reference (docs/configuration.md)` connect `Configuration Reference and nginx Mapping` to `Audit Evidence and Documentation`, `Configuration Reference and nginx Mapping`, `Configuration Reference and nginx Mapping`, `Example Global Configuration`, `Configuration Reference and nginx Mapping`, `Proxy Features and Architecture`, `HTTP2 Streaming and Tests`, `HTTP2 Pool Protocol Selection`, `Configuration Reference and nginx Mapping`?**
  _High betweenness centrality (0.115) - this node is a cross-community bridge._
- **Are the 11 inferred relationships involving `http()` (e.g. with `file_descriptors_run_out()` and `idle_and_malformed_clients()`) actually correct?**
  _`http()` has 11 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Off`, `Websocket`, `Hash` to the rest of the system?**
  _140 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Upstream Passive Ejection` be split into smaller, more focused modules?**
  _Cohesion score 0.060041407867494824 - nodes in this community are weakly interconnected._
- **Why does `tls` connect `Example TLS and ACME` to `Main Entry and Listeners`, `Example Global Configuration`?**
  _High betweenness centrality (0.033) - this node is a cross-community bridge._
- **Should `End-to-End Tests` be split into smaller, more focused modules?**
  _Cohesion score 0.051923076923076926 - nodes in this community are weakly interconnected._
- **Why does `Pool` connect `Upstream Passive Ejection` to `Proxy Route Rewriting`, `Typed Plugin Configuration`, `Runtime Site Compilation`, `Per-site Plugin Chain`, `Proxy Session Hooks`, `Proxy Response Status and Headers`, `Rule Actions and Header Templates`, `Upstream Balancing Configuration`, `Plugin Admission and Concurrency`?**
  _High betweenness centrality (0.033) - this node is a cross-community bridge._
## Documentation synchronization

This graph includes the working-tree documentation refresh for implementation `03cae2b`:
HTTP/1.1 default and optional H2/h2c, strict TLS policy, stream limits and gRPC constraints,
cache authority/variants, request snapshots on reload, reproducible vendor preparation and
current versus historical validation evidence. The current audit index is included; historical
findings, baseline inventory, source snapshots and raw logs remain excluded by .graphifyignore.
The pinned Pingora backport is described through vendor documentation; vendor source AST is
excluded. Earlier query memories and pre-H2 evidence retain their historical scope.


All 13 project Markdown documents were reviewed/updated, including this generated report.
Third-party Markdown under vendor/pingora and ignored local build copies are outside the
project-documentation scope. See [the audit index](../docs/audit/README.md) and
[documentation-sync.json](../docs/audit/documentation-sync.json) for link/example checks,
unchanged implementation hashes, graph integrity and output hashes. The user-reported
Krynox 24-hour run was not modified or independently verified.
