# Conduit management and Web UI roadmap

Status: planned, recorded on 10 November 2026. This document captures the agreed product
requirements; it does not describe released configuration keys or implemented management
features. The current implementation is the proxy revision `03cae2b`, with the documentation
refresh in `543a5d5`. See the [technical guide](technical.md#roadmap) for completed work and
the [configuration reference](configuration.md) for options that exist today.

## Product decisions

- Treat **lightweight, powerful, reliable UI/UX and enterprise operation** as simultaneous
  release requirements. Features and visual polish must stay within the resource budget;
  a small bundle alone does not establish usable or reliable management.
- Keep headless operation as the default. Enable the Web UI explicitly on the management
  listener. A native build without UI assets must remain possible.
- Bundle compiled browser assets with the UI-enabled distribution. Running it must not
  require a Node.js service, a CDN, or a separate frontend deployment.
- Use the **Hallmark skill** when designing and implementing the UI. Preserve Conduit's
  Lanes mark and Ember palette from the [brand reference](../assets/brand/README.md).
  Require responsive layouts, light/dark themes, keyboard access, visible focus, appropriate
  contrast, reduced-motion support, and useful loading, empty, error and conflict states.
- Svelte and TypeScript are the proposed frontend approach. Confirm the build integration
  during implementation; this proposal does not add frontend dependencies yet.
- Use SQLite for local management accounts, sessions, permissions, drafts, revision history
  and audit records. YAML remains the authoritative persisted proxy configuration.
- Headless proxy operation without management must not create or open a management
  database. A headless management API must enforce the same authorization as the UI.
- Proxy requests continue to use compiled in-memory runtime state. They must not query
  SQLite or depend on the availability of the UI or identity provider for ordinary routing.

## Current foundation and missing work

The existing admin service provides `/status`, `/metrics`, `/healthz` and
`POST /cache/purge`. It runs on a separate listener and currently has no authentication.
The existing per-site `basic_auth` and `forward_auth` plugins do not automatically protect
that admin listener. There is no native admin OIDC login, management SQLite database,
configuration editing API or Web UI in the current release.

Existing snapshots, configuration compilation and reload behavior provide a starting point.
The new management service must reuse those contracts while adding concurrency control,
durable change tracking, authentication and recovery. Configure monitoring/health access
explicitly so introducing management authentication does not silently break existing scrapers.

## Milestones

| Milestone | Scope | Completion requirement |
|---|---|---|
| 4. Management foundation | Common configuration contract, change coordinator, SQLite migrations, local authentication, authorization, sessions and audit | Concurrent-change, authorization, persistence and recovery tests pass before management access is released. |
| 5. OIDC and monitoring UI | Generic OIDC login, tested Authentik/ZITADEL integrations, Hallmark-designed monitoring dashboard | Protected UI/API, provider integration tests, accessibility and asset/resource budgets pass. |
| 6. Configuration management UI | Site/upstream/plugin editing, drafts, diff, validation, apply, rollback and cache purge | UI/API/CLI parity, conflict, secret-preservation, retry and failure E2E tests pass. |
| Later enterprise extensions | More granular roles, external audit export, organization policies and optional multi-instance management | Separate designs and acceptance evidence; no implicit HA guarantee from the single-node implementation. |

Authentication and backend authorization belong to the first releasable management system,
including a monitoring-only UI. Milestone numbering is an order of work, not a release date.

## One configuration contract

The backend owns the schema, capabilities, validation and effective defaults. UI types and
forms must derive from that contract rather than maintaining a second set of defaults.
Every supported runtime setting must be inspectable; editing support and restart requirements
must be explicit. Unsupported or newly added fields must not disappear when an older UI saves.

The UI reads the active configuration from the backend. A draft is clearly marked as a draft,
and a successful file save must not be presented as a successful runtime apply. The API must
distinguish desired/persisted, active and pending-restart revisions. Settings that require a
restart remain labelled pending until the running process actually uses them.

SQLite holds management metadata and historical snapshots; it must not become a competing
editable copy of the active YAML configuration. Secret values are redacted in read APIs and
diffs. Omitting an unchanged secret preserves it; replacing or clearing it requires an explicit
operation. Preserve unrelated fields, comments and operator formatting where possible;
any normalization must be shown in the preview.

## Prevent silent overwrites

All supported writers—UI, management API and management CLI—use one change coordinator:

1. Read the active revision and configuration fingerprint.
2. Submit only the intended changes, the expected revision and an idempotency key.
3. Authorize the operation and serialize writers. Recheck the revision and file fingerprint
   while holding the coordinator's write lock.
4. Validate and compile the complete candidate using the backend configuration contract.
   Preview the diff, including effective defaults, redacted secrets and restart-only fields.
5. Persist and apply through a journalled operation, then report the actual resulting state.
6. On a stale revision, reject the write and show a conflict with a fresh diff. No automatic
   last-writer-wins replacement or silent merge of conflicting changes is allowed.

For example, an editor based on revision 10 cannot overwrite revision 11. A disconnected
client retrying the same operation must retrieve its result, not apply the change twice.
Authorization and concurrency checks happen on the backend for every client.

### External YAML edits

Detect file edits through the watcher and fingerprint checks, validate them as external
candidates, and assign a new revision when accepted. Invalid YAML must leave the active
runtime unchanged and be shown as a failed external candidate. An older UI draft must not
overwrite an accepted external change.

A coordinator lock cannot control an arbitrary text editor that ignores that lock. For a
strict guarantee during concurrent writes, management mode must give the coordinator exclusive
write ownership of the live file; operators submit/import YAML through the management CLI/API.
Legacy headless file editing remains supported. Document the ownership policy explicitly;
do not promise race-free arbitrary direct writes based only on a watcher or atomic rename.

## Durable apply, rollback and recovery

Track each operation through explicit prepared, persisted, applying, active, failed and
rolled-back states, with the operation ID, actor, base revision, candidate hash and result.
Durable file replacement must use the same filesystem and appropriate file/directory syncs.
Database transactions, YAML writes and in-memory publication are separate operations; their
coordination requires a journal and startup reconciliation, not a claim of one ACID
transaction spanning all three.

Define and test crash recovery at every transition, including a crash after file persistence
but before runtime activation. Preserve the last known usable configuration and make failure
states visible. Rollback is a new authorized, version-checked operation: it must not blindly
write an old file over newer edits. Validate backup/restore of both YAML and management state,
with migrations, ownership/permissions, retention and secret handling documented.

If management storage becomes unavailable, configuration writes fail visibly and the proxy
continues serving its last active runtime. Do not silently weaken login or authorization.
Bound audit/history growth and UI work so management activity cannot consume unbounded
memory, disk, connections or proxy worker time.

## Authentication and authorization

- Protect the management API as well as UI routes. Hiding buttons is not authorization.
- Start with admin and viewer roles, enforced per operation. Define the access policy for
  machine monitoring and automation independently of browser sessions.
- Store password hashes, not passwords; choose and benchmark the password-hashing policy
  during implementation. Bound login work, rate-limit attempts and audit authentication events.
- Use secure backend sessions with expiry, logout, revocation, session-fixation protection,
  appropriate cookie attributes and CSRF protection for state-changing browser requests.
- Provision the first local administrator explicitly; no shared/default password. Any local
  recovery account is an operator-controlled, audited policy with a documented recovery path.
- Keep secrets out of ordinary config responses, diffs, audit payloads and frontend bundles.
  Specify protected storage and key management before persisting client secrets or tokens.

## Generic OIDC, Authentik and ZITADEL

Implement the panel as a standard OpenID Connect relying party. Configure issuer/discovery,
client ID, client authentication, exact callback URL, scopes and claim-to-role mapping. Use
Authorization Code with PKCE and validate state, nonce, token signatures, issuer, audience
and lifetime. Support bounded discovery/JWKS refresh and key rotation. A successfully decoded
token is not sufficient authentication.

Bind an external identity to issuer plus subject, not email alone. Provider group and role
claim formats differ; mapping must be configurable, deny access when no allowed mapping
matches, and reject privilege escalation. SQLite stores identity associations, local session
state and audit records; the IdP continues to own the user's SSO password.

MFA/passkeys follow the IdP's policy. Where an operator requires a particular authentication
assurance, validate the configured claim/policy instead of assuming any OIDC login proves MFA.
Define refresh, revocation, role-change and logout behavior, including the maximum delay
before a removed role or terminated IdP session loses local access.

Authentik and ZITADEL must each have a real integration test. Additional compliant providers
can use the generic configuration, but named compatibility requires test evidence. References
consulted through Context7:

- [Authentik OAuth2/OIDC provider](https://github.com/goauthentik/authentik/blob/main/website/docs/add-secure-apps/providers/oauth2/index.mdx).
- [ZITADEL OIDC authentication methods](https://github.com/zitadel/zitadel/blob/main/apps/docs/content/guides/integrate/login/oidc/_authmethods.mdx).
- [SQLite's embedded/serverless model](https://www.sqlite.org/serverless.html).
- [Svelte client compilation](https://github.com/sveltejs/svelte/blob/main/packages/svelte/src/compiler/types/index.d.ts).

## Lightweight UI and observability

Start with process/version/uptime, site and pool health, HTTP policy, certificate/ACME state,
cache, request/error/latency metrics and reload outcomes. Show configured policy separately
from measured protocol negotiation; current `http_version: auto` status is a policy value.
Log viewing requires a bounded, authorized API; a filesystem path in `/status` is not a log API.

Initial asset budget: at most 200 KiB gzip for the primary JS and CSS, excluding lazy-loaded
optional screens. This is a design target, not a measured result. Pause routine polling when
the tab is hidden, bound response sizes/history, and define behavior for stale/disconnected
views. Optional long-term metric storage belongs to an explicit telemetry integration.

Measure startup, binary size, RSS, CPU, FD count, request latency and throughput with UI
disabled, enabled but idle, and actively used under management load. Set and publish release
budgets from that baseline. UI/session/database failures must not crash the proxy data path.

## UI product quality release gates

| Requirement | Acceptance evidence |
|---|---|
| Lightweight | Enforce the 200 KiB gzip primary JS/CSS budget in the build; lazy-load optional screens and bound tables, charts, polling and history. Record startup, interaction, browser memory and proxy resource/performance results under a reproducible workload. |
| Powerful | Maintain a capability matrix mapping backend settings and operations to UI inspect/edit support, role permissions and restart requirements. Support everyday site/upstream/TLS/plugin workflows and advanced configuration without silently discarding fields. Scope limits must be visible. |
| Reliable UI/UX | Complete core workflows through real API-backed E2E tests: login, navigation, edit, validate, diff, apply, conflict resolution, rollback and logout. Preserve drafts on validation/network failures and expired sessions; require safe reauthentication before retrying writes. Show success only after backend acknowledgement of the actual state. |
| Consistent and accessible | Use Hallmark with the Conduit design system, consistent controls/messages, responsive light/dark themes and a documented supported-browser matrix. Verify keyboard, focus, labels, zoom/reflow, contrast, error recovery and reduced-motion behavior. |
| Enterprise operation | Verify backend authorization, OIDC/session policy, secret redaction, audit, backup/restore, concurrent writers, recovery and bounded resource use. Publish the scope and evidence for every readiness claim. |

Target [WCAG 2.2 Level AA](https://www.w3.org/TR/WCAG22/) for the management UI, with both
automated checks and manual assistive-technology/keyboard review. Map applicable security
requirements to [OWASP ASVS 5.0.0](https://asvs.owasp.org/), recording versioned requirement
IDs, verification results and reasons for non-applicability. These are acceptance targets,
not present certifications or claims of conformance.

Define severity and the core workflow/browser matrix before implementation. Release with
no known unresolved critical/high security, configuration-integrity or UI/UX defects. Any
failure that blocks a core workflow, misreports active state, loses a draft/change or silently
overwrites configuration blocks release. Other known issues require an explicit impact,
owner and disposition. Test evidence reduces risk; it is not a promise of zero bugs.

Run functional E2E, targeted visual regression, accessibility and long-session/repeated-action
checks, including slow/disconnected networks, request timeouts, retries, expired sessions and
out-of-order responses. Add a regression for every fixed issue, and repeat the relevant
stability/performance and final closure checks before completing a milestone. Hallmark review
is a design gate alongside these behavioral tests.

Draft recovery must respect permissions and secret lifetimes. Preserve non-secret input when
reauthentication is needed; never retain passwords, tokens or secret edits in unprotected
browser storage. Require sensitive input again when safe recovery is not possible.

## Release acceptance evidence

| Area | Required scenarios |
|---|---|
| Configuration parity | Schema/default consistency, full round trips, unknown-field preservation, redacted-secret preservation, pending-restart fields. |
| Concurrency | Two editors, UI versus CLI, imported/external YAML changes, stale revisions, rollback versus a newer apply; no silent lost updates. |
| Retry and recovery | Lost response, repeated idempotency key, crash at each apply transition, disk full, database busy/unavailable, invalid config, restart and backup restore. |
| Authentication | Local login, expiry/revocation, first-admin setup, admin/viewer boundaries, CSRF, unauthorized API calls and login throttling. |
| OIDC | Authentik and ZITADEL, state/nonce/issuer/audience/signature failures, key rotation, role mapping, MFA policy where configured, logout/expiry and provider outage. |
| UI | All UI product quality gates above: Hallmark review, core workflow/browser matrix, accessibility, visual regression, draft preservation and failure/stale states. |
| Proxy compatibility | Existing full application suite and explicit ignored cases, HTTP/2/gRPC, reloads, graceful upgrades, certificate rotation and cache behavior. |
| Stability/performance | Mixed proxy and management load, competing users, soak, bounded history, audit-write failures, resource drain and measured UI-on/off comparison. |

Each implemented fix receives a focused E2E regression; completed milestones also require
the full relevant suite, soak/stability/performance checks and a final closure review.
Publish tested source/binary hashes, dependency versions, workload, duration, raw results
and remaining limits. The existing H2 validation does not establish these future UI guarantees.

## Enterprise scope

The initial design is a single-node management system with explicit production requirements.
SSO, role enforcement, audit and tested recovery are concrete acceptance criteria. HA/multi-node
coordination, replicated state and failover require a separate design for storage, write ownership,
fencing, sessions and recovery; sharing or copying a SQLite file is not that design.
No production/enterprise readiness or provider compatibility claim is complete without the
corresponding acceptance evidence. This roadmap does not deploy or alter the Krynox soak run.
