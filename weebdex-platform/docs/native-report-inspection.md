# Signed personal native report inspection

This slice adds a separate private, read-only interface for native thread and
reply reports. It does not add these categories to the public UUID report API,
modify the native-cookie report interface, expose a moderator queue, or create
protocol moderation events. Original supplied frontend sources remain unchanged.

## Routes and authority

- GET `/api/v1/nostr/account/reports/native?expectedAccountId={uuid}` lists only
  reports submitted by the actual signer-bound native account.
- GET `/api/v1/nostr/account/reports/native/{nativeReportId}?expectedAccountId={uuid}`
  reads one report under exactly the same owner predicate. Foreign, deleted,
  non-thread/reply and absent report IDs all return the same404 response.
- `expectedAccountId` is mandatory on both routes. It is a captured-account fence,
  never an authority selector. Authentication is the existing admitted Nostr
  request capability, rechecked by `bound_account_read` in the same repeatable-read
  transaction. The preferred actual account UUID comparison precedes the report
  count, selection, and detail lookup. Stable stale captures return409 without
  rows or counts, including empty pages and unknown detail identifiers.
- Moderator roles and report.update do not widen the owner predicate. Reading
  existing personal submissions does not require report.create. A cookie cannot
  provide or change the selected actor. Existing admission/binding/account locks
  remain transaction-owned; there is no deserialized or fabricated principal.
- Both successful and error responses use `private, no-store` and vary on
  Authorization and Cookie. Errors do not disclose database errors or raw DTOs.

## Bounded, deterministic projection

List pages have exactly five available row slots. `page` is1 through2001, with
maximum offset10000. Supported optional filters are category (`thread` or
`reply`), status (`waiting`, `accepted`, `refused`, `autoresolved`), native
`objectId`, and native `reasonId`. Unknown, duplicate, malformed, empty or
out-of-range fields fail. Detail accepts only the mandatory account fence.
The fixed order is creation timestamp descending and native report ID descending
with C collation. Six fractional UTC timestamp digits preserve the ordering
information rather than silently collapsing sub-millisecond ties.

Count and rows use one repeatable-read snapshot. The shared moderation module's
narrow personal projection uses an unconditional actor ownership predicate and
never calls the broad native report_json projection. A left reason join preserves
any malformed selected row so it fails the whole page rather than disappearing.
Native ID grammar/128-byte bounds and reason label/version/category are validated.
SQL guards persisted TEXT before materialization: report details at most20000
UTF-8 bytes (the native database constraint), reason text1 through8000 UTF-8 bytes.
There is no truncation. Five rows fit even the maximal JSON-escaped body/reason
case; the reused bounded writer also checks the exact complete1MiB response.

Output contains only native report ID, category, historical native subject ID,
status, timestamp, the owner's submitted details, and current reason ID/label/
version/details requirement. No reviewer, notes, audit, effects, account contact
information, target content or target expansion is returned. A missing or hidden
target does not erase the owner's report; its historical identifier makes no
claim that the target still exists or can be opened. The displayed reason is
explicitly the current revision, not a fabricated submission-time snapshot.

## UI and lifetime

The additive `/reports/signed/native` route is outside the native `/my` cookie
gate. The same page's `reportId` parameter selects a detail read and cannot be
combined with list filters. Signed catalogue inspection and the native-user
signed discussion link to it. The browser-login report link remains explicitly
separate, with no silent actor switch. Native IDs are labelled as native IDs,
not converted into public UUIDs. The bound account name and UUID are visible.

The shared signed-account scope captures signer generation, service key/origin,
catalogue namespace and account before reading; it verifies them again after
reading, even for an empty page. Route generations, cancellation and signer
changes invalidate the displayed snapshot and abort requests. Filter changes and
Back/Forward navigation never cache private rows in browser history. Initialization
and inspection are explicit user actions; there is no anonymous signed fallback.
Unexpected errors are mapped to safe fixed UI messages, never arbitrary signer,
transport or server text. At page2001 with further rows, the UI explicitly says
offset10000 is a pagination limit rather than the end of report history.

Inspection neither accesses nor removes pending-write fences. Matching rows,
empty lists, changed status and details do not reconcile uncertain submissions.
No POST, replay, retry-write, moderator action or reconciliation control is added.

## Qualification status

Backend source review: the integrator reported no blocker in frozen2ce19be.
Authored real-HTTP test target: `backend/tests/public_native_report_inspection.rs`.
It uses only synthetic accounts/keys and an isolated runner, covering signer proof,
owner/moderator isolation, required account fence before empty/detail reads,
private error response boundaries, filters/pages, missing targets, reasons,
full escaped text and malformed projection refusal. Compilation and execution
are pending the separately owned Rust/PostgreSQL slot; authored assertions are
not runtime evidence.

Finite controller tests: `tests/ci/test_native_report_inspection.mjs`.
Source-mounted Svelte route coverage is added to
`tests/ci/test_public_report_draft_dom.mjs`, including signer-only/native-cookie
separation, detail, cancellation, binding and route changes, safe errors,
pagination and uncertainty fences. Their transport is synthetic, not proof of
real HTTP authentication or native-browser rendering. Execution and final
composed strict/build qualification are separately recorded when actually run.

Finite execution at source62f7944:282/282 focused cases passed, including102 new
controller and12 new mounted native cases (38 mounted cases total). Original
frontend git tree identity is unchanged. See
[receipts and limitations](../test-evidence/native-report-inspection-20261022/README.md).
The first mounted attempt did not launch because its declared evidence directory
was absent; the preserved receipt is not counted as a test failure or pass.
