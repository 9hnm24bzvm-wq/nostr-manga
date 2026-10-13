# Round-nine independent UI source-completeness audit

Reviewed 2026-10-13, beginning 06:33 UTC. This reviewer did not implement the
preceding repairs. The shared → reader → local-library transforms were applied
independently to an inspection copy, enforcing their original source hashes and
exact occurrence anchors. Mounted resolver configuration, composed callers,
external integration modules and the relevant backend handlers were read directly.

## Finite verdict

**Source signoff withheld: four substantive, bounded findings were established.**
The four round-eight repairs are present in the inspected candidate. UI9-02 is a
new interaction of the stable discussion lifecycle repair with a one-shot personal
vote read. The previous reader-remount feedback loop is not repeated as if that
repair were absent.

No browser journey, application runtime, database operation, Cargo command, Vite
build or test suite was executed. Source transformation, lexical control scans and
hashing are not application tests. Every runtime acceptance below is **unrun**.
No implementation source was edited by this reviewer; this report is the sole
repository deliverable. Findings were sent to the parent and frontend owner as
they were established. Later repair candidates require a fresh independent pass.

## Independently frozen source

Snapshot taken at `2026-10-13T06:35:05.393Z`:

- 387 repair definitions, 396 exact occurrences, 82 original paths
- 541 copied/transformed `.svelte`/`.ts`/`.js` source files
- SHA of JSON-serialized composed path/SHA records sorted with `localeCompare`:
  `1779efe1f7228d8be90a9c844ecb01a58f6c88c2f28967133a8874e29934b34f`
- Shared overlay:
  `dba2778267d7763e54dd8f51638684e942e7a41102237158edcfb1d87ad33144`
- Reader overlay:
  `70ec02db52c0867872f1545ff20667a9b7e0f7e747261de8b5a57f62285c9505`
- Local overlay:
  `acc47af42d7638f6cc77d2529c71b553f7d1ce73c5b8ebe11b3461fb275a043f`
- Independently calculated external integration inventory SHA:
  `5c4fd48fb4fd7914bd7629f643bab4abd0cd4f73a1e4d5ee9c2d9f5f2b4d92cf`

All 676 original attachment paths were independently rehashed against the
`source_sha256` entries in `test-evidence/frontend/source-parity.json`: none was
missing or different. Logical frontend paths/lines below refer to the composed
inspection copy, rather than the raw preserved attachment. External paths name
the actual imported integration modules. Historical explanatory fragments are
not substituted for mounted transformed source.

The existing compiler receipt at `2026-10-13T06:30:57.551Z` reports a passed
73-component/40-script source compilation with these same integration hashes.
That is separate owner evidence, not a compiler or runtime run by this reviewer.

Relevant backend source hashes read during this review:

- `backend/src/domains/auth/clients.rs`:
  `61952652ea07769fb54e5f1cdecbbaca298dc0a5f711c8234a5d5055e22ad57e`
- `backend/src/domains/community/mod.rs`:
  `9c64d255f012a4f6a672e8aa653ea43237576e337ea5631a5ded0057cba9cfb2`

## Findings

### UI9-01 — P1: A disposed reader can admit its automatic history write for a replacement account

- Composed `src/lib/components/reader/reader-history.svelte:11-13` waits for
  `openLibrary()` and then reads the current library mode before calling
  `API.ChapterHistory.UpdateBatch`. It captures neither the mounted reader's
  actor/authority nor a live target/disposal token before that wait.
- Its `onDestroy` at line 10 only clears the device-progress timer and flushes
  queued device progress. It does not fence the pending account-history callback.
- Composed `reader-provider.svelte:49-59` stores the original Chapter in its
  ReaderState. Disposal clears the aggregate and revokes blobs, but does not clear
  that Chapter. The old callback therefore still has a usable old chapter target.
- Actual `platform-integrations/local-library/ChapterHistory.ts:32-34` also waits
  for opening before capturing the mutation context. Its account helper correctly
  binds the actor it receives, but cannot reconstruct the old reader's actor.
- Scenario: account A's reader mounts while initial durable-library opening is
  deferred. Navigate away, sign out/sign in as B, then release opening with the
  persisted account-library mode. The destroyed A-reader callback sees current
  account mode and admits a history POST for its old chapter using B's current
  account context. B's history records an A-reader visit. The expected-account
  check correctly receives B and consequently does not reject this already
  reinterpreted intent. Same-account navigation/disposal also leaves this old
  unsent callback able to start after its owning reader has gone away.
