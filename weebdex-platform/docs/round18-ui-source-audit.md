# Round-eighteen independent UI source-completeness audit

Reviewed 2026-10-13, 09:56–10:09 UTC. This fresh reviewer did not implement
the UI17 repairs. The review independently composed the actual shared →
reader → local-library transforms, then followed mounted callers, resolver
substitutions, executed external components and the native response contracts.

## Finite verdict

**No additional substantive in-scope UI source-completeness gap was established
in this finite pass.** The two assigned UI17 findings are source-closed; the
owner's related created-draft acknowledgement recovery and real versioned
relation integration are also source-traced. A concurrent owner-found original
cover-extension contract mismatch was independently confirmed and its bounded
outward DTO repair independently inspected below. That repair is source-closed.

This is a finite clean **source** verdict on the identified candidate, not
perfect production behaviour, individual completion of all 475 control anchors,
or end-to-end acceptance. The reviewer executed no application, browser journey,
database operation, compiler, Cargo command, Vite build or test suite. All
runtime acceptance below is **unrun**. Only inspection copies, hashes and this
report were written; implementation and preserved attachment sources were not
edited. Native backend compilation/review, current bundle, fault fixture and
real API/database/browser acceptance remain separate gates.

## Independently frozen source

The frontend inspection copy was independently composed at
`2026-10-13T09:57:40.324Z`. It matches the owner candidate recorded at
`2026-10-13T09:55:36.487Z` in
`test-evidence/frontend/round17-owner-source-repairs/summary.json` and its source
manifest:

- 533 repair definitions, 542 exact occurrences, 105 original paths
- Every original input SHA and every sequential replacement-anchor count was
  enforced by the actual transformation modules
- All 105 transformed-source hashes independently match the owner manifest
- 541 copied/composed `.svelte`, `.ts` and `.js` source files
- Independent path-sorted composed-source inventory SHA:
  `e45357d67371bf83a451e7520358f25cf8a6778a29ebb6e1102c0d40d0c149a9`
- All 63 exact external integration inventory entries, including their sizes
  and file hashes, independently match the owner manifest; independently
  recomputed external inventory SHA:
  `ed86937126cb4a7895dd6ab7b68d416dadce96e409dcea67f30fc62284473c22`
- Shared overlay SHA:
  `6c2324801226db03d8671303a64d96548d789c1fe37768e3777f83fb418ac017`
- Reader overlay SHA:
  `70ec02db52c0867872f1545ff20667a9b7e0f7e747261de8b5a57f62285c9505`
- Local overlay SHA:
  `d394e81fdd7f6e02745ea905ec69f52d278bdac942ef9b2794110cc4ca6fc473`

All 676 original paths were independently rehashed against both recorded
source/copy hashes in `test-evidence/frontend/source-parity.json`; none is
missing or different. The matching owner receipt reports successful source
compilation of 93 Svelte components and 56 scripts. Those are owner evidence,
not checks executed by this reviewer. Build17/source516 is historical and does
not certify this source533 candidate.

`scripts/vite.platform.config.mjs:27–40` supplies the actual `$platform`,
announcement and `$localLibrary` resolution and shared → reader → local plugin
order. The local resolver replaces the actual API barrel's MangaFollow and
ChapterHistory imports. Logical `src/` lines below refer to the resulting
composed code. Raw attachment bodies and historical fragments were not
substituted for executed code.

## Assigned UI17 closure

### UI17-01: Incremental cover acknowledgements — source-closed

The title edit page mounts `MangaEditBoundary.svelte`, whose current full-data
guard mounts the retained composed editor. In that actual editor:

- `src/lib/components/draft/manga/MangaEdit.svelte:241–254` immediately commits
  every real acknowledged cover ID/version into the bound row and authoritative
  local cover baseline. It releases the accepted blob preview only after that
  acknowledgement. The former final-only temporary list is gone.
- `:274–290` removes a row only after a real 204, and acknowledges updates with
  the actual returned version. Later steps cannot discard these prior results.
- `:293–310` reads current title/cover state after a stopped batch, merges fresh
  IDs/versions while retaining pending Files and local cover-field edits, and
  does not associate unknown uploads by filename. Failed reads leave saving
  fenced with reload guidance; unresolved uploads remain fenced until deliberate
  inspection/Revert. `:160–167` prevents a falsely authoritative Reset when
  reconciliation failed.
- The current `MutationIntent` owns the actor, target and lifetime. No cover
  POST is automatically replayed after a lost acknowledgement.

