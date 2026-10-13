# Deployment and production readiness

## Current claim boundary

The Rust platform and retained original `manga-node` module implement the
runtime controls described below. A successful source build is not evidence of
production operation, feature completeness, security verification, measured
performance or successful public deployment. Final validation started only after implementation. A disposable live HTTP
smoke caught a SPA deep-link404, now corrected with regression source. Final
corrected HTTP/browser acceptance is paused for the requested wiring review;
this document does not upgrade that partial evidence to an end-to-end pass. No accounts, credentials, publishing
identities, cloud services, DNS, TLS certificates or public deployment are
provisioned by this implementation.

### Implemented runtime controls

- Explicit `production: true` validates HTTPS public origin, TLS database mode,
  genuine mail configuration, absolute runtime paths and drain limits. Full
  protocol mode requires an operator-configured publication runtime and supervised
  original native listener with its sole projection backend. Explicit
  `protocol_read_only: true` requires a self-hosted original reader on the UI
  origin and does not require a publisher signer. Read-only mode cannot publish
  canonical originals and is not advertised as a full-feature publisher.
- Loopback-only API listener; API/auth errors remain JSON and never become SPA
  success. The six actual retained `/auth/login`, `/auth/register`, `/auth/logout`,
  `/auth/recovery`, `/auth/verification`, `/auth/error` deep links serve their
  client page. Navigation fallback needs HTML navigation; absent reserved assets,
  service-worker/manifest files, misrouted canonical `/originals` URLs and templates
  return real 404. Recovery Go
  templates are not publicly served.
- Public UI and private native/protocol storage roots cannot overlap. CLI
  preflight requires a recognized bounded SvelteKit bootstrap and rejects public
  artifact symlinks/special files. Treat the entire UI tree as an immutable public
  deployment artifact; never put secrets or runtime files there.
- Original `Repository` owns the shared bounded PostgreSQL pool. Its existing
  connection/TLS policy has 5 s statement, 1 s lock, 10 s idle-transaction limits and
  bounded checkout/create/recycle deadlines. Publication runtime requires at
  least 4 shared connections; default 8. Explicit platform migration is separately
  checksummed, transactional, advisory-locked and limited to 60 s statements/3 s
  locks. It does not alter the exact original protocol migration history.
- API admission has 32 active requests by default, fail-fast saturation 429 with
  Retry-After and original class-based bounded read admission. Owned permits
  remain held after client disconnect until admitted handler work finishes.
- JSON/auth body budget is 1 MiB with 30 s total intake deadline by default. Multipart
  chapter upload is 96 MiB and cover upload 40 MiB with 180 s total intake deadline.
  Atomic `POST /api/upload/{session}/files/{file}/replace` has its own exact
  40 MiB multipart proxy/core budget and upload intake deadline. The nginx
  example preserves its URI and `X-WeebDex-Upload-Version` header while keeping
  the generic JSON cap at 1 MiB. Operational acceptance must exercise a real
  encoded replacement larger than 1 MiB through the actual proxy.
  The body wrapper streams without whole-upload buffering and preserves
  backpressure. Upload module adds 32 MiB/file, image decode/pixel/frame limits,
  bounded decode slots and its own field/file deadlines. Intake timeout does not
  cancel or report completion of a database transaction.
- Central client-peer extraction defaults to actual transport only. Explicit
  `trusted_proxy_peers`/exact `trusted_proxy_hops` accept only a single bounded
  edge-overwritten XFF field from trusted immediate/intermediate proxies;
  malformed/ambiguous chains fail400. Analytics120/minute is per validated client,
  not the nginx loopback address. Auth's independent global/browser/identifier/mail
  bounds remain intentional. See `trusted-proxy-peers.md` for the trust boundary;
  the supplied nginx policy overwrites XFF and supports one proxy hop.
- Same-origin-only credentialed CORS, rejection of external/null/duplicate Origin,
  validated preflight methods/headers, production Host validation, existing
  flow/session-bound CSRF and bearer authorization policies. Browser session
  authority does not silently become Nostr publication authority.
- CSP computed from exact integrated bootstrap hashes, no arbitrary script or
  analytics origins, nosniff, frame denial, same-origin opener/resource policies,
  limited browser permissions, production HSTS and private/API/auth no-store.
  Handler-specific stricter CSP/no-referrer headers are preserved.
- `/livez` is process liveness without database dependency. `/healthz` and
  `/api/health` report database reachability. `/readyz` only becomes 200 after
  schema/runtime startup and withdraws on drain. Full production checks original
  schema and configured publication availability. A read-only readiness response
  describes its limitation; remote endpoint health is an operator check.
