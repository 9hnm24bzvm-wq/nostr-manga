# Device history retained-data successor

Immutable product source: `eeafdcdad2abcef4cc96bb11086c89da774942d3`.
Retained-graph/adaptive core: `f845639fb3bdbe3293f91400ddc36d18103b5fe2`.
Owned transport remains the independently reviewed `0e1e9a6a76b80ef846924c9b27721ed99b4df2b4` implementation. Earlier rejected-source receipts remain unchanged.

## Corrected representation accounting

The rejected implementation expanded a shared normalized parent once per chapter with JSON.stringify. One valid 15 KB description shared by 100 chapters exceeded its per-batch threshold even though the actual graph and wire response were small. Retrying repeated the identical error. Separate per-batch and cache checks also did not bound the combined retained representation.

The successor uses one bounded iterative graph walk with actual object-identity deduplication. It charges UTF-16 string content, object and property/edge units under the existing 4 MiB admission cap, with an independent 65,536-visit/pending-work cap. It does not serialize/expand shared parents or recurse through the graph. This is a deterministic retained-data accounting model, not measured JavaScript heap bytes. It must not be described as a 500K memory qualification.

Pending hydrated DTOs, prepared saved-date DTOs, the rolling window, all optional cached pages and the currently displayed DTO pin are checked together before a batch commits. Optional caches are evicted first. Obsolete scanning rows are not retained. The displayed pin is separate from the cache, so eviction cannot pretend the visible DTOs vanished. The component clears a different page, awaits Svelte tick, and only then releases its displayed pin. Same-page continuation keeps displayed DTOs accounted while waiting.

Raw response and parsing intermediates remain independently bounded by the 1 MiB response and 8 MiB action budgets. The graph cap governs admitted representation; it is not an exact peak heap claim about already bounded transport/parse temporaries or rendering/runtime overhead. The full local snapshot and numeric-key sort remain O(N) storage and O(N log N) sorting work outside the hydration action deadline; actual 30K/500K responsiveness and heap remain pending.

## Oversized valid data has an honest fallback

An oversized pending scope halves the next explicit scope, without advancing its IDs. If a one-ID addition cannot fit the current page, the pager reduces page size, clears that scan and visibly restarts at page 1 with an unknown total. Repeated reductions are finite, down to one item. No result is silently dropped to fill a page and no preference is removed. A single item that cannot fit the deterministic representation model produces a terminal stopped error; retrying that poisoned pager makes no new request. The UI offers explicit Reopen history rather than an ineffective Retry or automatic continuation.

Changing page size changes this history-only internal continuation contract: callers must use the returned page/limit and present its restart notice. The mounted component does so. The legacy numeric-total Feed still refuses incomplete device scans; account mode and its original numeric-total contract remain unchanged.

## Source-bound evidence

96 finite Node cases and 14 mounted Svelte/JSDOM cases pass with zero failures/skips. The six additional pager cases cover:

- Shared-parent identity, cyclic/deep iterative traversal and visit/content caps.
- Actual publicHistoryBatch with 100 chapters and a 15 KB parent description; an executed rejected-source counterfactual repeats the same old failure without advancing.
- Many distinct large parents through actual historyJson, including exact wire caps, adaptive scopes, visible page-size restart, combined retained accounting and traversal of every saved ID without skips.
- Displayed rows surviving optional-cache eviction and explicit release allowing the next bounded page.
- Terminal oversized records that never advance or retry the same poisoned cursor.
- A valid small-wire dense resource that exceeds the retained representation model and stops after two HTTP subreads.

The four new mounted cases verify page 1 restart with the returned smaller limit, display release only after the old DOM is gone, same-page display pinning while pending, and explicit stopped/reopen UI for cleanup-unconfirmed and terminal capacity errors. All earlier transport, cancellation, preference, scope/revision/account, 10K/10,001 and old-source counterfactual controls remain green.

Evidence is in `test-evidence/device-history-retention-successor-20261022/qualification.json`, `finite.log` and `mounted.log`. All 679 original frontend files are unchanged. No dependencies/builds were duplicated. No browser, sockets, services, installs, Cargo/Rust, real user data/keys, native-upload work or publishing occurred. Independent review/integration and full type/build/browser/database qualification remain separate gates. Global title sorting and updates caps remain intentionally open.