Native cover success remains an actual 200 with the stored cover DTO:
`backend/src/domains/uploads/sessions.rs:1295–1332`. Native corrupt media can
still genuinely fail after an earlier cover committed; the repair preserves
that earlier acknowledged result rather than treating the batch as atomic.

**Acceptance, unrun:** first real upload/update/delete succeeds, a later
operation returns a genuine rejection or unknown transport result. Verify the
first exact identity/version and saved row survive; pending edits remain
recoverable; subsequent explicit edit/delete targets the actual stored ID.
Include main-cover selection, Reset, authoritative reload, target/account
change and an unknown upload whose bytes cannot be identified by filename.

### UI17-02: Private draft-cover reads — source-closed

`src/routes/(app)/title/[id]/edit/+page.svelte:26–50` owns a fresh current title
and cover read; `:17–20,56–59,64–91` exposes the resulting full DTO/head/editor
only under its current authority. A prior loader DTO cannot remain rendered
after an actor/permission switch. `editor-cover-read.ts:21–39,41–68` owns the
actor, target, navigation and disposal, attaches browser credentials plus the
expected-account header, selects `?draft=true` for a non-published full title,
and accepts only a real 200 array. A failed read is not an empty editable
baseline. Minimal publication receipts issue no cover request.

The publication boundary uses the same owned public/private-title and cover
sequence at `MangaEditBoundary.svelte:41–59`, with actual unavailable/error
states. Native `backend/src/domains/catalog/read.rs:579–608` supports that exact
private scope and retains owner/staff permission and recovery/API-client
restrictions. Public `Cover.List` and the public Covers tab remain public.

**Acceptance, unrun:** Create → Edit and direct private draft edit show current
saved covers; publication refresh does likewise. Genuine 200 `[]` is allowed.
401/403/404, malformed data and network failure must settle without opening an
invented empty editor. Repeat with owner/staff, recovery capability, route and
account transitions and ordinary same-account data refreshes.

## Owner's related created-draft recovery — source-traced

The actual native title create is 201, cover upload 200 and relation create
201. Composed `MangaCreate.svelte:173–181` retains the real acknowledged title
ID/version and frozen original metadata before subsequent steps. `:184–221`
retains each cover acknowledgement immediately and reconciles stopped batches;
`:224–254` retains each real relation tuple and owning-title version. A lost
title acknowledgement without an ID is unknown and cannot cause a second
automatic create. Known acknowledgements are not replayed.

The approved mounted Error branch at `:348–359` supplies the concrete Retry
remaining draft steps and Edit saved draft controls after a real title ID.
Retry requires the original actor and resolved known rejected/unsent work;
unknown writes/read uncertainty fence it. Changed title metadata requires the
explicit versioned edit route. Final fresh-read failure keeps its existing
saved-draft navigation instead of falsely claiming completion.

**Acceptance, unrun:** one 201 title, one accepted cover, later real failure;
explicit retry sends only remaining steps. Include later relation failure,
lost title/cover/relation acknowledgements, current actor changes and a final
fresh-read failure. Confirm one title identity, no duplicate accepted cover or
relation, truthful status and reachable current saved-draft editing.

## Actual versioned relation contract — source-traced

`backend/src/domains/catalog/mod.rs:37–42` actually mounts PUT alongside
POST/DELETE. The current native implementation is
`backend/src/domains/catalog/mutations.rs` SHA
`53e764a999f7fe00b85c122145f1a727588b44be92a90ff09630eecf04ca53d9`.

- `:746–777` keeps duplicate POST conflict409 and actual create201
- `:782–820,823–877` requires a real owner-title version for PUT; same-type
  PUT is a no-op and changed type increments the owner once
- `:879–904` requires the related title to be currently published/visible and
  returns the real projection plus `manga_version` and instance-local scope
- `:906–953` keeps DELETE204 with the actual updated owner version header
- `:969–972` and `repository.rs:507–538` retain mutation validation/current
  principal, owner/staff/permission/lock checks, the owning title row lock and
  immutable pending original-publication fence

Executed `MangaEdit.svelte:314–382` chooses existing-relation PUT versus new
POST, sends the owner precondition, consumes `manga_version` rather than the
related title's `Relation.version`, and durably applies each successful tuple
or removal before the next operation. The composed HTTP body at
`src/lib/HTTP.ts:104–109` actually reads the DELETE version header. A stopped
batch reads current saved relationships and merges local pending edits;
ambiguous writes are not blindly replayed. These are instance annotations and
do not alter the signed source event.

**Acceptance, unrun:** change an existing relation type, add/remove several
relations, same-type PUT, duplicate POST, missing related pair, stale owner
version, locked/unauthorized/ineligible target and immutable pending source
command. Lose a later acknowledgement and verify current read-only
reconciliation, exact saved tuples and versions, with no write rebound to a
new actor or unrelated title.

