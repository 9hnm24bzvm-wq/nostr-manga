# Signed local chapter pages: source-only candidate

Base: f7c598cb57f9a2afbecc107c1d81bb4b30ac4102. Backend prerequisite:
bc37d6196d4054c068aab8d3a1ac0cda57dfdf41. No test, build, service,
network, browser or publication execution was admitted for this candidate.
All tests below are authored controls, not passing evidence.

## Integration and authority

The existing public upload route overlay still selects PublicChapterEditor. That
component now mounts PublicChapterPagesEditor, which reuses the retained native
UploadChapterPages through the existing exact-hash public overlay. Original
frontend attachment bytes, source hashes, composed/output overlay hashes and
native cookie mode are unchanged. Async chapter account setup opts into explicit
caller-managed owner disposal via its already-registered onDestroy; ordinary
synchronous upload owners retain default component lifecycle registration. The
mounted fixture now uses actual Svelte onDestroy, not a lifecycle no-op. The panel establishes the existing mutex,
upload-session and signed-owner contexts. Native page input/admission, original
bytes, finite per-page/session budgets, upload progress, cancellation and private
preview cleanup stay in the existing page component/adapter, not a second uploader.

The SQL chapter selection appends `_local_page_source_mutable` after native JSON
metadata merging, from the stored `protocol_event_id IS NULL`. The public DTO
exposes `localPageSource=local|protocol`, a source eligibility fact only. Missing
facts fail closed; absence of `nostr`, source URLs or client properties is never
interpreted as local authority. Fresh bound-account grants, exact uploader/group
roles, chapter version, target visibility and session ownership remain separate.
The existing catalog writer still refuses protocol-backed chapters at admission
and atomic commit. No original signed entry is rewritten.

`replace` is a strict optional boolean, default false, on signed edit admission.
It forwards to the same core used by native cookie begin-edit: false copies
original page references, true starts empty staging. No per-file destructive
emulation occurs. The successful signed admission receipt binds chapter UUID,
version and replace. The existing private snapshot includes stored `editVersion`.
The current live pages are unchanged on begin, cancellation, failed commit and
unconfirmed commit; only the existing successful atomic commit replaces them.

## State and recovery

The parent starts metadata/claim/removal locked until current owner staging has
been inspected. Any restored session or unresolved outcome retains that lock on
remount. The adapter also rejects chapter metadata commands when it owns an
active page session, and page writes while a chapter command is unconfirmed.
The native page panel and buttons share a mutex. No mount/unmount silently
cancels or creates a session. A foreign session is shown with its original editor
link, not deleted. Reordering is explicitly saved with Save staged order; navigation
blocks unsaved order/local page work. Save Pages first persists exact acknowledged
order and draft, checks current local permission/version, and requires the terms
checkbox. Upload scheduling is not offered for this local page-edit controller.

An admission uncertainty marker stores only identifiers/version/mode in the
existing same-tab signer/service/catalogue/account recovery store. A positive
unsent transport result or complete typed refusal can clear it; later read failure
cannot. Recovery requires an already witnessed accepted receipt binding the exact
session ID, chapter, version and mode, then the exact session/parent/editVersion
snapshot. An unknown POST with no accepted session ID remains fenced even if
current owned staging matches chapter/parent/version. Its separate read-only
inspection does not adopt that staging as proof of the original admission or
infer mode from its pages. Another tab could have admitted the opposite mode;
the core's open-owner check and unique open-owner index prevent two concurrent
open sessions, but an original delayed POST may admit after the other session is
cancelled. No durable begin attempt/mode lookup exists, so fully resolving an
unknown admission without its accepted receipt remains pending a separately
reviewed durable backend mechanism. No cancellation or replay is offered through
that fence. No-session and contradictory receipts also leave it intact.
This is same-tab/remount recovery, not a new
cross-tab or browser-restart guarantee. Existing commit/replacement fences are
preserved. A local commit response must bind the same chapter and expected next
version; contradictory or missing accepted receipts stay fenced. Explicit saved
receipt reconciliation is read-only and never retries commit.

## Authored controls and required execution

- `test_signed_local_page_editing_dom.mjs`: mounted actual parent/child Svelte,
  actual page controller and signed adapter with finite fake transport. Only the
  complex native image-grid child rendering is stubbed. Covers explicit source
  facts, roles, both choices, double-clicks, terms, metadata exclusion, cancellation,
  unknown begin/commit across remount, contradictory receipts, changed identity,
  service, account, grant and version, and disposal before dispatch.
- `test_public_upload_adapter_recovery.mjs`: additional actual-function tests for
  mode/receipt binding, original staging versus empty replacement, explicit source
  refusal, known accepted-ID recovery with stored editVersion, unknown admission
  versus a different opposite-mode same-target session, and wrong commit target fences.
  Existing original-byte, cap, preview and replacement controls remain required.
- `backend/tests/public_uploads.rs`: two added isolated real-HTTP fixture tests.
  They exercise native and signed same-core default/false/true admission, malicious
  metadata overwritten by stored provenance, refusal of writable source facts,
  copied originals/private preview, cancellation, stale/foreign/no-grant cases,
  grant revocation before commit, exact local atomic replacement without original
  publication commands, and a genuinely published protocol-original refusal for
  both modes. Existing isolated fixture guards and STRFRY requirement remain.

Run after separate resource admission, against this exact candidate in a complete
qualification tree. New mounted and actual-adapter controls support
`WEEBDEX_LOCAL_PAGE_EDIT_CONTROL_SOURCE` pointing to the predecessor public-api
source directory. The predecessor must fail positive page-entry/mode/recovery
controls for the missing implementation, while supported negative/default behavior
must remain discriminating. Do not count a missing fixture/dependency or compile
error as a passing old-source counterfactual. Current source needs strict Svelte/
TypeScript, existing metadata/claim/removal and upload regression suites, exact
679-original-byte and overlay hash checks, composed frontend/backend builds, and
real browser page-management acceptance. Nothing here completes those stages.
