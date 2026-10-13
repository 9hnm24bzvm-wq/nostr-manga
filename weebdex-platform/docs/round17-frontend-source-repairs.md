# UI17 cover and created-draft acknowledgement recovery

Finite four-family source-only owner repair; real browser/API/database execution remains unrun. Build18/source533 passed; Build17/source516 remains historical. Exact before/after SHA/count anchors are in frontend-overlays.mjs; frozen composed sources and external hashes are in test-evidence/frontend/round17-owner-source-repairs.

## Private editor reads

The initial editor re-reads current title authority before attaching any head/body/child DTO. Its script facade exposes only the fresh DTO for the exact current account and target; cached minimal publisher-only receipts cannot attach under another account. The retained head/body markup itself stays byte-identical. Both the title edit route and pending-publication boundary use editor-cover-read.ts. Non-published full draft DTOs request /api/manga/{id}/covers?draft=true with browser credentials and X-WeebDex-Expected-Account. Published editor titles use the ordinary public cover path with a frozen actor header. Public Cover.List and the public Covers tab are unchanged. Minimal signed receipts do not issue cover requests. Non-200, lost or malformed reads are errors, never inferred empty lists. Target, navigation, disposal and actual account/permission changes invalidate reads; same-account ordinary data refresh does not discard a valid read. Boundary public-title/private-receipt/cover reads share one owned admission ticket.

## Partial editor batches

Every acknowledged upload/update commits its real returned cover ID/version immediately into the baseline and edited row, before the next request. Only an acknowledged delete removes its baseline/row. Accepted blob previews are released after acknowledgement; failed pending previews remain usable. No final-only temporary array can discard earlier successes. A failed batch performs owner-bound reads of current title/main-cover and covers, preserving pending Files and unrelated edited metadata against the fresh baseline. Known rejections can be deliberately retried. Ambiguous upload Files are not associated by filename (the real DTO has no file hash) and cannot be replayed. After successful reconciliation the existing Revert control can discard unknown pending Files while keeping actual saved covers; if current reads fail, save/revert are fenced with explicit reload guidance.

## Created-title recovery (owner-found same-family gap)

Actual native manga/relation create acknowledgements are201; cover upload acknowledgement is200. The real created title ID/version and frozen metadata are retained immediately before any cover/relation request. Retry never repeats that title create, an acknowledged cover File or an acknowledged relation. Each cover immediately receives its real returned identity/version; each relation records its actual acknowledged tuple. Fresh owner reads reconcile failures and final title version. Changed metadata requires an explicit versioned edit through the saved draft, rather than silently reusing a create request. A lost create response without a real ID is an unknown outcome: My drafts provides review, and no second title request is sent automatically.

One exact parent-approved markup addition in the existing error branch supplies Retry remaining draft steps and Edit saved draft only when a real title ID has been acknowledged. Retry is enabled only for the original actor and known rejected/unsent remaining steps, with no unresolved write/read uncertainty. Ambiguous cover uploads remain fenced; positive fresh matching relation state may reconcile a lost relation acknowledgement, but only a later deliberate retry continues remaining steps. The original classes, ordinary layout and unrelated controls remain unchanged. This necessary rendered extension is not globally identical executed HTML.

## Required later real acceptance

- Private owner/staff draft with existing covers: initial load and publication refresh return real covers;403/404 never become empty success.
- Editor batch: first genuine cover accepted, second genuinely rejected; first real ID/version survives and retry does not upload it again.
- Create batch: one actual201 title, first actual200 cover, later rejection; real title/cover survive and the reachable retry sends only remaining known steps. Later relation failures retain acknowledged tuples.
- Lost title/cover acknowledgements: no automatic or disguised POST replay, no filename-based mapping, truthful unknown outcome and current read-only reconciliation.
- Pending Files/draft edits survive a failed read; acknowledged deletions/updates do not regress. Existing Revert/reload guidance stays reachable and honest.
- Account A to logout/B, target change or component disposal cannot attach/reinterpret an old write/read; an already sent operation stays attributed to its original actor.

## Actual existing relation update

The owner-found relation edit mismatch is repaired against catalogue's exact confirmed contract. An existing changed type uses PUT /api/manga/{id}/relations with related_id,type and the actual owning manga.version; new relations use POST201. PUT200 returns the actual saved Relation plus manga_version. DELETE204 supplies x-weebdex-manga-version. Every acknowledged row/removal commits immediately, and every subsequent request consumes the actual parent version; related-title Relation.version and synthetic increments are never used. Known rejected rows remain pending; lost outcomes are read-only reconciled and unresolved intents are fenced. Fresh metadata is adopted only where no local field edit was pending. Current immutable publication flags notify the existing outer receipt boundary instead of continuing an old editable form.

Required real acceptance also includes two type edits/removals in one batch, stale owner409, missing404, duplicate-create409, actual last owner version, first acknowledged relation followed by rejection, and source-prepared/current-authority fencing. No such runtime case has executed.
