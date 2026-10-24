# Bounded reader consumer source, 2026-10-22

## Status and source boundaries

This is source-only work. No test, compiler, build, browser, service, publication,
or installed dependency was run or created by this lane. Independent source
reviews are not runtime qualification. Frozen main `f7c598cb57f9a2afbecc107c1d81bb4b30ac4102`
and its historical evidence remain unchanged. The backend contract successor is
`4ef72614477eb93ccc50e2957eb9dc3b3ba428b9`; its Rust/HTTP cases remain unexecuted.
The earlier seven retained-order JavaScript goldens are historical backend-lane
results, not results for this consumer source.

Nine actual native consumer files are adopted through original-hash checks and
exact anchors: context, provider, chapter selector, edition selector, pagination,
page selector, keyboard listener, end-card and MangaActions. Twelve explicitly
listed markup exceptions cover only owned selector/control substitutions and the
two incorrect page-selector underline expressions. Canonical native originals
remain immutable; no aggregate/index/length compatibility object is introduced.

Native reads use build-configured PUBLIC_API_URL and retained same-origin cookie
semantics. Public reads use fixed same-origin /api/v1 with credentials omitted.
Identity discovery uses the exact same-source capabilities endpoint, without
cookies. Its request shares the initial action budget and transport slot. Every
selector state requires the actual server catalogIdentity to match the first
verified origin/namespace/service key before rows, boundaries or history are
accepted. Failed verification does not erase that remembered identity. Positive
identity change requires a reload, never silent reuse of mounted chapter IDs.

## Authored qualification inventory: 109 cases, zero executed here

All files are under tests/ci:

- test_bounded_reader_shared_owner_equivalence.mjs: 26 cases. Executes exact old
  forum source and current shared transport/wrapper, including concrete error
  classes/status, fixed allocation, raw fragment/byte limits, healthy EOF,
  no-abort 404, delayed fetch/read/cancel settlement and retained active slots.
- test_bounded_reader_controller.mjs: 45 cases. Executes actual controller,
  public query translator, shared transport and minimal DTO validator against
  synthetic HTTP JSON bytes. Includes native/public request and order profiles,
  actual retained comparator goldens, full-set winner independent of edition
  page, 10,001 groups without retaining their rows, target witnesses, Start
  edition choice, title query overrides, observed-only Continue, history failure
  and cancellation retries, identity rejection in every state, and held/rejected
  router navigation with epoch fencing.
- test_bounded_reader_history_owner.mjs: 12 cases. Executes actual direct-key
  history owner and actual IDB request/terminal helpers. The event-controlled IDB
  adapter is synthetic. Covers 50-key cap, same-transaction revision/scope,
  unavailable/partial/account history, ignored abort, remount exclusion, delayed
  actual terminal, stale result rejection and cancellation settlement.
- test_bounded_reader_consumers.mjs: 15 cases. Composes current native/public
  overlay pipelines over original source, checks exact allowed markup, and
  invokes the actual resulting page/keyboard methods with the actual controller
  transport path. DOM geometry, page mutex, router, and keyboard event objects
  are controlled seams. Also binds actual title/group/edition/status components.
  It is not a compiled Svelte or browser run.
- test_bounded_reader_runtime_lifecycle.mjs: 11 cases. Executes actual runtime,
  controller, client, endpoint, DTO and navigation-intent modules. Svelte hooks,
  authority stores and local-library observation sources are explicit controlled
  seams. Covers navigation without IndexedDB, positive identity change while
  pending/ready, response identity drift, safe revision refresh, disposal and
  verification failure followed by changed/unchanged identity retry.

Counts are authored test registrations, including bounded test-registration
loops, not an execution report. HTTP/server ordering and browser behavior remain
separate qualification requirements.

## Proposed finite execution admission

No launch is implied by this plan. The resource owner must admit each stage from
current CPU, memory and disk state, with no overlap with another finite owner.

1. Run the shared transport file alone, then history, controller, runtime and
   composed consumer files, one Node process at a time. Use VM modules, built-in
   TypeScript stripping, test concurrency one and a 60-second owner timeout per
   file. A proposed 128 MiB V8 old-space ceiling is not an RSS bound: actual RSS
   headroom must be admitted independently and observed by the owner.
