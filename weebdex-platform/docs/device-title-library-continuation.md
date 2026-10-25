# Bounded device title-library continuation

## Scope and guarantees

The device My Library route uses an owner-scoped continuation view. Account mode retains the original account Search callback and pagination. Custom-list selection, local reading status and explicit catalogue filters are applied before matching. The library's existing absent-rating behavior remains all saved ratings; unrelated discovery defaults are not injected. The saved device records are never modified by this read/index path.

Each explicit action considers at most 100 candidate identities, or hydrates at most one page of at most 100 titles. Public reads share the reviewed owned history transport's byte, deadline, fragment and unsettled-cleanup accounting, but do not change history behavior. No full-library DTO cache, unbounded fetch loop or deletion of the existing public-adapter cap is used.

The default followedAt order is known from local metadata. Its lightweight keys are sorted once, using validated UTC instants and an ascending ID tie-break. Original timestamp spellings remain displayed unchanged. ISO offsets and fractional spellings are normalized for comparison only, preserving sub-millisecond ISO digits within the existing 40-character timestamp bound; legacy non-ISO dates retain their accepted Date.parse millisecond meaning. A page may be shown after all its predecessors have been considered; the filtered total remains unknown until the complete scan. That O(N log N) local sort is not a measured large-library performance result.

Catalogue-dependent ordering is an observed completed scan, not an atomic server snapshot. No exact matching total or globally sorted result is shown before all candidate titles have been considered. Indexing keeps only private derived ID/ordering keys in temporary IndexedDB storage. A display page is hydrated again, and changed/missing selected sort keys require explicit rebuilding. A non-displayed title may change after its original read without detection: the API supplies no global catalogue revision token. The UI discloses this limitation and the scan start time.

## Exact ordering

The compound IDB key has separate null rank, exact primary order, secondary order and ascending ID tie-break. It is always read with an ascending cursor. Nulls remain last in both requested directions and UUID ties remain ascending. Decimal sign, significant digit magnitude and fractional precision are represented without IEEE-754 rounding. Reversed text has an explicit terminal marker so prefix ordering is reversed correctly. Public title keys are server-issued hexadecimal PostgreSQL comparison bytes, not browser locale sorting or reconstructed lowercase strings.

## Storage ownership and budgets

The scratch database is separate from the saved library. Each index has an unpredictable owner ID and token. Serialized metadata transactions enforce at most four owners, two million index records per owner, 256 MiB estimated ordering-key storage per owner and 512 MiB aggregate estimated key storage. These are admission estimates, not measurements of browser memory or IndexedDB disk overhead. Browser quota can stop earlier. Long keys can hit the byte budget before a large candidate set is fully indexed; no complete-capacity success is then claimed.

Transactions retain their operation and terminal-event observers through confirmed commit or abort. A five-second storage deadline is followed by a 250 ms abort-settlement grace. Unconfirmed operations remain owned and block further storage until they really settle; at most four storage operations can be in flight in one module. Rebuild awaits disposal of its prior index.

Every admitted batch/page renews a ten-minute idle lease. Cleanup checks the exact token, lease generation and touched timestamp within the same readwrite transaction. A previously renewed index is not deleted by a stale snapshot, and a disposed old owner cannot erase a replacement. Idle or crashed scratch indexes are reclaimed by a later opening. Disposal never clears another owner's keys or touches the saved-library database. Quota and storage failures are explicit stopped states with rebuild controls.

## Verification boundaries

The focused tests execute production TypeScript, fake-indexeddb's IndexedDB transactions/cursors, actual Svelte compilation and mounted JSDOM components. They do not launch Chromium, local sockets, servers, Rust builds, PostgreSQL or external CI. Exact cursor ordering is compared with the existing production compareCatalogueRows function. Existing public library/history orchestration remains a separate test boundary.

The 30,000- and 500,000-candidate controls establish only initial bounded admission and truthful incomplete progress. They do not fully index those libraries, exercise real browser storage, measure full-scale memory/latency, or qualify either capacity. Until separately measured, do not describe this as a 500K-library performance acceptance.
