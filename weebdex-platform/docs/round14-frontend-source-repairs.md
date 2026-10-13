# Round 14 frontend repairs

Frozen source: `test-evidence/frontend/round14-owner-source-repairs/summary.json` (2026-10-13 08:42:15 UTC). Build15 completed successfully against this exact source; later Round15 edits make that generation historical.

Three independently established source findings were repaired:

1. Entity chapter feeds retain the existing paginator on the last requested page and on a now-empty requested page after deletion. The exact existing paginator is not duplicated on a normal nonempty page. Nonentity continuation/end behavior is unchanged.
2. The two read/unread handlers, card Follow/Unfollow and title Unfollow check the real acknowledgement. Account 401/403/409 and ambiguous outcomes produce truthful feedback only for the still-owned actor/library/target. Admission is captured before waits, acknowledged maps are updated only on success, and mutations are never replayed. Other title update/bell handlers already checked failures and were excluded. Device storage errors were already rendered in the local panel; device error handling is regression coverage, not a claim that all device failures were previously invisible.
3. The mounted public LatestUpdatesGrid distinguishes read errors from valid empty feeds. The mounted private MyUpdatesCarousel settles failures rather than retaining a perpetual skeleton. Both have explicit, loading-disabled Retry controls; private context changes synchronously clear old rows and coalesce the latest safe read. No recommendations or feed records are fabricated.

Exact SHA/anchor exception manifests cover the two entity-paginator repairs and four widget error/empty controls. Unrelated original markup/classes/styles remain checked literally after only enumerated exceptions are normalized. All 676 supplied files remain byte-identical.

Source compilation passed for85 Svelte components and54 scripts. Build15 passed with401 generated files/6,991,798 bytes, unchanged1630 protected inputs and no surviving owned descendants. Neither result establishes browser behavior. Runtime acceptance is unrun: total101/200/201 last-page and empty-page back links; five controls with real401/403/409 or response loss; failed public and personal update reads, Retry, account switch and delayed old responses.
