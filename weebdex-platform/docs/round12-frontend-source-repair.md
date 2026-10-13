# Finite UI12 chapter-feed continuation repair

The original non-entity feed relied solely on an IntersectionObserver entry transition. Compact manga cards can leave the same footer inside the expanded viewport after appending a page, so no later transition requests the remaining pages.

The mounted script now uses the imported `feed-continuation.ts` controller. One visibility entry or explicit user action admits at most four additional sequential page reads. After every successful page, it waits for Svelte DOM settlement and rechecks actual footer geometry. When more compact pages remain after that budget, the existing footer contains an explicitly approved, keyboard-reachable outline **Load more updates** button. Failed or nonadvancing reads expose **Retry loading updates** with existing truthful error feedback. During an admitted load the control is disabled; duplicate clicks do not create parallel requests.

Each read and continuation captures the route URL, navigation-start epoch, selected-library context and current callback before yielding. Disposal, target or authority changes prevent further reads and discard late outcomes. A safe GET already sent may finish; its old result cannot attach to the new view. No mutation is replayed. Exact requested page and valid pagination metadata are required; a page containing only previously held chapter IDs is rejected before append. Empty result/end-of-total stops continuation.

Entity feeds retain the existing GenericPagination unchanged. Existing grouping, cards, classes and styles remain unchanged. The one exact non-entity footer control addition is separately enumerated under `approved_feed_pagination_exceptions` in the preservation manifest. All 676 supplied files stay byte-identical; executed JavaScript and globally rendered HTML differ.

## Acceptance still required

- With 300+ allowed chapters concentrated in one/few manga, observe sequential page-three/page-four continuation while the footer stays in range and settlement at the real end.
- With more than four additional compact pages, activate Load more updates by keyboard and verify all later pages remain reachable within another bounded burst.
- Hold an actual read, navigate or switch library context, then release it: no old result, cache write or new continuation attaches.
- Return error, duplicate-only page, unexpected page number or empty page: retain previous acknowledged entries, show truthful feedback and stop automatic requests.
- Verify entity pagination and ordinary scrolling that moves the footer outside the expanded viewport.

This is source-only closure. Composed Svelte/script compiler and literal parity results are recorded separately. No browser/application journey has executed.
