# Device-history bounded continuation candidate

Product source: `a147c4f8f34f5fc0d436fd3f6ece31a55f2fe67e`.
Base: `ca0934534be6b40638eff264151281db8fa6886a`.
The prior integrated product/build qualification remains `d299191cddd668b0fdf7bceb80b753ab14e18695`; its green result is not transferred to this candidate.

## Source-proven failure and narrow change

The old `ChapterHistory.Feed` gathered every completed local chapter ID, hydrated the entire filtered catalogue and only then sorted/paged. In public UUID mode the existing 10,000-ID hydration cap rejected 10,001 records before any request. Slicing the first 100 local IDs would not preserve preferences: title exclusions, chapter language, excluded groups/uploaders and missing resources may leave very sparse matching rows.

Only device history has a locally owned ordering key. The new session captures exact completed `{id, mangaId, readAt}` metadata from the existing validated snapshot, with the existing two-million-record store admission bound and no IDB schema migration. It sorts by the same readAt string ordering and ascending ID tie-break. Each mounted session owns its scope/context generation, library revision, account namespace, preferences and route lifetime. Opening rechecks the snapshot revision and verified full catalogue scope. Changes invalidate old sessions and late rows, including cached-page reads. No signing keys or personal test data are used.

Hydration queries contain at most 100 exact chapter IDs and their captured parent IDs. In public mode title preferences are sent only to those parents; chapter preferences are sent only to the relevant saved child IDs. Both successful response envelopes must remain within the exact scope. Every returned chapter must have exactly its captured parent. Successful omissions are filtered/missing resources, not fabricated placeholders. A failed transport, malformed envelope, wrong parent, changing page total, duplicate or unrequested identity does not advance that batch's cursor. Previously acknowledged complete batches remain resumable.

## Explicit bounds and presentation

One user action shares one budget across all title/chapter HTTP subpages:

- At most 1,000 local candidate IDs scanned.
- At most 100 IDs per scope; smaller exact scopes are permitted.
- At most 40 actual HTTP requests, including byte-prefix subpages.
- At most 8 MiB of raw response bytes per action and 1 MiB per response, counted before JSON parsing.
- A shared 10-second hydration deadline and cancellation signal, not a reset timer per batch.
- At most three cached full pages, with a 4 MiB serialized cache bound; a two-page rolling hydration window is also retained. Each committed batch must fit half that cache-byte bound. Metadata remains separately bounded by the existing snapshot record limit.

When a valid byte-prefix server cannot finish even one whole scope within an action, the next explicit continuation retries a smaller exact scope. It does not consume IDs from the unfinished scope or repeat an unfinishable 100-ID request forever. A single oversized response is an explicit error, not a truncated success.

The device-only component reuses retained Grid/ChapterFeedMangaEntry presentation and adds explicit Continue scanning/loading, Cancel, Retry and page navigation. A sparse exhausted action is not an empty/end result. The filtered total remains null/unknown until the complete metadata snapshot has been scanned successfully. Exact totals describe that completed scan; they do not promise an atomic server catalogue snapshot across separate requests. Live catalogue changes may require a reload.

Only the current page renders; this is deliberately not an unbounded infinite-scroll accumulation. Cached previous pages are available only while retained. Older pages rescan from the start and display that behavior. Unmount/reload discards the session; there is no persistent hydrated page cache. The local metadata snapshot and initial sort are synchronous/O(N) storage and O(N log N) sorting work, outside the per-hydration action clock. Actual 30K/500K browser memory, responsiveness and performance remain unqualified. The existing full snapshot read/clone remains an important large-library limitation, not a claimed scalable persistent index.

## Contract and unchanged paths

The composed `/history` route is the only visible `API.ChapterHistory.Feed` caller found in the retained source. It now uses the owned session in device mode and keeps the original ChapterFeed/callback in account mode. The account request, numeric total and query contract are unchanged.

The internal legacy `ChapterHistory.Feed` requires a numeric exact total. Device callers receive that only if a bounded session action establishes it; otherwise they receive an explicit continuation-required 409, rather than a fabricated total. The mounted device route never uses that legacy branch. The two prior direct device orchestration assertions were moved to the owned API while preserving saved-read-date, exact UUID, unresolved-legacy and no-native-fallback checks. Search/List completion membership behavior is unchanged.

The 10,000-title library and 1,000-parent updates caps are unchanged. Neither title global sorting nor updates is repaired by this shortcut. They require a separately reviewed merge/index design and remain open.

All 679 tracked original frontend files are unchanged. Two exact original-hash-bound markup exceptions add the device/account branch; the preparation verifier declares the new external component and reverses those exact exceptions. No dependency tree or build artifact was copied. The sparse worktree measured approximately 9.2 MiB allocated (6.3 MB apparent before documentation/evidence).

## Finite evidence and limits

The source-bound evidence directory is `test-evidence/device-history-continuation-20261022/`.

- 72 Node cases passed across the new pager/transport suite, retained public library catalogue suite and updated real library orchestration suite.
- 10 mounted Svelte 5.55.1/JSDOM 27.0.1 cases passed using existing dependency roots. They cover unknown sparse totals, retry without mislabeled old rows, actual preference/scope/account invalidation, cancel while opening, unmount/late success, truthful out-of-range emptiness and exact account-only route wiring.
- The executed old-source counterfactual rejects 10,001 IDs before hydration. A second executable control shows why the unchanged global parent-composition helper is not safe for history preference batching.
- The actual raw transport control limits each action to 40 HTTP calls while completing a 100-row fixture whose parent/chapter collections return one-row byte prefixes.
- Real library-store boundary tests exercise revision, account and namespace changes during public hydration, unseen snapshot revisions, unverified scope and owner-generation invalidation.
- The first 22-case run was 21 passed/1 failed because a negative fixture's expected error expression was too narrow. The existing envelope validator correctly rejected the malformed changed-total fixture earlier. The expectation was corrected to include that exact earlier validation; no product check was weakened.

This is not full strict/type/build qualification, actual browser/HTTP/database acceptance, pixel parity, sustained load or 500K capacity evidence. No browser launch, sockets, services, installs, Cargo/Rust work, publishing or denied-operation retry was performed. Independent review and integration are still pending at this candidate's initial evidence freeze.
