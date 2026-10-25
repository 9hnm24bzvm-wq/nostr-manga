# Native-account automatic Continue, v1

Status: source-authored implementation and acceptance controls. Not compiled,
executed, deployed or performance-qualified by this change. The reader consumer
must be reviewed and qualified separately. Device history stays on its working
retained path and must never be sent to this endpoint.

## Authority and transport

GET `/api/user/history/manga/{nativeMangaId}/continue` accepts only a normal native
session cookie and exactly one `X-WeebDex-Expected-Account` header. Bearer,
signed, API-client, recovery, revoked, expired and disabled authority is refused;
normal owner-library access is preserved for restricted/unverified accounts.

Required single query controls:

- `accountEpoch`: canonical unsigned JavaScript-safe integer, including zero.
- `catalogNamespace`: canonical lowercase hyphenated UUID.
- `serviceKey`: lowercase 64-hex public key, or literal `null` for native-only.
- `requestId`: 1..64 ASCII letters, digits, underscores or hyphens.

Only existing native chapter filters are accepted: `tlang`, `tlangx`, `uploader`,
`uploaderx`, `group`, `groupx`, `id`, `manga`, including existing array/repeated
forms and native validation limits. The complete query is at most 16 KiB.
Client history/completed maps, ordering, current/group references, cursors,
offsets, limits and pages are refused. UI preference/explicit-URL precedence
remains the consumer's existing responsibility, not a new backend override.

Unfinished request bodies, even zero-size-hint or Content-Length-zero bodies,
are rejected without polling; ordinary ended empty GET remains legal. Outer
router LibraryHead/request admission and private no-store/Vary apply to all
refusals, including capacity, malformed requests, CORS and unfinished bodies.
Expected-account GET CORS is added only on this native route. The previously
admitted completion route retains its exact CORS/privacy behavior. There is no
public-ID or signed Continue route and no global header/method widening.

## Snapshot and selection

The eight-second operation includes pool acquisition. One RepeatableRead
transaction sets two-second statements, 250 ms lock timeout, three-second idle
transaction timeout and UTC. It is not PostgreSQL READ ONLY, because authority
is fenced by row locks, but contains no mutations. The shared principal helper
performs candidate-token lookup, user SHARE NOWAIT, then exact-session SHARE
NOWAIT, with expected-account matching. Catalogue namespace and configured
service identity are checked in this same transaction. Lock/deadlock/
serialization states 55P03, 40P01 and 40001 remain conflicts; ordinary SQL/timeout
failures are unavailable. Locks last through commit, not response delivery.

Parent publication/deletion/live-source visibility is required, independent of
whether the supplied chapter filters yield anything. Missing/hidden parents
return 404. Eligible chapters use native chapter filters and source visibility,
not public availability, empty-page, future-date or content-rating defaults.

`catalog/reader_selector.rs` owns selection and reuses its private group equality,
native order, edition order, projection, DTO and witness helpers. The eligible
CTE contains every eligible chapter with this account's exact chapter-and-manga
history membership. Positions group exact nullable `(volume, chapter)` labels
and compute `bool_or` membership. An eligible alternate edition completes its
whole group. `progress.completed` is irrelevant. Filtered-out, deleted, withdrawn,
foreign-parent and other-account history cannot complete an eligible group.

Choose the oldest incomplete group; if every group is completed, choose the
newest group. Empty/no history therefore chooses oldest. Resolve the winner
across the complete selected eligible group. An independent LIMIT 2 probe
produces multiplicity without bounding the winner or the eligible set. There is
no preliminary 10K, page or edition cap. At most one winner row leaves SQL.
This is a result/transport bound, not a bounded-work or large-scale latency claim.

Raw nullable labels stay raw: null, empty, `02`, `2`, `2.0` and special labels are
not merged or normalized in JavaScript. Native prefix-NUMERIC ordering and actual
database label collation are preserved. Edition order remains JavaScript
Object.entries promotion of canonical array-index keys first numerically, then
ordinary native keys in the retained BTreeMap/C byte order, not public order.

## Response

Successful envelopes contain exactly:

- `version: 1`, `operation: "native-continue"`, `orderingProfile: "native-aggregate-v1"`.
- `mangaId`, `catalogIdentity`, `account: { id, epoch }`, `requestId`, `scope`.
- `consistency: "live-request-v1"`, `continueProof: "native-account-snapshot-v1"`,
  `crossRequestSnapshot: false`.
- `state: "ready"` or `"boundary"`; reason `oldest-unread`,
  `all-completed-newest` or `empty`.
- `target: { volume, chapter, anchorChapterId }` or null, `winner` or null,
  `hasMultipleEditions`.

Winner is the existing minimal bounded native reader DTO. Its visible group
identities are capped at 100 with refusal, never truncation. The entire serialized
Continue envelope has its own fixed 32 KiB ceiling. Completion and forum 16 KiB
limits are unchanged; the consumer needs its own 32 KiB transport allowance.

Scope hashes catalogue identity, native profile, operation, manga, account and
canonical effective filter sets. Epoch and request ID are echoed generation
witnesses, not authority, catalogue/history revisions or durable snapshots. A
new request can legitimately choose a different answer under live drift.
Multiple editions signal the bounded chooser; exactly one can navigate. An
empty boundary must not reuse a previous target. The proof applies only to this
request's complete eligible set and native account history.

## Authored controls and outstanding qualification

`backend/tests/reader_continue.rs` contains ignored real loopback HTTP/PostgreSQL
controls for empty/partial/all membership, alternate completion, progress
non-equivalence, raw labels/native index keys, over-10K groups and editions,
filters/visibility/foreign history, cookie authority, admission, private CORS,
unfinished no-frame bodies and real HTTP/1.1 chunked intake, group-identity
budgets, pool waits, contention in both directions, namespace/history drift and
an authority serialization race. It also installs the real protocol visibility
function over migrated synthetic ledger rows and transitions a source from
hosted to withdrawing/removed. This is policy-state test data, not signed
publication evidence. Production authority tables are not altered by the API.

The parity helper `backend/tests/native_continue_reference.mjs` extracts and runs
the actual composed retained MangaActions selection block. Small-fixture rows
come from the actual native aggregate HTTP route, so label ordering is not
reimplemented. Above the legacy aggregate cap, explicit expected fixtures
exercise the real new HTTP/SQL route rather than pretending the old aggregate
can enumerate beyond its unchanged limit.

Run only after separate admission through the owned disposable PostgreSQL
runner, with the complete composed source and Node available. Use serial
`--test-threads=1`: causal races briefly gate shared fixture tables and do not
relax production deadlines. Their gate must release inside 250 ms or the control
fails closed. Unit controls cover request grammar, SQL structure, envelope
scope/nullable identity and refusal beyond the serialization ceiling.

Compilation, SQL syntax/plans, all authored tests, HTTP/2 wire qualification,
large-set latency under the unchanged two-second SQL budget and final consumer
integration remain unexecuted. No test pass is implied by source presence.
