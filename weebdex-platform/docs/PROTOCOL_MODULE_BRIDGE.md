# Original protocol module integration

## Authority and module boundary

The platform links `manga-node` by Rust path dependency and invokes its repository,
canonical manifest parsers, signed-event validation, `Publications`, `Groups`,
`PeerSync`, guarded `UploadStore`, original backend, and native HTTP listener in
process. It does not replace canonical protocol storage with the platform schema.
The original 52 migrations remain byte-for-byte unchanged.

The explicitly authorized original-source delta is recorded as23 files in
`platform-planning/progress/protocol-round2-lifecycle-delta.json`, with immutable
prior, immediate-before, and authorized-after hashes. The original52 migrations
remain unchanged. Most additional edits are mechanical scoped fork substitutions;
the lifecycle owner is in `node.rs`, and the facade owns source projection policy.
Standalone `serve_prepared` retains its default behavior. The injected listener
shares the same repository pool and existing original storage owner as the platform
publisher; a second physical store owner is refused.

Embedded shutdown closes public admission and idle relay sessions between commands.
Already-owned signed mutation, nested storage/audit/decode work and blocking closures
retain terminal leases after HTTP cancellation or cancellation of an awaiting parent.
The lease drops with the actual future/closure, and contains no retained task-handle
cycle. Shutdown stops maintenance parents and observes their actual joins, keeps
projection/feed infrastructure alive while finite work drains, cooperatively joins
managed serving, then stops infrastructure and observes all remaining descendants.
Worker-stop, finite-drain and final-descendant phases each have a five-second limit;
managed serving retains its own original bounded reader-shutdown policy. The platform
supervisor also applies its configured overall grace. Any unconfirmed phase reports
failure and reconciliation uncertainty rather than successful cleanup. Unexpected
original worker exit withdraws the supervised listener. Platform/original managed-group
key roots must match. Federation proofs use the configured original service-holder
identity, which may differ from the publication operator identity.

Embedded backend commands retain their original GNU timeout/flock/strfry arguments.
A finite scoped child joins input/output/error pipes and terminal process cleanup after
parent cancellation. The leader is observed without reaping while its process-group ID
is reserved; remaining descendants terminate before leader reap. Linux `/proc` is used
to verify no runnable descendant remains. Failed terminal observation closes admission
and is a sticky supervised failure, even after the Rust task returns. Forced runtime
cancellation kills the owned group and never substitutes for a verified graceful join.
Standalone command execution preserves its inline behavior.

Current lifecycle/importer/deletion fixes are source-frozen, awaiting fresh independent
review and serialized compilation. Their new acceptance tests have not run.

## Actual connected pathways

* Explicit owner-role platform `migrate` prepares a visibility function generated
  from the original catalogue policy. Startup verifies the original schema and exact
  prepared policy without DDL; missing, mismatched or wrong-mode policy rejects
  readiness. Native-only preparation hides every source-linked resource. See
  [database role setup](database-role-setup.md).
* The owned importer scans actual signed kind-1002/1001 source events in bounded
  repeating keyset sweeps. A preflight length query selects a <=2MiB prefix before
  loading bodies, so a16-row aggregate cannot wedge the cursor. Individually invalid
  or oversized rows get durable explicit rejection records and cursor progress;
  later sweeps revisit them. Cursor, aliases and rejection visibility share one
  platform transaction. Repeated sweeps cover commits below an older cursor.
* Imported series/chapters, covers, credits, tags, threads, and counters are catalog
  projections. Existing preferred original compatibility aliases map to durable
  ten-character platform IDs. Source publisher identities remain `protocol:<pubkey>`
  unless an explicit operator-provisioned binding exists. Group publication bindings
  refer to existing native attribution groups, without granting those groups source
  membership/signing authority.
* Staff-reviewed native draft Publish now uses the same actual durable signed1002
  command service. Native review is not signing authority: an explicit admitted
  operator binding is still required. Source acceptance survives native rollback,
  but only the identical submitted native version is recovered automatically.
  A changed review requires explicit reconciliation. Pending source acceptance
  does not make the public native DTO readable.
* A new native upload commit uses the original typed CBOR builders and real
  publication service when a publication runtime is explicitly configured and the
  authenticated user has an explicit signer binding. The first upload can publish
  the native series snapshot and its chapter. Native original bytes are preflighted
  before buffers are allocated, checked by the source image parser, staged into the
  original store, and signed using the configured existing identity.
