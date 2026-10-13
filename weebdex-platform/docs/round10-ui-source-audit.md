# Round-ten independent UI source-completeness audit

Reviewed 2026-10-13, beginning 06:59 UTC. This fresh reviewer did not implement
the preceding repairs. Shared → reader → local-library transforms were applied
independently to an inspection copy, enforcing the immutable-source hashes and
exact occurrence anchors. Actual composed callers, resolver configuration,
imported integration modules and relevant native backend handlers were read.

## Finite verdict

**Source signoff withheld: four substantive, bounded findings were established.**
The four round-nine repair families are present and source-close their reported
scenarios. UI10-04 is an additional success-status contract failure in the same
existing client-creation workflow, rather than a claim that name validation was
not repaired.

No application runtime, browser journey, database operation, Cargo command,
Vite build or test suite was executed. Source transformation and hashing are
inspection operations, not application tests. All acceptance scenarios below
are **unrun**. No implementation source was edited by this reviewer; this report
is the sole repository deliverable. Findings were reported to the parent as
they were established. Later repair candidates require a fresh independent pass.

## Independently frozen source

Frontend snapshot: `2026-10-13T07:00:21.579Z`.

- 420 repair definitions, 429 exact occurrences, 91 original paths
- 541 copied/transformed `.svelte`/`.ts`/`.js` source files
- SHA of JSON-serialized composed path/SHA records sorted with `localeCompare`:
  `dee38169ac23d72d98f45782717bf08cce64e5e3357573175d7d94920923d5cf`
- Shared overlay:
  `688ce8392dfbf4397e59eafc3d6bd90b908db30d680ff36ce6834c2102e5ab58`
- Reader overlay:
  `70ec02db52c0867872f1545ff20667a9b7e0f7e747261de8b5a57f62285c9505`
- Local overlay:
  `85b7aad688a47f459b4bd993d6956bc2d68f881fb52a0c12bbac9426d39aa344`
- Independently calculated external integration inventory SHA:
  `7c91a5720e59100920e02e998966dec05eda6f3469130b3ca33f086d10f26eda`

All 676 original attachment paths were independently rehashed against
`test-evidence/frontend/source-parity.json`: none was missing or different.
Logical `src/` paths and lines below refer to the composed inspection copy, not
the raw preserved attachment. External paths identify actual source files;
historical explanatory fragments are not substituted for mounted source.

The existing owner compiler receipt at `2026-10-13T06:57:06.263Z` reports passed
79-component/46-script source compilation with these same integration hashes.
Its matching freeze receipt is
`test-evidence/frontend/round9-owner-source-repairs/summary.json`. This is owner
evidence, not a compiler/runtime run by this reviewer. Build9 is stale for this
420-definition candidate; this review certifies no final executable bundle.

Relevant native source was separately read and copied for inspection:

- `backend/src/domains/auth/clients.rs` SHA:
  `3537f7634a74d83d4a9416ebdb0dd9eb76d6d5d0fe14e730772b5a968bda172d`
- `backend/src/router.rs` SHA:
  `544a69ff1466f7a6231b062c6631a17f35b71a48ef5409aa8bea45489634087b`

## Findings

### UI10-01 — P2: Abandoned auth/settings callbacks can overwrite newer navigation

- Composed `src/routes/(app)/auth/login/+page.svelte:141-151` checks pathname
  after the login response, then awaits `Session.initialize(true)` at line 148.
  There is no new lifetime/navigation check before the redirect at lines 149-151.
  Registration has the same pattern at
  `src/routes/(app)/auth/register/+page.svelte:150-159`.
- Neither component has a disposal/flow-generation fence. Their initial flow
  reads also compare only the pathname, then globally replace navigation state
  (`login:63-77`, `register:71-85`). A disposed instance therefore passes the
  check when a new instance has reopened the same route.
- Composed `src/routes/(app)/settings/+layout.svelte:40-47` likewise awaits
  `Session.isValid()`, then reads the later global pathname and redirects if its
  last segment is not a settings tab. It carries no layout lifetime/route fence.
- Scenario 1: submit a valid login or registration, let its success response
  arrive, hold the subsequent `/auth/check`, and navigate to `/search`. Releasing
  that read lets the disposed auth form redirect over the newer navigation.
- Scenario 2: hold initial session readiness while opening preferences, navigate
  away to `/search`, then release readiness. The old settings callback reads
  `search` as its tab and sends the user back to preferences.
- Scenario 3: hold a login/registration flow GET/create response, leave and
  reopen that auth route, then release the older response. The old instance can
  replace the new route's flow query with its own flow ID. Its local form state
  belongs to the disposed instance, so the current URL and form can disagree.