- Closure: carry a live reader/target and admitted account/mode/authority context
  across library opening and all later waits. An obsolete, unsent automatic
  history intent must not be newly admitted in a replacement reader/account.
  Keep ordinary current-reader account history usable and retain truthful
  uncertainty for already-sent writes; do not automatically replay mutations.
- Acceptance: defer initial opening across reader navigation, disposal, A→B,
  A→logout→A and library-mode changes. Release it and verify the old callback
  admits no mutation for the replacement context. Separately delay a sent
  history response and verify its original binding. A normal account-reader
  mount admits one visit and settles; device progress still saves its captured
  local visit without a server fallback.
- Runtime acceptance: **unrun**.

### UI9-02 — P2: Same-account history reconciliation permanently drops the initial thread-vote read

- Composed `src/lib/components/comments/comments-votes.svelte:26-32` loads the
  personal thread vote only in `onMount`, using a mutation-owner intent. A stale
  read is silently discarded; there is no authority-reactive replacement read.
- Actual `platform-integrations/frontend-behavior/thread-read.ts:5-10` captures
  the complete native authority revision and aborts/rejects on a Session write.
  `authenticated-mutation.ts:12,25-32` likewise invalidates the read's intent on
  every Session write.
- Account-reader History mounts automatically at `reader-history.svelte:11-13`.
  Its normal successful reconciliation writes Session at actual
  `platform-integrations/local-library/library.ts:206-212`.
- The repaired `comments-provider.svelte:34,53` correctly uses stable authority
  epoch. `mutation-authority.ts:12-16` advances write generation but keeps epoch
  unchanged for the same actor/roles/permissions/verification. Thus this own
  history reconciliation retains Reader and its mounted Votes component.
- Scenario: A previously upvoted this chapter's discussion. Open its reader with
  the normal panel containing `Comments.Votes` (`reader-panel.svelte:115-116`).
  Hold that initial personal vote read while the automatic history POST and
  matching account reconciliation finish. Session changes generation; the held
  vote read is discarded. No remount or replacement vote read follows, so
  `comments.voteState` remains undefined for the mounted reader. Clicking Up now
  invokes Vote rather than Unvote (`comments-votes.svelte:56`) and adds one to
  the displayed count (`44-47`). Backend `community/mod.rs:165-167` merely
  upserts the same existing vote and returns 204. The intended toggle does not
  remove the vote, and the local count is inflated until a fresh view.
- Closure: make the owned personal thread-vote read react to relevant authority
  revisions, discard obsolete reads and reload the current vote after a retained
  same-account reconciliation. Keep unknown/loading state distinct from a
  confirmed absent vote when admitting a toggle. Do not solve this by recreating
  the entire Reader or weakening mutation-generation guards.
- Acceptance: defer vote GET/auth checks around a successful account-history
  reconciliation for existing upvote, downvote and no-vote fixtures. The latest
  confirmed state is restored in the stable reader; one click performs the
  correct toggle with correct counts. Exercise multiple mounted vote controls,
  actual account/permission changes, query/navigation and disposal. No obsolete
  result attaches and no vote mutation is replayed.
- Runtime acceptance: **unrun**.

### UI9-03 — P2: The API-client creation form advertises names the server refuses

- Composed `src/routes/(app)/settings/client/ClientCreateDialog.svelte:64-65`
  permits 5–255 characters. Its tooltip at line 79 states that same range, and
  the Create button at line 87 admits every name in that range.
- Backend `backend/src/domains/auth/clients.rs:44-48,80-84` trims the name and
  permits only 1–64 printable Unicode scalar characters with an additional byte
  bound. A longer name is rejected with HTTP 400.
- Scenario: enter an ordinary 65-character ASCII client name. The existing input
  is valid and Create is enabled, but the actual endpoint necessarily rejects
  it. The dialog's result branches at lines 41-45 turn that validation failure
  into the generic Unknown message, leaving no actionable correction to the
  disclosed 5–255 requirement. This is a current supplied creation workflow,
  not a request for a scope editor or another client feature.
- Closure: align the bounded backend and supplied frontend name contract,
  including trimming and character-count semantics. Preserve the mandated
  original UI according to the project's approved behavior-repair rules, and
  provide actionable validation feedback for a genuine rejected input.
- Acceptance: valid boundary names under the agreed disclosed contract create
  exactly once; below/above-boundary, whitespace-only and Unicode inputs receive
  coherent validation. A 65-character ASCII name cannot pass the disclosed
  input/button contract and then fail solely because of an undisclosed 64 limit.
  Network uncertainty, stale actor and recent-auth refusal remain truthful and
  never cause an automatic credential-creation replay.