2. Set READER_FROZEN_SOURCE_ROOT to the frozen main's weebdex-platform directory.
   Current files are read from this sparse lane; unchanged module/overlay inputs
   are read from that existing frozen tree. Do not copy its frontend or install
   dependencies. Clear NODE_COMPILE_CACHE. The five source suites generate no
   application files; allow at most 64 KiB bounded receipt per file, 320 KiB total.
   Measured process peaks are not available before execution.
3. Stop on timeout, resource pressure, source mismatch or a failing assertion.
   Review the concrete result and separately admit a correction/rerun; do not
   claim any historical pin/evidence automatically covers this source.

The separate scripts/integrated-load/mounted-reader.mjs caller now composes the
bounded overlay, compiles the actual bounded runtime and mounts the real reader
provider. It waits for the admitted current witness, not aggregate length. It is
currently a public-mode mounted seam, not a native-mode or real-browser proof.
Its source closure requires the existing pinned Svelte 5.55.1, jsdom 27.0.1,
fake-indexeddb 6.2.4 and TypeScript 5.9.3 dependencies; it must not install or copy
them. It additionally needs current backend source, an admitted owned service and
scripts/integrated_load_scenarios.py / integrated_load_budget.py. Re-admit request
and byte budgets for capabilities plus /manga/{id}/reader rather than inheriting
old aggregate behavior. Its generated-output and RSS peaks remain unmeasured, so
its temporary-generation cap, service allocation and receipt cap require a
separate owner plan before launch. It is not bundled into the 109 source cases.

## Deliberate semantics and remaining gaps

Continue uses local completion keys only; it sends no history, read IDs, signer
identity or completion payload to a server/relay. An observed completed edition
marks only that observed label group. A live cursor is not a cross-request
snapshot: exhaustion cannot prove oldest-unread/all-read. There is no automatic
newest fallback. Candidate inspection and explicit opening are intentionally
separate UI; exact automatic Continue parity remains a documented gap. Account
history is unavailable here rather than silently replaced by device history.
The existing application's other library snapshot behavior is not made globally
bounded by this focused key-read implementation.

Edition choices currently display exact group IDs rather than fetching an entire
group dictionary. Friendly group-name hydration is not implemented in this
minimal consumer; any later lookup must be separately bounded and source-scoped.

The ten-second preparation deadline bounds selector/history work, not SvelteKit
router completion. After a validated navigation is dispatched, its owner remains
held until goto actually settles. The UI does not pretend Cancel can abort the
router, and a changed epoch cannot restore stale ready state. Real RTL/LTR,
focus, browser back/forward, image/page URL restoration and last-page destination
behavior still require mounted/browser qualification.

## Historical source-pin compatibility, explicitly pending

Do not rewrite historical hashes, normalize physical runtime source, or inherit
f7c598c qualification. Final bundle composition with other lanes must first be
frozen. Then add exact named successor reconstruction and caller controls for:

- scripts/prepare-frontend-integration.mjs: bounded overlay order, source inputs,
  owned component inventory and source-only evidence fields.
- scripts/compile-frontend-source.mjs and scripts/check-composed-frontend.mjs:
  new bounded stage in the actual native/public composition pipeline.
- scripts/vite.platform.config.mjs: bounded overlay plugin and $boundedReader
  alias, preserving existing public/upload stages.
- scripts/integrated-load/mounted-reader.mjs: current bounded runtime/source
  inventory and changed readiness condition, with newly admitted HTTP budgets.
- platform-integrations/local-library/library.ts and
  platform-integrations/public-api/native-forum/reader.ts: exact source successors
  for the observation/key-read seam and reviewed shared transport extraction.

Affected comparison families include reviewed-bundle-source-successors.{mjs,py}
(and their tests/metadata), test_full_application_restore.py,
test_public_upload_qa_adjacency.py, test_public_upload_build_recovery.mjs,
test_public_upload_overlay_recovery.mjs,
test_public_upload_provenance_recovery.mjs,
test_composed_semantic_projection.mjs, test_account_title_composition.mjs,
test_public_language_composition_guard.mjs and
 test_public_rating_build_seam.mjs. Their exact successor recipes must retain
old expected bytes and reject unknown new bytes. Existing aggregate-reader tests
remain historical controls; new bounded consumers need explicitly composed
successor coverage, not a global replacement of old fixture expectations.
Full strict checking, production build, live backend, native browser and
publication remain outside this source-only evidence.