* Durable commands contain the planned alias, stable creation time, input digest,
  source event identity, native session, commit digest and the exact typed native
  commit JSON. Migration74 additionally stores the exact canonical source CBOR and
  pins its existing native original media objects until materialization. Prepared
  title retries replay those captured bytes rather than regenerating a signature
  from renamed authors, changed tags or a different selected cover. The submitted
  title itself is fenced against edits/rejection/removal while its fixed intent
  remains prepared or source-accepted; unrelated author/tag edits remain possible.
  Missing legacy input snapshots require explicit reconciliation. The owner-only
  upload resume response restores draft/order/files and
  discovers expired prepared/accepted sessions. Edit/remove/cancel stays blocked for
  that fixed intent while identical commits can retry. The actual returned signed
  event must match that identity. Commands survive native transaction rollback;
  verified visible source events recover native associations and authenticated
  receipts without waiting for an active native commit. Local tombstones survive.
  A lost title materialization response replays the current actual signed event
  after its temporary cover pins have been released, rather than attempting to
  stage cleared temporary inputs again.
* The authenticated commit/retry reports `original-protocol` with either
  `accepted_pending_projection` or `locally_hosted`. An accepted request is not a
  claim that every peer received it. Without publication configuration, uploads
  remain explicitly `instance-local`; configured but unauthorized publication fails.
  Missing/revoked source signer authority is rejected before allocating a durable
  title command. Historical full DTOs never authorize a retry response: only a
  minimal authenticated pending receipt is permitted after the original module
  confirms still-hosted healthy chapter/parent generations and genuinely pending
  projection, plus current original signer/group authority. Local tombstones and
  source removal/quarantine fail closed.
  The private draft route exposes a small genuinely pending published-title receipt
  only to the actual durable-command publisher with current original admission.
  Other staff and library-only API credentials do not gain that capability.
  Submitted prepared-title DTOs distinguish a durable native checkpoint from source
  acceptance and advertise identical-version retry only while its exact owner,
  configured binding and original publisher admission still authorize it.
* `/data/{chapter}/{name}` and cover URLs check canonical source and parent policy
  before serving. Signed original names are their canonical content hashes, while
  the reader's node stays the platform public origin. Original bytes use the
  original module's download snapshot. Platform request/read admission remains
  attached to the original streaming response body until EOF or cancellation, in
  addition to the original snapshot's own download budget. Repeated hashes preserve
  page positions.
* Native genuine derivatives are independent instance capabilities, leaving signed
  originals untouched. Imported source covers are fetched only from the owned
  original store or the explicitly same-instance original endpoint, hash/MIME checked
  at ingestion, and decoded within fixed budgets into real local 256/512 WebP files.
  Missing derivatives fail closed until materialized. No remote peer is a reader CDN.
* Source-linked public DTOs and media use current primary-database visibility checks.
  Native public media revalidates rather than claiming year-long immutable access;
  original media is `no-store`. Conditional ETags do not bypass removal checks.
* Explicit DELETE of a source-backed chapter validates native delete authority,
  explicit publisher binding or actual original managed-group remove permission,
  persists a real signed request, and submits it to the original moderator-review
  module before applying the local soft hide. Response headers expose the actual
  request ID and distinguish source review from instance removal. There is no
  automatic approval or peer-copy deletion; absent signing authority fails explicitly.
* The original node retains its authenticated approved-peer sync and moderator
  deletion endpoints. The runtime's callable peer seam enforces original approved
  peer policy. A signed deletion request alone does not approve removal.

## Metadata representation

Full source metadata is retained as `protocol_source_metadata`. Native DTO facets
support titles/alt titles/descriptions, original language, year/status/rating,
author/artist credits, category tags, and original cover descriptors. Missing language
is `und`; unknown/missing classification is conservatively `pornographic`. Unsupported
status falls back to native `ongoing` with `protocol_projection_defaults` disclosure.
Native facets omit overlong tag/creator labels and non-author/artist credit roles;
those values remain in preserved source metadata. Unrepresentable essential title,
language, descriptor, and chapter fields are rejected/deferred with durable reasons.

## Production prerequisites

Provision existing identities and permissions yourself through the original
operator workflow. The platform does not generate production keys, admit publishers,
approve peers, bind ordinary logins to signers, or create managed group authority.

Full production requires a verified original schema, TLS PostgreSQL, distinct native
and original storage directories, an explicitly installed original projection backend,
a preexisting admitted publisher with a private restricted signer file, original
storage owner/holder identities, the holder's existing service key, and explicit
native user/group publication bindings. The embedded native listener must be a
separate loopback bind; preserve exact public NIP-98 paths without prefix rewriting.
Deploy the platform and native original protocol endpoints behind the supplied HTTPS
proxy configuration. Production read-only mode needs no signer but requires a
verified same-instance original-serving endpoint. Genuine auth mail configuration is
separately required for account recovery.