- Runtime acceptance: **unrun**.

### UI9-04 — P2: Search dialogs discard the newest query while displaying an old query's results

- Composed `src/lib/components/SearchDialog.svelte:24-77` writes manga, author
  and group results without comparing a captured query or a live dialog token.
  Its debounce at lines 80-91 uses `mutex.exec`.
- Composed `src/lib/Mutex.ts:46-52` returns immediately when already locked; it
  does not queue that callback. The search input remains editable while the
  previous operation holds the mutex (`SearchDialog.svelte:107-114`).
- Scenario: type query A and hold its requests. Type query B, then allow B's
  500 ms debounce to fire while A is still loading. The mutex discards B's
  callback. Release A: its results are displayed below the now-B input, and
  no future B request exists. The state remains wrong until another input or
  reopen action. Clearing the input while A is pending has the same stale-result
  seam; the current query and the installed feed do not agree.
- Composed `src/lib/components/EntitySearchDialog.svelte:26-43` has the same
  dropped-newest pattern for the actual author/group/entity selection dialogs.
  Its editable input is at lines 54-61. A user can therefore select a result from
  the earlier query while the selector visibly contains the newer query.
- Closure: capture query/generation and dialog lifetime for each read, discard
  obsolete results, and ensure the newest pending query is actually executed
  after a busy operation or safely replaces it. Clear empty-query results and
  stop pending debounce/result attachment after dismissal. This is safe read
  ownership; it does not authorize replaying any selected entity mutation.
- Acceptance: A requests delayed past B's debounce eventually show only B's
  correctly labeled results; A→empty shows none. Repeat through multiple fast
  edits, mixed manga/author/group response order, failures, Enter, Close/reopen
  and selecting an entity. Old timers/results cannot attach to a new dialog or
  cause a newer query to be dropped indefinitely.
- Runtime acceptance: **unrun**.

## Independent reread of round-eight closures

1. **UI8-01 is source-closed for the identified Group/User follow variant.** Both
   retained controls now synchronously capture actor, target and desired follow
   state before their first await, carry the owned intent through transport and
   only apply a matched acknowledgement. Their private follow reads also carry
   owner/disposal context. Native transport still freezes payload/actor before
   its cancellable limiter wait and preserves unknown sent outcomes.
2. **UI8-02's original feedback loop is source-closed.** Stable authority epoch
   separates descendant lifecycle from write-generation fencing. An ordinary
   same-authority history reconciliation no longer changes the provider key.
   UI9-02 establishes a different one-shot-read interaction, rather than claiming
   that reader remounts still occur on those writes.
3. **UI8-03 is source-closed.** The actual retained native Share button has a
   handler and accessible name. It captures the current canonical thread URL,
   invokes native share or awaits clipboard copy, distinguishes cancellation and
   failure, and fences late feedback on thread/disposal changes.
4. **UI8-04 is source-closed.** Duplicate removal accepts equality only for
   acknowledged full server content hashes. Hashless local/unsent/rejected pages
   get unique keys and are retained. Unknown outcomes remain fenced, and remote
   rows/previews are removed only after an acknowledged DELETE. No filename-only
   equality or automatic re-upload is used.

These are finite source conclusions, not passing repeated/interrupted browser
journeys or certification of later owner changes.

## Coverage and remaining gates

The pass inspected actual source/resolver composition, native request admission,
the preceding closures, discussion target/query/authority lifetimes, preferences
and blocking, public discovery/search/filter/pagination callers, selected-library
feeds and read flags, reader composition/aggregate/pages/keyboard/queue/history,
upload image/archive/folder admission, staging/restore/autosave/replacement/remove/
duplicate controls, title/editorial controls and private boundaries, report
submission/moderation, account/client/authentication and restricted recovery,
saved-device lists/backup/import/editing, signed public identity/review/recipient
binding, publication/recovery and exact outbox controls. A finite lexical scan
of non-generic frontend components found 11 native buttons and 204 Button
controls with an explicit handler, form type or supplied trigger props; this
does not prove every semantic action or keyboard journey works. No further
substantiated in-scope defect was established in this finite pass.

The retained URLSearchParams prefix override was checked before evaluating query
concatenations. Raw-original behavior changed by mounted overlays was not counted
as an active defect. Unused API wrappers do not by themselves justify a new UI.
No speculative feature, styling, label-only or production-perfection request is
part of these four findings.

Final executable bundle, actual browser/E2E/system journeys, offline boot,
persistence, revocation and production gates remain separate. None was executed
or certified by this review. Source compilation and unchanged original bytes
cannot substitute for those gates or establish production readiness.