## Concurrent owner finding: original Cover.ext contract — source-closed

The owner reported this while the pass was running; it is not claimed as a
new independent discovery. The mismatch was independently confirmed:
the former final projection at
`backend/migrations/0082_catalog_projection_visibility.sql:11` returned bare
`c.ext`, while executed `MangaEdit.svelte:60`, `editor-cover-read.ts:72` and
`MangaCoversSection.svelte:30` concatenate the cover ID and DTO extension.
For a stored PNG this produced `{id}png`; native
`uploads/media.rs:253–273` requires `{id}.png`. Thumbnails could still work,
masking a broken original preview/lightbox.

Independently inspected repair:

- `backend/migrations/0085_cover_extension_contract.sql:4–6` returns
  `'.'||c.ext` only in the outward Cover DTO and preserves the exact final82
  canonical-source/parent eligibility predicates
- `backend/src/domains/mod.rs:183–184` registers that later migration
- Repair SHA:
  `3668bf28a8032cd4b2d4f485cca06625502aaf173fc3d6d7546f512765cae692`
- Storage and native/protocol media continue using their existing bare
  extensions; no raw original frontend file was changed

This closes the demonstrated URL contract at source. **Unrun acceptance:**
apply the final migrations on an isolated database; upload/list/read/update a
native cover, then open its original editor preview and public gallery
lightbox. Verify the exact dotted original URL and existing 256/512 URLs.
Repeat private owner/staff and imported source covers, missing/revoked source
and conditional media reads. Database application and real bytes are not
established by inspection.

## Broader finite review coverage

The pass also revisited these mounted paths and their effective imports:

- **Discovery and navigation:** effective preference/default/all-ratings-off
  queries, advanced search filter/reset/sort/page paths, native artist/tag
  parameters, public terminal pagination and uncapped library branch,
  current-query home carousels, title chapter-filter applied baselines,
  public Covers empty/error/Retry and filtered gallery URLs, and bounded
  chapter-feed continuation/resume/exhaustion. The root's retained leading-`?`
  URLSearchParams serialization was checked; raw missing-`?` reports would be
  false positives. Fresh deep-link startup ordering still needs browser proof.
- **Reader:** mounted provider/aggregate, descending natural chapter
  navigation, direct page/spread selectors, offset and selected second spread,
  actual wrapper tap geometry, admitted preload neighbours, parallel loading,
  bounded XHR/decode failure and explicit Retry, object URL disposal,
  quality capability, scroll geometry/layout changes, keyboard/control/modal
  ownership, account-history acknowledgement and device current-page/exit
  flush destination ownership.
- **Upload and edit:** images/archive/folder admission and natural ordering,
  bounded central ZIP/expanded bytes, staging resume/version/authority,
  metadata/order autosave, begin/cancel/commit, per-page exact-byte unknown
  recovery, failed/unsent placeholders, reset/navigation ownership and native
  immutable page-replacement/publication boundaries. Hypothetical AVIF or
  invented minimal edit DTOs inconsistent with native intake were not findings.
- **Local-first/offline:** durable saved-device editing and backup review,
  independent current account mode, explicit account import, custom device
  list membership, catalogue namespace/service scope and migration fences,
  canonical family inspection ownership, canonical streaming backup import,
  source identity adoption, exact reviewed public snapshot/recovery and the
  signed public-action outbox. Current native account access does not become
  protocol signer authority; device progress is not account history.
- **Authentication/account:** current login/register/verification form and
  navigation ownership, logout, restricted recovery-only capability and
  truthful unavailable email flow, dirty preference draft/current baseline,
  acknowledged preference version, API-client creation/rotation/delete/copy,
  current account user/group follow and edit routes.
- **Community/moderation/announcements:** mounted discussion target/actor
  keys, comment create/edit/remove/vote/quote/counter paths, report
  subject/reason/close/reopen lifetime and truthful create/status/delete ACK,
  noncached private feed identity, reconstructed public announcement
  list/detail/home transport and staff manager permissions, versions,
  edit/save/delete/page-switch ownership.

The review did not file speculative new features, stylistic changes,
unmounted widgets/fragments, unsupported media formats, imaginary native
response shapes, or generic lifecycle hypotheses without a concrete mounted
flow and adverse outcome. Its stopping condition is completion of this
bounded independent source pass with no additional established in-scope gap
and the concurrent owner cover contract source-closed. Runtime/publication
readiness remains subject to the separate current-source acceptance gates.