- Graceful SIGINT/SIGTERM withdraws readiness, drains Axum connections and then
  waits owned request permits so disconnected writes are not abandoned merely
  because their network connection is gone. Default drain 210 s; a deadline failure
  exits unsuccessfully and reports unresolved work, never claims commit success.
  Durable upload pending markers, cleanup ledger and publication command journal
  support restart reconciliation. A single owned maintenance task retries bounded
  cleanup batches every 30 seconds and removes bounded regular temporary upload
  files older than 24 hours; it never scans published original storage. Both listeners are supervised;
  unexpected native listener exit withdraws
  readiness and stops the instance. Platform worker handles are explicitly owned and joined
  or aborted/awaited during shutdown. The embedded original worker-owner
  shutdown is undergoing a separate source audit before final lifecycle evidence.
- Structured logs omit request bodies/cookies/credentials/DSNs. Example proxy
  access format omits query strings, including auth recovery/verification tokens.

## Release and runtime entrypoints

Preserve the layout in which `weebdex-platform/backend` depends on the adjacent
`manga-protocol-lab/server` path. Use the tracked Cargo lockfile and unchanged
frontend Yarn lockfile. The reviewed restore/build instructions are in
`frontend-preservation.md`; use Node 24 and Yarn Classic 1.22.22 with frozen
lockfile and disabled dependency lifecycle scripts. The supplied build worked
from the externally restored dependency tree; this is not a reason to copy
unknown dependency caches into production.

After all final tests/contract/parity checks pass, run deliberately:

```sh
WEEBDEX_FRONTEND_ROOT=/absolute/verified/frontend scripts/build-release.sh
```

This invokes the external same-origin frontend integration wrapper and
`cargo build --locked --release`, creates the frontend and Rust executable
artifacts and records SHA-256 hashes/tool versions in
`artifacts/release-manifest.json`. The receipt states its validation boundary.
Pin the build host/tool versions when reproducible bytes matter; this script
establishes a repeatable locked build procedure, not proof of bitwise determinism.
No software/dependency install is done by the release script.

Install artifacts into a new immutable release directory and use an atomic
operator-owned current-release switch. Provision writable private native/original
storage separately, with adequate disk/inodes and tested backup/restore. Review
`deployment/weebdex.service.example` and `deployment/nginx.example.conf`; these
are templates, not verified installed services.

Inject runtime JSON and secrets through the service manager or approved secret
store. The only database value is `PLATFORM_DATABASE_URL` (legacy
`MANGA_DATABASE_URL` fallback); it is not a JSON configuration property. Mail
configuration names a bearer-token environment variable. Existing signer/group
keys are private runtime files outside the source, artifacts and public UI,
mode 0600 or stricter. Never generate/commit keys automatically. Do not print the
environment, DSN, secret file or bearer token in test evidence/support logs.

Examples intentionally use reserved `example.invalid` domains and invalid public
identity placeholders. Replace them in private operator configuration. Publisher
`check-config` fails until identities are valid, and full startup proves the
existing admitted signer and storage authority through the original module.
No API call grants those authorities implicitly.

```sh
PLATFORM_CONFIG=/etc/weebdex/platform.json scripts/run-platform.sh check-config
PLATFORM_CONFIG=/etc/weebdex/platform.json scripts/run-platform.sh verify
PLATFORM_CONFIG=/etc/weebdex/platform.json scripts/run-platform.sh migrate
PLATFORM_CONFIG=/etc/weebdex/platform.json scripts/run-platform.sh serve
```

`check-config` only inspects configuration/artifact files. `verify` connects and
checks schema and the exact prepared visibility policy without DDL or opening a
publication ownership runtime. `migrate` is an explicit privileged platform-only
operation, including policy preparation, never an automatic startup side effect.
Use separate owner/runtime DSNs and the grants/order in
[database role setup](database-role-setup.md).
Original protocol migrations must first be applied through its separate reviewed
`manga-node migrate NODE_CONFIG` workflow, against the same intended database.
Use a backed-up staging copy first; there is no automatic destructive downgrade.
Backward binary rollout is rejected if its migration manifest differs.

## Original media and protocol reverse proxy

Use a separate native protocol host to avoid collisions between root relay and
client frontend routes. Its public `base_url` must match the configured HTTPS
protocol host exactly. Forward the original URI, method, body and Authorization
without a path prefix rewrite; NIP98 signs their exact binding. The example
preserves WebSocket Upgrade, disables proxy body/response buffering and forwards
Range/If-Range/If-None-Match with original 206/304/416 and length/ETag semantics.
Do not terminate canonical original ownership with an unrestricted filesystem
alias or route a remote peer as the user's reader CDN.

A limited read-only platform uses its own same-origin `/originals/<64 hex>` proxy
route to the user's actual original node. The platform verifies event visibility,
parent series and exact image descriptor before redirecting there. Native
publisher serving should use the one injected, owned original storage instance,
not a second original-node process mounted on the same physical volume. Configure
original explicit CORS/read-policy/enrollment/admin settings for the intended
instance; the original node's development defaults are not a production policy.

## Operator prerequisites, still required