- Closure: own asynchronous auth/settings continuations by mounted lifetime,
  navigation/flow generation and the captured target. Recheck after every wait
  before navigation, flow attachment or global history writes. Preserve ordinary
  current-flow authentication and authoritative Session reconciliation. A sent
  authentication operation may already have completed; do not replay it or
  claim that discarding a late UI redirect reverses it.
- Acceptance: defer flow creation/read, login/registration acknowledgement and
  the subsequent Session refresh across navigation, Back/Forward, same-route
  reopen and a changed flow query. Old work cannot redirect or replace the new
  flow state. Repeat the settings readiness case. A current valid flow still
  authenticates once and continues to its approved return target; errors and
  recovery restrictions remain truthful.
- Runtime acceptance: **unrun**.

### UI10-02 — P2: Failed previous-chapter navigation makes an unrelated chapter open completed

- Composed `src/lib/components/reader/context.svelte.ts:15` keeps
  `isNavigateLastPage` as module-global state. Any `ReaderState` constructor
  consumes it at lines 60-61 without comparing the destination chapter.
- `gotoPreviousPage` sets this global flag before awaiting automatic previous-
  chapter navigation (`context.svelte.ts:161-163`). It has no target binding or
  cleanup for a failed, cancelled or superseded navigation.
- Actual composed chapter route loaders
  `src/routes/(app)/chapter/[id]/+page.ts:6-9` and
  `src/routes/(app)/chapter/[id]/[page]/+page.ts:6-9` throw on a 403/404 response.
  Their route components instantiate `Reader.Provider` only for a real chapter
  (`+page@.svelte:12-20`), so an unavailable target does not consume the flag.
- The next valid Reader consumes it, and
  `src/lib/components/reader/reader-provider.svelte:68-71` selects that reader's
  last page and writes its last-page URL. The automatic device-progress effect
  regards that state as complete at `reader-history.svelte:45-46`.
- Scenario: from chapter A's first page, use the supplied previous-page control
  with automatic advance enabled. The aggregate identifies chapter P, but its
  detail has become 403/404. After the unavailable view, open unrelated valid
  chapter B at `/chapter/B/1`. B inherits A→P's leftover end-state, opens at its
  last page, and can save completed progress without the user reading B.
- Closure: bind the previous-navigation end-state intent to its exact target
  and owning navigation/lifetime. Failed, cancelled and superseded attempts
  cannot be consumed by an unrelated reader. Preserve successful automatic
  previous-chapter last-page navigation and one-based URLs.
- Acceptance: use a real unavailable previous target, then open an unrelated
  chapter's explicit page-one link and verify page one and incomplete progress.
  Repeat with delayed/cancelled/superseded navigation and Back/Forward. A normal
  successful previous-chapter attempt still opens the intended chapter's last
  page, and later unrelated direct navigation never inherits that intent.
- Runtime acceptance: **unrun**.

### UI10-03 — P2: Late tag initialization silently discards edited preferences

- Composed `src/routes/(app)/settings/preferences/+page.svelte:30` initializes
  `draft` from the authority effect. The existing form renders once `draft`
  exists (`:131-133`), and its ordinary language/rating controls are editable
  while tag loading remains pending.
- A separate `onMount` awaits `Promise.all([Session.isValid(), populateTagList()])`
  at lines 34-36, then unconditionally calls `onReset()` at line 40, including
  after a caught failure. `onReset` replaces the entire draft at lines 43-46.
- Actual composed `src/lib/stores/global.ts:8-26` performs awaited tag catalogue
  requests on a fresh selector cache. Completion is independent of edits in the
  already mounted preference form.
- Scenario: open preferences directly with a ready session and an empty tag
  cache. Delay the tag response; change an existing language or content-rating
  preference without saving. Release the tag response, or allow it to fail.
  The late initializer resets every unsaved field to stored preferences without
  a user reset or an account change. The edited value and dirty state disappear.
- Closure: initialize/hydrate a draft under its actual account/lifetime/dirty
  context. A non-authority tag-read completion cannot overwrite an active edited
  draft. Explicit Reset, genuine account changes and acknowledged preference
  saves should retain their intended hydration behavior.
- Acceptance: on a first direct preferences visit, defer and fail tag loading
  while editing language/rating fields and verify the edits survive. Save then
  persists the reviewed values once; Reset restores actual stored values. Repeat
  across account change, own acknowledged save, navigation and selector reopen.
  Old initialization cannot restore another account's draft or erase newer edits.