Native PNG/JPEG/WebP input can be signed within the original manifest/staging byte
and pixel bounds. Native-only GIF and larger sessions remain supported locally.
Configured source publication rejects unsupported animation/total sizes explicitly;
it does not convert animations or silently mark them federated.

## Explicit browser signer mode

The additive public adapters expose `/api/protocol/capabilities` and a bounded
`/api/protocol/resources` mapping (at most256 requested native manga/chapter IDs).
Only actual current canonical signed events and healthy visible parents are mapped;
native-only, deleted, pending or revoked resources are omitted. Actual source event
ID, kind, publisher and original preferred compatibility alias come from the original
module. The persisted public catalogue namespace is allocated by platform migration73;
a rebuilt native catalogue gets a new namespace, while a database restore retains it.
Browser local data must bind API origin plus this namespace, not infer that ten-character
aliases on different instances represent the same source resource.

The separate frontend adapter reuses the original browser client's local IndexedDB,
store, outbox, publishing and cryptographic verification modules. External signer and
explicit reviewed recipients are required for opted-in kind11999 publication/import.
Profiles0/3/10002 use the existing original `/account-metadata` workflow and current
`/accounts/{key}/metadata/{kind}` reads. Comments1111/reactions7 use `/social` signed
submission and public query; ordinary author kind5 withdrawals use `/account-deletions`.
These exact original HTTP/WS endpoints run in process in the embedded listener.
Native password sessions never authorize any of those external signatures. Original
account admission is a separate prerequisite; no account is automatically admitted.

CSP exposes only exact configured reviewed origins plus the explicitly configured
original listener origin. Direct original browser calls also require its CORS allowlist
to include the exact UI origin. Public capabilities disclose browser transport
availability and the same effective approved connect origins; they never expose keys,
private drafts, signer bindings or native account identities. Final frontend adapter
wiring and synthetic signed transport tests remain unverified at this source-review
checkpoint.

## Exact remaining limitations and verification

Native metadata editing, author/group/tag management, instance community/library,
and editorial soft-removal are platform overlays. A configured, explicitly authorized
native draft Publish and a new native chapter upload both create real signed source
publications. Unconfigured native publication remains explicitly instance-local. Editing an overlay does not mutate an immutable signed source record.
Authorized native chapter DELETE submits the real source deletion request and hides
only the instance projection. Operators approve source removal separately through
the original authenticated moderator workflow. Ordinary source-linked manga/cover
editorial soft-removal remains a local overlay action; it does not invent a revised
source event or automatically delete peers' retained originals.

Current validation receipts and gaps are recorded in
`platform-planning/progress/protocol-bridge.json`. Tests use only owned isolated
PostgreSQL and generated synthetic fixture identities/images. The new integrated
seam uses the existing real strfry binary and exercises native acceptance followed
by native rollback, importer recovery, original bytes/ranges, source cover
materialization, visibility revocation before a sweep, moderator approval, a
single-store embedded listener, bounded shutdown, and approved/revoked peer transport.
It additionally creates a fully separate original authority database, store, and
embedded listener, synchronizes actual signed series/chapters with hash-verified
original bytes, and checks locally retained reader media survives source removal and
source listener shutdown. Authenticated native DELETE feedback is exercised separately
from original moderator approval. Execution receipts are still required before
calling these pathways verified. No
production deployment or complete protocol/frontend parity is claimed.

The combined original-backed local-first/NIP07 browser fixture is specified in
`PROTOCOL_BROWSER_FIXTURE.md`. Its provisioner and supervisor extensions are a
separate unexecuted acceptance gate; the existing native-only browser fixture cannot
establish signed library or profile/social delivery.

## Current native deletion intent generation

A native chapter DELETE is bound to the actual currently visible original event
version under an original-owned source row lock. Migration84 retains historical
intents as generation0 and stores a distinct durable UUID for each current action.
The signed request has exactly one target and contains that UUID and source generation.
Uncertain managed-group responses recover only this exact original signed body;
arbitrary old or multi-target requests cannot satisfy a new native action after
source restore/rehost. Original current group/member remove authority is rechecked.
This submits moderator review and soft-hides the instance overlay; source approval
and other instances remain governed by the original protocol.

A genuine managed-group restore/rehost native DELETE public-seam fixture still
needs authoring and execution. Other execution gates include disconnected blocked signed mutation terminal drain, source-heavy import,
idle WebSocket shutdown, and the combined original-backed signed browser fixture.
No execution success is implied by source-only tests or an older compiled binary.