- Provide a tested PostgreSQL deployment, certificate trust roots, least-privilege
  service/migration roles, migration staging/backup and restore verification.
- Provision original service/admin/publisher identities, admission and group
  bindings through authorized original workflows. Provide an installed original
  projection backend when its operations are required; no backend binary is
  invented or installed. Use exactly one physical-volume mutation owner.
- Supply a real mail endpoint implementing the documented accepted-delivery
  contract, sender domain and credential; verify actual delivery, verification,
  recovery and link origin. Enable registration only deliberately.
- Supply DNS/certificates, reverse proxy, closed loopback ports, management network
  policy, process manager, private data permissions and log retention/rotation.
  Change example `TimeoutStopSec` if changing application drain. Proxy body limits
  must not fall below route-specific supported upload limits.
- Review permissions, user bootstrap/staff policy, content licensing and original
  node instance/relay/read policies. Public-source provenance is not a publication
  license. Establish operator runbooks for disabled identities/storage faults,
  failed publication intents and manual recovery, without replaying an uncertain
  user mutation blindly.

## Final validation, not yet proven here

The shared compiler has built all targets and the normal debug executable.
Compiler revalidation is still required for the latest operational audit deltas;
no browser acceptance or corrected full HTTP smoke pass is asserted here.
Both owned disposable fixture attempts have terminal cleanup receipts with
platform/PostgreSQL exit0 and no forced cleanup. Required release gates include config/
origin/CSRF/path traversal/deadline/drain tests; actual PG fresh/upgrade/concurrent
migration and auth/revocation tests; multipart/range/conditional/body-cancellation
media tests; original module regression suite and native relay/listener lifecycle;
all visible API/UI feature and repeated/expired/error flows; immutable frontend
source parity and rendered desktop/mobile behavior; genuine mail integration;
proxy configuration/TLS/WS/range checks; backup/recovery drills; and representative
bounded-load measurements with dataset/hardware/error/latency/memory evidence.
No throughput, zero-downtime, durable-volume recovery or end-to-end production
claim is justified until those gates are actually run and recorded.
# Reviewed browser transport and account constraints

The platform CSP hashes only its served Svelte bootstrap and blocks upstream
analytics scripts, frames and beacons. `protocol_connect_origins` is a bounded,
explicitly reviewed HTTPS/WSS origin list (loopback HTTP/WS only in development).
The configured same-instance native origin is added through that same validator.
Relay origins get connect access only, never script/frame or image access. A single
configured, explicitly self-hosted original origin may be an image destination;
no remote metadata or arbitrary peer origin grants that permission. Production
operators must own and authenticate that native origin. Media redirects remain
descriptor/visibility checked, and same-origin reverse-proxy delivery is preferred.

Cross-origin native signed browser transport requires the original node's exact
CORS allowance for the platform public origin and the configured public counterpart
read policy. Capability discovery reports actual configuration/readiness rather
than creating signer/account admission. Library bearers expose only current
`self.library.read` authority and do not inherit native staff roles, email, drafts,
staged media or mutation capabilities.

Device-to-account adapters send `X-WeebDex-Expected-Account` on same-origin account
calls. It only constrains the resolved current account; a tab's cookie switch gives
409 before the read/mutation can silently change owner. Missing or revoked
credentials still fail authentication. This header must not be sent to external
signers, relays or original-module transport as an authorization mechanism.

`service-worker.js` is delivered with `no-cache, must-revalidate` so existing
controlled clients can discover privacy/cache-purge updates promptly.


Private runtime path preflight resolves existing symlink ancestors even for
uncreated final directories. Uploads, original storage/backend data, operator and
service signer files, and both configured group-key directories must be disjoint
from the public UI root, including equality and ancestor overlap. Startup checks
this before opening workers; restrictive file modes do not substitute for this
serving boundary. The provisioned UI is scanned for symlinks and special files.

Configured UI/private paths reject parent-directory (`..`) components, including
nonexistent/symlink mixtures such as `missing/../alias/future`. Use explicit
normalized paths. Development defaults are relative to the project root:
`artifacts/frontend` and `runtime/uploads`; production examples use absolute
provisioned roots. Native staging now journals before final writes and propagates
file/directory fsync failures before committing references. Supported storage
must provide local fsync and OS-lock semantics; no power-loss pass is claimed.

Original-source cover derivatives use the same durable object-write path as
native originals and derivatives: locked key-only intent before final creation,
file and parent-directory sync before committing SQL references, object advisory
locks, and retained bytes/intent for uncertain commits. No parallel unjournaled
source-cover writer remains. Writers verify the held lease still names the actual
journal/intake inode before continuing, so a long pause before lock acquisition
cannot resume against a name that maintenance already reclaimed.

The separate `protocol_publication.backend` platform worker alternative is
retired and rejected in development as well as production. Configure the
supervised original `native_node.backend` instead; no unsupervised platform worker
can bypass the embedded original lifecycle drain. Original standalone node
behavior remains separate.
