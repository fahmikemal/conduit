# Graph Report - conduit  (2026-11-10)

## Corpus Check
- 61 files · ~124,297 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 2301 nodes · 5224 edges · 138 communities (116 shown, 22 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 38 edges (avg confidence: 0.84)
- Token cost: unavailable; the agent tool did not expose measured token usage (no estimate recorded).

## Community Hubs (Navigation)
- Configuration Reference and nginx Mapping
- End-to-End Tests
- Proxy Features and Architecture
- Rate and Concurrency Limits
- Per-site Plugin Chain
- Forward and Basic Auth
- Pingora Fork and Stability Patches
- Proxy Response Status and Headers
- Global Config Sections
- Cache Storage Globals
- CrowdSec Client
- TLS OpenSSL Bindings
- Config Parsing
- Request Filter and Decide
- Access Log File IO
- ACME Certificate Service
- Rule Actions and Header Templates
- Proxy Session Hooks
- Plugin Admission and Concurrency
- Runtime Site Compilation
- Main Entry and Listeners
- Prometheus Metrics Registry
- Block Exploits Detector
- E2E Process and Port Helpers
- Example Upstreams and Health Checks
- Body Replace Plugin
- Example Global Configuration
- Typed Plugin Configuration
- Proxy Comparison Script
- Proxy Route Rewriting
- Upstream Passive Ejection
- Exploit Rule Categories
- Example Plugin Settings
- Access Log Writer Shards
- Static File Server
- WebSocket Soak Client
- CrowdSec Stream Deserializer
- TLS Lifecycle and Hot Reload
- Config Reload Watcher
- HTTP End-to-End Scenarios
- Authentication and Plugin Gates
- CrowdSec AppSec Plugin
- Brand Assets
- Cache Lookup and Purge Hooks
- Outbound HTTP Client
- Plugin Configuration Types
- Rule Match Compilation
- Upstream Health Service
- Fuzzing Harness
- Stability Tests and Soak Evidence
- Cache Store and Purge
- Exploit Scan Engine
- Example Site Settings
- Runtime Logger and Filtering
- CrowdSec Network Decision Updates
- Certificate Store Selection
- Trusted Client IP Resolution
- Upstream Pool Compilation
- Cacheability and Variants
- Performance Roadmap and Tradeoffs
- Admin API
- CrowdSec Decision Storage
- Background Services and Reload State
- Static File Reply Streaming
- Example Files and Basic Auth
- Static File Lookup and Listings
- Health Probes
- CrowdSec Decision Table Synchronization
- Parity Script
- ACME Certificate Issuance
- Soak Monitoring and Observability
- Static Files and Response Compression
- Access Log Entry Encoding
- Upstream Balancing Configuration
- Block Exploits Plugin
- mTLS E2E Tests
- ACME Key and CSR
- Cache Hit Body Handling
- Accept-Encoding Negotiation
- File Reply Plan
- Proxy Unit Test Runtime
- E2E HTTP Helpers
- Parity Stub Backend
- Soak Result Attribution
- Benchmark and Test Evidence
- CrowdSec and Authentication Limits
- Proxy Response Decoration
- RFC 3339 Time Formatting
- Mixed Traffic Soak Harness
- ACME Renewal Scheduling
- Rule Action Config
- CrowdSec Remediation Lookup
- Request Path Normalization
- Client Cert Verification
- ACME HTTP Transport
- Example TLS and ACME
- Example CrowdSec and AppSec
- Throughput Benchmark Harness
- TLS Certificate Construction
- Upstream Backend Selection
- Cache Storage Round Trip
- Performance Analysis and Profiling
- Reload Under Load Test
- Static File Configuration Tests
- Example Defaults
- CrowdSec IP Bit Masks
- Exploit Rule Parsing
- ACME Account Management
- Alternating Binary Comparison Harness
- Access Log Fuzz RNG
- Exploit Match Places
- Static File Path Templates
- CrowdSec AppSec Acquisition
- String Match Config
- Example Inline Upstream
- PGO Training Script
- Example Trusted Client IPs
- CrowdSec Log Acquisition
- Comparison Proxy Configs
- Container Resource Probe
- Hidden Files and Ranges
- Command Line Interface
- PGO Build Script
- Protocol Upgrade Modes
- Pebble Test Container Lifecycle
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
5. `Ctx` - 34 edges
6. `Shared` - 33 edges
7. `spawn()` - 31 edges
8. `Dur` - 27 edges
9. `PluginConfig` - 26 edges
10. `Plugin` - 26 edges

## Surprising Connections (you probably didn't know these)
- `c4cce19: task_pipe HTTP/1 task channel` --semantically_similar_to--> `JSON access log (per-worker buffers)`  [INFERRED] [semantically similar]
  vendor/README.md → docs/technical.md
- `keys_and_variants()` --references--> `meta`  [INFERRED]
  src/cache.rs → deploy/crowdsec/conduit-logs.yaml
- `compile_site()` --references--> `header`  [INFERRED]
  src/runtime.rs → conduit.example.yaml
- `compile_site()` --references--> `upstream`  [INFERRED]
  src/runtime.rs → conduit.example.yaml
- `mise development tasks` --references--> `scripts/bench.sh benchmark and regression gate`  [INFERRED]
  README.md → docs/technical.md

## Import Cycles
- 2-file cycle: `src/metrics.rs -> src/runtime.rs -> src/metrics.rs`
- 2-file cycle: `src/limits.rs -> src/plugin.rs -> src/limits.rs`
- 2-file cycle: `src/plugin.rs -> src/replace.rs -> src/plugin.rs`
- 2-file cycle: `src/crowdsec.rs -> src/plugin.rs -> src/crowdsec.rs`
- 2-file cycle: `src/proxy.rs -> src/tls.rs -> src/proxy.rs`
- 2-file cycle: `src/proxy.rs -> src/upstream.rs -> src/proxy.rs`
- 3-file cycle: `src/proxy.rs -> src/runtime.rs -> src/tls.rs -> src/proxy.rs`
- 3-file cycle: `src/proxy.rs -> src/runtime.rs -> src/upstream.rs -> src/proxy.rs`
- 3-file cycle: `src/metrics.rs -> src/runtime.rs -> src/upstream.rs -> src/metrics.rs`
- 3-file cycle: `src/crowdsec.rs -> src/proxy.rs -> src/runtime.rs -> src/crowdsec.rs`
- 3-file cycle: `src/crowdsec.rs -> src/metrics.rs -> src/runtime.rs -> src/crowdsec.rs`
- 3-file cycle: `src/metrics.rs -> src/tls.rs -> src/proxy.rs -> src/metrics.rs`
- 3-file cycle: `src/limits.rs -> src/plugin.rs -> src/replace.rs -> src/limits.rs`
- 3-file cycle: `src/plugin.rs -> src/upstream.rs -> src/proxy.rs -> src/plugin.rs`
- 3-file cycle: `src/crowdsec.rs -> src/proxy.rs -> src/plugin.rs -> src/crowdsec.rs`
- 3-file cycle: `src/proxy.rs -> src/rules.rs -> src/upstream.rs -> src/proxy.rs`
- 4-file cycle: `src/plugin.rs -> src/route.rs -> src/upstream.rs -> src/proxy.rs -> src/plugin.rs`
- 4-file cycle: `src/proxy.rs -> src/rules.rs -> src/route.rs -> src/upstream.rs -> src/proxy.rs`
- 4-file cycle: `src/metrics.rs -> src/runtime.rs -> src/tls.rs -> src/proxy.rs -> src/metrics.rs`
- 4-file cycle: `src/metrics.rs -> src/runtime.rs -> src/upstream.rs -> src/proxy.rs -> src/metrics.rs`

## Hyperedges (group relationships)
- **Inputs to the cache key that prevent cache poisoning** — docs_configuration_cache_key, docs_configuration_forward_auth, docs_configuration_mtls, docs_configuration_trusted_proxy_headers, docs_configuration_proxy_action, docs_technical_body_replace [EXTRACTED 1.00]
- **Plugins that can answer (end) the request** — docs_configuration_rules_plugin, docs_technical_ip_filter, docs_configuration_crowdsec_plugin, docs_configuration_rate_limit, docs_configuration_concurrency, docs_technical_basic_auth, docs_configuration_forward_auth, docs_technical_block_exploits [EXTRACTED 1.00]
- **Phase three core stability validation** — docs_technical_upstream_flapping_test, docs_technical_soak_24h_results, docs_technical_fuzzing, docs_technical_panic_audit, docs_technical_open_websockets_upgrade_test, docs_technical_parity_test [EXTRACTED 1.00]
- **Soak harness evidence and failure attribution** — docs_technical_soak_events, docs_technical_soak_samples, docs_technical_soak_report, docs_technical_soak_archives, docs_technical_soak_upgrade_failure_guard [EXTRACTED 1.00]
- **Pingora ten performance patches** — vendor_readme_patch_79bf391, vendor_readme_patch_32d98ca, vendor_readme_patch_ecef83b, vendor_readme_patch_9586c10, vendor_readme_patch_c448eff, vendor_readme_patch_9c64421, vendor_readme_patch_3357c9e, vendor_readme_patch_bb3859d, vendor_readme_patch_c4cce19, vendor_readme_patch_0a6aee4 [EXTRACTED 1.00]
- **Backend failure handling (selection, retries, ejection, health checks)** — docs_configuration_backend_selection, docs_configuration_retries, docs_configuration_passive_ejection, docs_configuration_active_health_checks, docs_configuration_upstream_pools [INFERRED 0.85]

## Communities (138 total, 22 thin omitted)

### Community 0 - "Configuration Reference and nginx Mapping"
Cohesion: 0.05
Nodes (89): description, filter, name, onsuccess, statics, expression, meta, parsed (+81 more)

### Community 1 - "End-to-End Tests"
Cohesion: 0.06
Nodes (29): Answer, auth_service(), body_exchange(), BOUNCER_KEY, cache_upstream(), Counts, CROWDSEC_TAG, DOWN (+21 more)

### Community 2 - "Proxy Features and Architecture"
Cohesion: 0.07
Nodes (34): block_exploits categories and rule ids, scripts/ab.sh A/B harness, Authentication tests, block_exploits plugin, block_exploits attack and ordinary-request tests, Stale cache revalidation and stale-if-error, Response-cache tests, clippy -D warnings (+26 more)

### Community 3 - "Rate and Concurrency Limits"
Cohesion: 0.08
Nodes (18): carried(), CARRIED, chain(), check_status(), client(), Concurrency, concurrency_holds_until_dropped(), Hasher (+10 more)

### Community 4 - "Per-site Plugin Chain"
Cohesion: 0.08
Nodes (13): HeaderOps, HeadersConfig, IpFilterConfig, Chain, chain(), HeadersPlugin, ip_filter(), IpFilter (+5 more)

### Community 5 - "Forward and Basic Auth"
Cohesion: 0.09
Nodes (23): basic(), basic_auth_flow(), basic_auth_settings(), CACHE_KEY, config(), Counters, forward_auth_request(), FORWARD_MAX_BODY (+15 more)

### Community 6 - "Pingora Fork and Stability Patches"
Cohesion: 0.08
Nodes (28): Real-binary end-to-end tests, Getting started (build from source), CasedHeaderName, daemon_wait_for_ready, Docker release build includes vendored Pingora, FlushTrackingMock, TransportConnector idle connection pool, [patch.crates-io] wiring of pingora-* crates (+20 more)

### Community 7 - "Proxy Response Status and Headers"
Cohesion: 0.07
Nodes (17): ACME_PREFIX, BASE, CACHE_STATUS, each_worker_thread_has_its_own_pool_group(), header_str(), new_request_id(), request_ids(), worker_pool_group() (+9 more)

### Community 8 - "Global Config Sections"
Cohesion: 0.08
Nodes (21): AcmeSection, AppSecSection, CacheSection, CrowdSecSection, FailureAction, Allow, Block, five_seconds() (+13 more)

### Community 9 - "Cache Storage Globals"
Cohesion: 0.07
Nodes (15): apply(), ENTRY_OVERHEAD, EVICTION, heuristic(), LOCK_AGE, LOCK, lock(), no_default_ttl() (+7 more)

### Community 10 - "CrowdSec Client"
Cohesion: 0.08
Nodes (20): appsec_request_shape(), APPSEC_WARNED, BAN_PAGE, base_url(), EPOCH, forwarded(), LAPI_DEADLINE, LAPI_MAX_RESPONSE (+12 more)

### Community 11 - "TLS OpenSSL Bindings"
Cohesion: 0.08
Nodes (7): ask_client_certs(), ask_for_certificate(), client_hello_sni(), ClientTrust, sni_from_extension(), SSL_set_session_id_context(), TRUST_INDEX

### Community 12 - "Config Parsing"
Cohesion: 0.09
Nodes (20): AccessLogSection, acme_section(), AdminSection, Config, DefaultSite, example_config_builds(), FilesConfig, LETSENCRYPT (+12 more)

### Community 13 - "Request Filter and Decide"
Cohesion: 0.13
Nodes (29): Effects, Local, Step, Pass, acme_response(), answer(), appsec(), check_client_cert() (+21 more)

### Community 14 - "Access Log File IO"
Cohesion: 0.07
Nodes (14): ESCAPE, FLUSH_EVERY, HEX, HIGHS, int(), needs_escape(), ONES, OpenFile (+6 more)

### Community 15 - "ACME Certificate Service"
Cohesion: 0.08
Nodes (7): cert_id(), CHALLENGES, failure_detail(), HTTP_TIMEOUT, MAX_RESPONSE, SiteStatus, Status

### Community 16 - "Rule Actions and Header Templates"
Cohesion: 0.11
Nodes (24): RuleConfig, Action, Deny, Proxy, Redirect, Respond, compile_headers(), forgejo_browser_redirect() (+16 more)

### Community 17 - "Proxy Session Hooks"
Cohesion: 0.16
Nodes (9): client_cert(), Conduit, Ctx, final_status(), forwards_upgrade(), ip_text(), pool_of(), proto() (+1 more)

### Community 18 - "Plugin Admission and Concurrency"
Cohesion: 0.11
Nodes (13): Held, Gate, Answer, Check, Pass, Replace, Route, Answer (+5 more)

### Community 19 - "Runtime Site Compilation"
Cohesion: 0.14
Nodes (20): yaml(), acme_dir(), acme_plan(), acme_site(), acme_sites(), acme_storage_per_directory(), AcmePlan, build() (+12 more)

### Community 20 - "Main Entry and Listeners"
Cohesion: 0.10
Nodes (8): check(), DEFAULT_CONFIG, GLOBAL, huge_pages(), listener(), main(), run(), socket_options()

### Community 21 - "Prometheus Metrics Registry"
Cohesion: 0.09
Nodes (7): CLASSES, forget_site(), LATENCY_BUCKETS, lazy!, PROCESS, SiteMetrics, status_classes()

### Community 22 - "Block Exploits Detector"
Cohesion: 0.09
Nodes (20): AGENT, Attack, ATTACKS, attacks_are_caught_by_their_rule(), detect(), ENGINE, every_rule_has_an_attack(), H (+12 more)

### Community 23 - "E2E Process and Port Helpers"
Cohesion: 0.22
Nodes (22): auth_plugins(), block_exploits(), body_upstream(), CrowdSec, crowdsec_bouncer_and_appsec(), docker(), echo(), free_port() (+14 more)

### Community 24 - "Example Upstreams and Health Checks"
Cohesion: 0.09
Nodes (25): url, weight, expect, fall, interval, path, rise, timeout (+17 more)

### Community 25 - "Body Replace Plugin"
Cohesion: 0.16
Nodes (13): BodyReplaceConfig, ReplacePair, bad_configs(), BodyReplace, chunk_boundaries_do_not_matter(), holds_back_no_more_than_needed(), replacer(), replaces_every_occurrence_first_pattern_wins() (+5 more)

### Community 26 - "Example Global Configuration"
Cohesion: 0.08
Nodes (23): access_log, path, queue_capacity, admin, listen, cache, lock_timeout, max_object_size (+15 more)

### Community 27 - "Typed Plugin Configuration"
Cohesion: 0.11
Nodes (13): BackendConfig, Defaults, from_map(), or_default(), ServerHeader, Off, Value, SiteCacheConfig (+5 more)

### Community 28 - "Proxy Comparison Script"
Cohesion: 0.17
Nodes (14): container_name(), docker_rm(), main(), med(), summarize(), measure(), oha(), report() (+6 more)

### Community 29 - "Proxy Route Rewriting"
Cohesion: 0.15
Nodes (14): ProxyConfig, RewriteConfig, bad_configs(), decoded_bytes_are_encoded_again(), encode_path(), path_prefix(), regex_rewrite(), Rewrite (+6 more)

### Community 30 - "Upstream Passive Ejection"
Cohesion: 0.17
Nodes (5): Lease, MemberStatus, now_ms(), Pool, Target

### Community 31 - "Exploit Rule Categories"
Cohesion: 0.10
Nodes (17): Category, .ALL, Dotfiles, Php, Rce, Scanners, Spam, Sqli (+9 more)

### Community 32 - "Example Plugin Settings"
Cohesion: 0.09
Nodes (22): except, skip_params, replace, types, max, copy_headers, url, response (+14 more)

### Community 33 - "Access Log Writer Shards"
Cohesion: 0.16
Nodes (5): AccessLog, lock(), Shard, Shards, Writer

### Community 34 - "Static File Server"
Cohesion: 0.10
Nodes (7): CHUNK, html_escape(), human_size(), LIST_LIMIT, NO_OPENAT2, url_escape(), with_query()

### Community 35 - "WebSocket Soak Client"
Cohesion: 0.24
Nodes (11): client(), shorts(), connect(), echo(), frame(), long_lived(), read_frame(), serve() (+3 more)

### Community 36 - "CrowdSec Stream Deserializer"
Cohesion: 0.13
Nodes (4): Apply, List, List<'_, '_, S>, &mut Apply<'_, S>

### Community 37 - "TLS Lifecycle and Hot Reload"
Cohesion: 0.12
Nodes (14): Built-in ACME client (HTTP-01, ARI), conduit check (config validation), Active HTTP/TCP health checks, Hot reload without dropped connections, Load-balancing algorithms (round_robin, random, least_conn, Ketama hash), Production rollout began in November 2026, RFC 9773 (ACME renewal information), Certificate rotation and reload under load test (+6 more)

### Community 38 - "Config Reload Watcher"
Cohesion: 0.15
Nodes (5): record(), Reloader, snapshot(), Stamp, stamp()

### Community 39 - "HTTP End-to-End Scenarios"
Cohesion: 0.20
Nodes (17): acme_with_pebble(), backend_state(), config(), echo_upstream(), end_to_end(), expiring(), file_server(), graceful_upgrade() (+9 more)

### Community 40 - "Authentication and Plugin Gates"
Cohesion: 0.15
Nodes (10): BasicAuth, cache_key(), check(), credentials(), credentials_parsing(), Hasher, refused(), verify() (+2 more)

### Community 41 - "CrowdSec AppSec Plugin"
Cohesion: 0.13
Nodes (10): AppSec, CrowdSecPlugin, http_version(), Plan, pull(), site_plugin_refuses_held_decisions(), Verdict, Allow (+2 more)

### Community 42 - "Brand Assets"
Cohesion: 0.15
Nodes (15): conduit-app-icon.svg, Conduit brand assets, Ember palette, favicon.svg / favicon.ico, Geist typeface (SemiBold 600, Regular 400, Mono 400), Lanes mark, conduit-logo-*-ember.svg / conduit-mark-ember.svg, conduit-logo-horizontal.svg (+7 more)

### Community 43 - "Cache Lookup and Purge Hooks"
Cohesion: 0.16
Nodes (8): key(), keys_and_variants(), push_framed(), Stats, stats(), storage_round_trip(), tracked(), variance()

### Community 44 - "Outbound HTTP Client"
Cohesion: 0.22
Nodes (4): Client, IDLE, Reply, Uri

### Community 45 - "Plugin Configuration Types"
Cohesion: 0.13
Nodes (18): BlockExploitsConfig, ConcurrencyConfig, CrowdSecPluginConfig, ExploitException, ForwardAuthConfig, MatchConfig, PluginConfig, BasicAuth (+10 more)

### Community 46 - "Rule Match Compilation"
Cohesion: 0.17
Nodes (10): HeaderMatch, BytesMatch, Exact, Prefix, Regex, HeaderCond, Equals, Present (+2 more)

### Community 47 - "Upstream Health Service"
Cohesion: 0.14
Nodes (4): EPOCH, MAX_ITERATIONS, PROBE_HTTP, PROBE_TCP

### Community 48 - "Fuzzing Harness"
Cohesion: 0.12
Nodes (4): config(), ip_backends_only(), ROUTER, ROUTES

### Community 49 - "Stability Tests and Soak Evidence"
Cohesion: 0.12
Nodes (15): Daemon readiness smoke (2026-11-10: 255,136 requests, 2 upgrades), Fuzz properties: config, path, access_log, request, route, body_replace, block_exploits, Fuzzing (cargo-fuzz, src/fuzzing.rs), Graceful binary upgrade (socket handover), Graceful upgrade test, upgrade_with_open_websockets test, Panic and misbehaving-client audit, PGO release build (scripts/pgo.sh) (+7 more)

### Community 50 - "Cache Store and Purge"
Cohesion: 0.30
Nodes (4): lock_shard(), Object, Shard, Store

### Community 51 - "Exploit Scan Engine"
Cohesion: 0.20
Nodes (12): after(), contains(), decode_query(), decoding(), Engine, fuzz_check(), has_escape(), header_suspect() (+4 more)

### Community 52 - "Example Site Settings"
Cohesion: 0.12
Nodes (17): default_ttl, stale_if_error, ca, sites, acme, cache, compression, force_https (+9 more)

### Community 53 - "Runtime Logger and Filtering"
Cohesion: 0.20
Nodes (7): ExploitMode, Block, Log, benign(), chatty(), Logger, only_the_pool_race_is_benign()

### Community 54 - "CrowdSec Network Decision Updates"
Cohesion: 0.18
Nodes (10): Apply<'_, S>, Delta, go_duration_ms(), ip(), Net, V4, V6, parse_net() (+2 more)

### Community 55 - "Certificate Store Selection"
Cohesion: 0.18
Nodes (4): render(), CertKey, CertResolver, CertStore

### Community 56 - "Trusted Client IP Resolution"
Cohesion: 0.26
Nodes (8): canonical(), Cidr, CLOUDFLARE, ip(), RealIp, recursive_skips_trusted_hops(), trusted_peer_header_used(), untrusted_peer_header_ignored()

### Community 57 - "Upstream Pool Compilation"
Cohesion: 0.17
Nodes (10): hashing_is_sticky(), least_conn_prefers_idle_backends(), passive_ejection_and_panic_mode(), pick(), pool_cfg(), resolve(), Resolved, round_robin_spreads_and_skips_excluded() (+2 more)

### Community 58 - "Cacheability and Variants"
Cohesion: 0.19
Nodes (8): eligibility(), req(), resp(), site(), SiteCache, stored(), vary_names(), what_is_stored()

### Community 59 - "Performance Roadmap and Tradeoffs"
Cohesion: 0.13
Nodes (11): Commands, conduit: technical documentation, Deferred features (session tickets, OCSP, admin API, OpenTelemetry, captcha, DNS-01), HTTP/1.1 and HTTPS throughput analysis, HTTP/1.1 and HTTPS throughput, Phase 2 (ACME, CrowdSec, limits, mTLS, auth), Phase 1 targets, Roadmap (+3 more)

### Community 60 - "Admin API"
Cohesion: 0.28
Nodes (6): Admin, bad(), reply(), service(), unescape(), unix()

### Community 61 - "CrowdSec Decision Storage"
Cohesion: 0.22
Nodes (7): Decision, Family, Family<K>, Map, Slot, Many, One

### Community 62 - "Background Services and Reload State"
Cohesion: 0.14
Nodes (4): AcmeService, ReloadStatus, Shared, HealthService

### Community 63 - "Static File Reply Streaming"
Cohesion: 0.17
Nodes (7): content_type(), FileReply, http_date(), open_beneath(), read_exact_at(), reply(), send()

### Community 64 - "Example Files and Basic Auth"
Cohesion: 0.13
Nodes (15): htpasswd, match, match, root, strip_prefix, try_files, method, path (+7 more)

### Community 65 - "Static File Lookup and Listings"
Cohesion: 0.27
Nodes (10): Files, Found, File, Listing, Missing, Redirect, Status, listing() (+2 more)

### Community 66 - "Health Probes"
Cohesion: 0.22
Nodes (5): ProbeType, Http, Tcp, Member, Probe

### Community 67 - "CrowdSec Decision Table Synchronization"
Cohesion: 0.23
Nodes (6): Applied, now(), parse_into(), stream_full_then_delta(), sync(), Table

### Community 68 - "Parity Script"
Cohesion: 0.26
Nodes (5): fetch(), hosts_from(), main(), run(), read_cases()

### Community 69 - "ACME Certificate Issuance"
Cohesion: 0.22
Nodes (8): challenge_tokens_are_withdrawn(), order(), Published, storage_lock_is_exclusive(), StorageLock, try_lock(), write_private(), AcmeSite

### Community 70 - "Soak Monitoring and Observability"
Cohesion: 0.18
Nodes (10): Cache purge endpoint (POST /cache/purge), Prometheus metrics, Unique soak archives preserve prior output and runtime files, Soak reload, backend flap, certificate and access-log rotation schedule, Soak report: hourly medians, per-process RSS and disruption attribution, Soak process sampling every 30 seconds, Seven-minute soak smoke configuration (scale 40, upgrades 1 and 3), Soak test (scripts/soak, conduit-soak unit) (+2 more)

### Community 71 - "Static Files and Response Compression"
Cohesion: 0.15
Nodes (8): Static-file conditional requests and single byte ranges, Static file server (files:), Static file-server tests, rules proxy action (strip_prefix, add_prefix, rewrite), RFC 9110 (HTTP semantics), routes_and_body_replace end-to-end test, rules plugin, Upstream Brotli finalization (b5dea5a, PR 1021, issue 1013)

### Community 72 - "Access Log Entry Encoding"
Cohesion: 0.24
Nodes (9): Entry, entry(), full_shard_drops_and_counts(), hostile_values_stay_one_line(), ip(), json_line(), line(), millis() (+1 more)

### Community 73 - "Upstream Balancing Configuration"
Cohesion: 0.18
Nodes (11): Balance, Hash, LeastConn, Random, RoundRobin, HashKey, ClientIp, Cookie (+3 more)

### Community 74 - "Block Exploits Plugin"
Cohesion: 0.24
Nodes (7): block_and_log_modes(), BlockExploits, detect_with(), Hit, may_log(), plugin(), settings()

### Community 75 - "mTLS E2E Tests"
Cohesion: 0.31
Nodes (5): Ca, cert_builder(), ec_key(), mtls_get(), MtlsReply

### Community 76 - "ACME Key and CSR"
Cohesion: 0.29
Nodes (8): asn1_unix(), bundle(), csr(), csr_names_every_host(), inspect(), issued(), new_key(), stored_bundle_loads_and_identifies()

### Community 78 - "Accept-Encoding Negotiation"
Cohesion: 0.21
Nodes (5): Accepted, Encoding, Br, Gzip, Zstd

### Community 79 - "File Reply Plan"
Cohesion: 0.20
Nodes (12): etag_listed(), parse_range(), Plan, plan(), Full, NotModified, Partial, PreconditionFailed (+4 more)

### Community 80 - "Proxy Unit Test Runtime"
Cohesion: 0.33
Nodes (11): acme_challenge(), acme_challenge_from_memory(), body_limit_and_default_site(), client_certificates(), https_redirect_runs_before_plugins(), plugins_see_normalized_path_and_client(), req(), routed_requests_skip_files_and_change_target() (+3 more)

### Community 81 - "E2E HTTP Helpers"
Cohesion: 0.30
Nodes (12): cpu_time(), exchange(), file_descriptors_run_out(), https(), open_fds(), parse(), raw_http(), raw_request() (+4 more)

### Community 84 - "Benchmark and Test Evidence"
Cohesion: 0.20
Nodes (11): Apache httpd, scripts/bench.sh benchmark and regression gate, Cross-proxy cgroup CPU and anonymous-memory comparison, Benchmark gate with baseline-fixed rates and p99 tolerances, Caddy, Envoy, HAProxy, Comparison with other reverse proxies (scripts/compare.py) (+3 more)

### Community 85 - "CrowdSec and Authentication Limits"
Cohesion: 0.18
Nodes (8): Cloudflare (CDN in front of the origin), CrowdSec AppSec (WAF) checks, Built-in CrowdSec L7 bouncer, CrowdSec parser for conduits access log (deploy/crowdsec), CrowdSec full-loop tests, Pebble ACME test CA, ACME end-to-end test with Pebble, rate_limit plugin

### Community 86 - "Proxy Response Decoration"
Cohesion: 0.24
Nodes (6): compressible(), decorate(), has_body(), respond(), status_page(), upstream_error_status()

### Community 87 - "RFC 3339 Time Formatting"
Cohesion: 0.25
Nodes (5): civil_from_days(), rfc3339(), rfc3339_millis(), seconds_part(), write_rfc3339_millis()

### Community 88 - "Mixed Traffic Soak Harness"
Cohesion: 0.42
Nodes (10): cert(), event(), every(), flap(), keep_log(), rotate_log(), sample(), soak.sh script (+2 more)

### Community 89 - "ACME Renewal Scheduling"
Cohesion: 0.24
Nodes (9): at(), backoff(), Due, At, Now, lifetime_renewal(), random_unit(), Stored (+1 more)

### Community 90 - "Rule Action Config"
Cohesion: 0.31
Nodes (10): ActionConfig, Deny, File, Proxy, Redirect, Respond, ActionRaw, FileConfig (+2 more)

### Community 91 - "CrowdSec Remediation Lookup"
Cohesion: 0.25
Nodes (6): d(), lookup_cost(), Remedy, Ban, Captcha, table_lookups_and_priority()

### Community 92 - "Request Path Normalization"
Cohesion: 0.25
Nodes (6): access_log(), n(), normalize(), PathError, Invalid, percent_decode()

### Community 93 - "Client Cert Verification"
Cohesion: 0.20
Nodes (5): ClientStatus, Invalid, Missing, Valid, distinguished_name()

### Community 95 - "Example TLS and ACME"
Cohesion: 0.20
Nodes (10): directory, email, storage, cert, key, name, tls, acme (+2 more)

### Community 96 - "Example CrowdSec and AppSec"
Cohesion: 0.20
Nodes (10): body_limit, failure_action, timeout, url, crowdsec, api_key_file, appsec, lapi_url (+2 more)

### Community 97 - "Throughput Benchmark Harness"
Cohesion: 0.36
Nodes (8): baseline_rate(), cpu_ticks(), load(), peak_rss_kb(), pids_of(), run(), bench.sh script, wait_port()

### Community 98 - "TLS Certificate Construction"
Cohesion: 0.29
Nodes (7): CertConfig, client_trust_from_file(), mismatched_key_rejected(), selection_order(), self_signed(), subject_as_rfc4514(), write_pair()

### Community 99 - "Upstream Backend Selection"
Cohesion: 0.22
Nodes (3): load_balancer(), LoadBalancer<S>, Select

### Community 101 - "Performance Analysis and Profiling"
Cohesion: 0.22
Nodes (7): Benchmark, Comparison with other reverse proxies, samply CPU profile, Latency under load (fixed-rate p99 overhead), Memory notes, samply CPU profiler, Threads and runtime

### Community 102 - "Reload Under Load Test"
Cohesion: 0.25
Nodes (7): load_until(), metric(), presented(), rotation_and_reload_under_load(), self_signed(), served(), tls_connect()

### Community 103 - "Static File Configuration Tests"
Cohesion: 0.31
Nodes (7): body(), Dir, files(), precompressed_try_files_browse_prefix(), serving_and_confinement(), settings_are_checked(), tree()

### Community 104 - "Example Defaults"
Cohesion: 0.25
Nodes (8): defaults, compression, connect_timeout, keepalive_timeout, max_body_size, read_timeout, upstream_idle_timeout, write_timeout

### Community 105 - "CrowdSec IP Bit Masks"
Cohesion: 0.25
Nodes (6): Bits, .WIDTH, u128, .WIDTH, u32, .WIDTH

### Community 106 - "Exploit Rule Parsing"
Cohesion: 0.32
Nodes (4): all_rules(), categories_mask(), cost(), tautologies()

### Community 107 - "ACME Account Management"
Cohesion: 0.33
Nodes (5): acme_err(), Ari, Connected, Manager, SiteState

### Community 108 - "Alternating Binary Comparison Harness"
Cohesion: 0.48
Nodes (5): config(), cpu_ticks(), load(), ab.sh script, wait_port()

### Community 110 - "Exploit Match Places"
Cohesion: 0.29
Nodes (4): Place, Header, Path, Query

### Community 111 - "Static File Path Templates"
Cohesion: 0.33
Nodes (4): Candidate, Part, Lit, Path

### Community 112 - "CrowdSec AppSec Acquisition"
Cohesion: 0.33
Nodes (5): appsec_config, labels, type, listen_addr, source

### Community 113 - "String Match Config"
Cohesion: 0.60
Nodes (5): StrMatch, Exact, Prefix, Regex, StrMatchRaw

### Community 114 - "Example Inline Upstream"
Cohesion: 0.40
Nodes (5): upstream, backends, balance, hash_key, verify_tls

### Community 115 - "PGO Training Script"
Cohesion: 0.60
Nodes (3): fetch(), pgo-train.sh script, urls()

### Community 116 - "Example Trusted Client IPs"
Cohesion: 0.50
Nodes (4): real_ip, header, recursive, trusted

### Community 117 - "CrowdSec Log Acquisition"
Cohesion: 0.50
Nodes (3): filenames, labels, type

### Community 118 - "Comparison Proxy Configs"
Cohesion: 0.67
Nodes (3): configs(), hcm(), listener()

### Community 120 - "Hidden Files and Ranges"
Cohesion: 0.83
Nodes (3): Hide, Name, Suffix

### Community 121 - "Command Line Interface"
Cohesion: 0.50
Nodes (4): Cli, Cmd, Check, Run

### Community 123 - "Protocol Upgrade Modes"
Cohesion: 0.67
Nodes (3): UpgradeMode, Off, Websocket

## Knowledge Gaps
- **116 isolated node(s):** `Path`, `Query`, `Off`, `Websocket`, `Answer` (+111 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 584 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **22 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `tls` connect `Example TLS and ACME` to `Example Global Configuration`, `Main Entry and Listeners`?**
  _High betweenness centrality (0.259) - this node is a cross-community bridge._
- **What connects `Path`, `Query`, `Off` to the rest of the system?**
  _116 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Configuration Reference and nginx Mapping` be split into smaller, more focused modules?**
  _Cohesion score 0.050042955326460484 - nodes in this community are weakly interconnected._
- **Why does `conduit configuration reference (docs/configuration.md)` connect `Configuration Reference and nginx Mapping` to `Example Global Configuration`, `Proxy Features and Architecture`?**
  _High betweenness centrality (0.239) - this node is a cross-community bridge._
- **Should `End-to-End Tests` be split into smaller, more focused modules?**
  _Cohesion score 0.058001397624039136 - nodes in this community are weakly interconnected._
- **Why does `sites` connect `Example Site Settings` to `Example Files and Basic Auth`, `Example Global Configuration`, `Example Plugin Settings`, `Example Inline Upstream`?**
  _High betweenness centrality (0.067) - this node is a cross-community bridge._
- **Should `Proxy Features and Architecture` be split into smaller, more focused modules?**
  _Cohesion score 0.07142857142857142 - nodes in this community are weakly interconnected._
## Latest synchronization

Final publication refresh for commit `cf21c85`: 2 documents read in full (959 and 304 lines), including daemon fix `5c765c6`, 109 unit tests, and November 2026 production rollout. 1 new node, 2 new edges. Graph health: OK; no missing/dangling endpoints, collapsed edges, or self-loops.
