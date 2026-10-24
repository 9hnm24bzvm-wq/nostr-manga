# Native automatic Continue consumer: source-only candidate

Backend contract pin: `acebdaf2e658a2e5805ebc52e2cc31c643aedbf6`, `docs/native-account-continue-contract.md`. This document records authored source and test scope, not runtime acceptance. Main and retained recovery revisions are unchanged.

## Actual title branches

- Device mode retains the actual composed MangaActions aggregate selection algorithm and original multi-edition dialog. A successful history result is required. Undefined, failed, pending, unverified or changing history never means empty history. No-history selects the last descending group; partial history selects the last group with no completed edition; all-completed selects index zero. These are the retained oldest / oldest-unread / newest behaviors.
- Native catalogue plus account mode has one primary automatic Continue button using the new native endpoint. It does not read legacy badge history or require successful device IndexedDB opening.
- Public catalogue plus account mode has one primary bounded Start button and explains native-account Continue is unavailable. There is no cookie or device fallback.
- Device and native-account branches expose one collapsed, explicitly named alternative: “Start from first chapter (ignore saved completion)”. It invokes bounded Start, including when the device aggregate refuses a title over 10,000 chapters. It is not relabeled exact Continue.
- The device chooser captures the original query/preferences/manga context, pathname, navigation epoch, exact history reference and library admission. Its immediate capture-phase link guard also requires an open, uncleared chooser. It does not rely only on the eventual reactive invalidation effect. Device and bounded actions cannot compete through their pending/chooser guards.

## Native authority and lifecycle

The route-specific factory dispatches only configured native API `/user/history/manga/{id}/continue`, with `credentials: include`, `redirect: error`, and the captured native cookie account in `X-WeebDex-Expected-Account`. It never derives identity from a signer. Public, forum and selector defaults are not widened. Completion retains 16 KiB; Continue has an explicit 32 KiB envelope. Both native factories share the existing module-wide unsettled fetch/read/cancel owner.

Every awaited boundary and pre-navigation admission checks library selection epoch (including pending mode transitions), account actor/authority epoch, committed context/account revision, configured endpoint, manga/filter/navigation context and exact catalogue identity. Response acceptance requires the exact 16-key native contract, ordered query scope hash, request correlation, account and catalogue witnesses, compact winner and raw nullable group. Known identity/admission conflicts latch reload; generic failures are not empty history or all-read results.

Canonical scope hashing has its own module-wide actual-promise owner. Its input is at most 32 KiB. Aborted callers stop waiting, but an unabortable WebCrypto digest retains the slot until actual settlement. Late rejection has a handler; remount cannot accumulate digest work. The shared selection deadline remains ten seconds. Router navigation is explicitly outside that deadline and remains owned until the router promise actually settles.

The existing private transport ID helper and minimal row-reference validator are restored byte-for-byte to their prior implementations. Their JavaScript expressions have no multiline flag and already reject trailing LF/CRLF. The former claim of a newline bypass was incorrect. Controls retain valid colon-native/public UUID positives, foreign-prototype refusal, and invalid decoded-JSON rows; no production validation is relaxed or expanded.

## Selection and chooser revalidation

Initial native Continue repeats the original query and requires the same raw group, winner witness, reason, scope and multiplicity. A single edition navigates automatically. Multiple editions open the bounded chooser with a full-set winner independent of its page. Later page actions repeat the original query first; changed intent or page winner/multiplicity requires retry rather than replacement.

Choosing an edition repeats the original query, then sends a singleton query that intersects the original ID set and preserves every other inclusion/exclusion filter. The singleton response must match the selected row and raw group exactly. It proves current account/catalogue admission and that row's eligibility, not the original Continue result in another transaction. No cross-request snapshot is claimed.

Default worst-case preparation reservations are: initial chooser three requests, 320 KiB, 54 rows; later edition page two requests, 288 KiB, 53 rows; chosen edition two requests, 64 KiB, two rows. Each remains within the existing action maximum of eight requests, 1 MiB, 400 rows and ten seconds. No all-ID or whole-history payload is introduced by native Continue.

## Authored finite controls, not executed

- `tests/ci/test_bounded_reader_native_continue.mjs`: 82 cases. Actual current controller, native adapter, canonicalizer, DTO and shared transport execute against synthetic bounded server responses and authority observations. Includes automatic singleton, empty/drift, all-completed reason, chooser/off-page winner, exact ID intersection, stale account/mode/selection/revision/endpoint, reload latch, ignored cancellation/remount, held digest and router, byte/query budgets, strict rows and no local-history upload.
- `tests/ci/test_bounded_reader_consumers.mjs`: 44 cases. Actual native/public overlay composition and extracted actual device handler/admission code, plus existing reader edges. Device cases cover no-history, partial, any-edition completion, all-completed, chooser, pending/failed history, immediate query/preferences/path/manga/navigation changes without flushing effects, and explicit aggregate refusal. Reactivity, aggregate HTTP, dialog rendering and router are synthetic seams; this is not Svelte mounting.
- `tests/ci/test_bounded_reader_runtime_lifecycle.mjs`: 20 cases. Actual runtime in the existing VM harness, including native automatic Continue without DB/badge history and sticky native identity/admission failures. Svelte runes/hooks, network and router remain declared seams.

The marker `chapter10001` demonstrates compact consumer handling without preliminary hydration, not a 10,001-row SQL evaluation. Backend full-set selection, real cookies, browser behavior, actual current Svelte compilation/mounting and full strict/build checks remain pending. Historical 127, 14 and earlier receipts retain their exact source associations and are not inherited. The 8486e9f native round remains unqualified at 80/82: its two direct cross-realm fixture values were refused before reference validation. The successor controls use the actual byte/parser path; they test unchanged validation rather than inventing an old/new newline distinction.

## Remaining integration gaps

Whole-history badge loading remains an incomplete boundedness path. Exact automatic device Continue beyond the retained aggregate limit remains unsupported; the primary action refuses honestly while explicit bounded Start stays available. The old eight mounted cases target the superseded observed-scan title UI and need a new current-mode case plan. The mounted loader now binds actual native-continue source; it does not supply a stub factory or claim those older assertions qualify this candidate. The separate generated-root owner/layout proposal and actual-HTTP fixture/source-pin successor work are still pending.
