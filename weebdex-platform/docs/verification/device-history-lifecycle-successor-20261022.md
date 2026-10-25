# Device history lifecycle successor

Immutable product source: `0e1e9a6a76b80ef846924c9b27721ed99b4df2b4`.
Rejected predecessor: `a147c4f8f34f5fc0d436fd3f6ece31a55f2fe67e`. Its original receipts remain unchanged.

The response transport now copies into one fixed 1 MiB buffer. Independent limits allow at most 65,536 read attempts and 128 empty fragments. Every request remains within the existing shared 10-second action deadline, 40-request count and 8 MiB byte accounting. A child request controller follows the action while active; healthy EOF detaches without abort/cancel, including JSON or subsequent scope-validation failures after EOF.

Failures own the fetch, eventual response, read and cancellation promises. Cleanup waits up to 250 ms for actual settlement. If it is not confirmed, an explicit cleanup-unconfirmed error poisons the pager. The single transport slot, reader lock and late fulfillment/rejection observers remain owned until the underlying operations settle. Expiration is not reported as successful cancellation and cannot launch another history read. A disposed/reopened view also cannot bypass the occupied transport slot. Arbitrary non-cooperative dependencies cannot be forced to settle; this limitation is reported rather than hidden.

Numeric chronological keys replace lexical readAt sorting, retaining original read_at strings for display. Equal instants tie by ID in both directions. A chunk is now clamped to the exact remaining scan allowance: limit 99 scans 10×99+10, and adaptive scope 12 scans 83×12+4, never 1,089/1,008.

Source-bound finite evidence: 90 Node cases and 10 mounted Svelte/JSDOM cases passed. The dedicated 16-case lifecycle suite tests fixed allocation with tiny/zero fragments, work caps, early abort, pending reads, stalled/rejecting cancellation, late fetch fulfillment/rejection after grace, retained locks, no next request, poisoned deadline behavior, actual ReadableStream cancellation, healthy EOF and exact rejected-source counterfactuals. Initial fixture/assertion corrections are recorded in the receipt.

Evidence: `test-evidence/device-history-continuation-successor-20261022/qualification.json`, `finite.log`, `mounted.log`.

This commit is not integration-ready: independent review identified a further retained-byte accounting defect (shared parents expanded by JSON accounting and no combined page/window/cache bound). That correction belongs to a subsequent product commit. No full build/type/browser/HTTP/database, 500K memory/performance or native-upload qualification is claimed. No browser, sockets, services, dependencies, native code or original frontend file was changed.