- Runtime acceptance: **unrun**.

### UI10-04 — P2: Successful client creation is reported as failure and its one-time secret is discarded

- Composed `src/routes/(app)/settings/client/ClientCreateDialog.svelte:38-42`
  installs the created client/key and closes the dialog only for HTTP 201.
  HTTP 200 reaches its generic failure branch at line 49.
- Native `backend/src/domains/auth/clients.rs:37-42` builds its reply with
  `Json(body).into_response()` and adds only `Cache-Control`. This is HTTP 200.
  The successful creation handler commits the inserted credential at line 118
  and returns that unchanged reply at line 119, including the one-time `key`.
  Router source contains no response-status conversion for this endpoint.
- Scenario: submit an ordinary valid client name with a recent authorized browser
  session. The backend creates the credential and returns its key with HTTP 200.
  The actual retained dialog reports Unknown, remains open and never adds the
  client or exposes its key. Clicking Create again makes another credential,
  rather than recovering the first one, until throttling/client limits intervene.
- This is independent of UI9-03: the repaired trimmed Unicode name contract is
  present, but its valid happy path still fails at the success-status seam.
- Closure: make the creation success status agree between the native endpoint
  and the retained frontend. Show the one-time key only for the acknowledged
  created credential and keep unsuccessful/ambiguous outcomes distinct. Do not
  introduce an automatic creation replay or weaken credential authorization.
- Acceptance: exercise the real retained Create control with a valid name and
  verify one durable new client, matching success status, dialog closure and the
  corresponding one-time key in the newly installed row. Refresh omits the key;
  explicit authorized regeneration remains usable. Invalid names, recent-auth
  refusal, rate/client limits and lost responses create no false success or
  automatic retry. Do not retain credential material in test evidence.
- Runtime acceptance: **unrun**.

## Independent reread of round-nine closures

1. **UI9-01 is source-closed for the identified delayed automatic-history intent.**
   `action-owner.ts` captures library admission before storage opening and checks
   target, disposal, selected mode and read authority after the wait. The mounted
   account-history effect requires readiness and a current actor, admits work
   through that owner and avoids repeated POSTs on same-authority data refresh.
   Device exit saves only its previously admitted immutable page action. UI10-02
   concerns the separate navigation end-state producer.
2. **UI9-02 is source-closed.** Personal thread reads use stable actor/authority
   epoch. The mounted vote control owns/cancels its reactive reads, and every
   toggle freshly confirms personal vote state before mutation. Missing/stale
   state is never interpreted as a confirmed absent vote. Acknowledged changes
   reread current counts instead of applying the old optimistic increment.
3. **UI9-03 is source-closed for validation.** Input/button disclosure and the
   shared helper agree on 1–64 trimmed printable Unicode scalars, the server's
   byte bound and controls. API creation/rename both validate before admission;
   genuine 400 feedback is actionable. UI10-04 is the remaining success contract.
4. **UI9-04 is source-closed for dropped/late query ownership.** Both mounted
   search dialogs use `createLatestSearch`, carrying the newest debounce,
   generation, abort signal, completion and busy state. Empty/current-query and
   lifetime checks reject old attachment; relevant inline author/group/user
   callbacks forward cancellation. Full input/Enter/dismissal journeys are unrun.

These are finite source conclusions, not passing repeated/interrupted browser
journeys or certification of later owner changes.

## Coverage and remaining gates

The pass inspected actual transform/resolver composition, transport admission and
no-replay behavior, the preceding closures, public search/filter/pagination and
selected-library feeds, current read/follow flags, reader aggregate/context/pages/
queue/keyboard/history/quality/retry, upload admission/archive/folder/splitting/
batch/replacement/remove/duplicates/staging/restore/autosave/cancel, account
preferences/blocking/client/auth/restricted recovery, report creation/review/
moderation/private feeds, reconstructed announcement reads and management,
device records/custom lists/backup/import/scope verification, and signed public
identity/reviews/recipient binding/recovery/publication/exact outbox controls.

The mounted URLSearchParams prefix behavior was considered before judging
retained query concatenations. Actual composed native HTTP was inspected;
the historical standalone `frontend-behavior/HTTP.ts` was not treated as the
mounted implementation. Raw-original defects already repaired by mounted
transforms, unused wrappers, speculative features and visual/style changes are
not findings. No further substantiated in-scope defect was established in this
finite pass.

Final executable bundle, actual browser/API/database/system journeys, offline
boot, persistence, revocation and production gates remain separate and unrun by
this reviewer. Source compilation and preserved original bytes cannot substitute
for those gates or establish production readiness.
