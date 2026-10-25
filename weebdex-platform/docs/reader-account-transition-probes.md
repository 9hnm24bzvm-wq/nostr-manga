# Reader transition and account denial probes

Baseline: reviewed source commit `01a593b7d167add9a4c8e7936d4e86abd10fcf21`, tree `0103a59fdbbab71750ddbda27b3ac9b58843ccbb`, verified hosted run `38554048951`.

The run passed reader prerequisites and failed the four later reader-transition steps. It passed all account-failure cases except title-unfollow under genuine native 429. The finite artifact has no inner checkpoint evidence for either phase, so it does not establish a product cause.

## Proven reader harness conflict

The settings helper clicks Single and Left-to-right, leaving a native button focused (or the auto-advance switch if it needs changing). The immediately following ArrowLeft reaches that control. The exact composed round6 keyboard handler deliberately yields to buttons, links, editable controls and handled events. Removing that protection would change product behavior and conceal the test setup conflict.

The journey now clicks the retained inert Direction text after settings, then checks that focus is outside the handler's protected control selectors before pressing ArrowLeft. This uses an ordinary visible control-area gesture. It does not call product navigation methods, mutate a store, blur with an internal helper, or click the page-reading surface. Missing or ambiguous text and retained control focus fail closed.

Each of the four original cases has seven fixed checkpoints: open, page-one, image-bytes, settings, shortcut-focus, navigation and destination. The original controlled 403/404, delayed real detail, same-realm, exact last-page, persisted completion and unrelated-page-one assertions remain intact.

## Unresolved account 429 failure

Only the title-unfollow429 case receives thirteen ordered checkpoints: prerequisite, open, account-mode, durable-baseline, trigger, option, pressure-captured, capacity-ready, pressure-released, denial, feedback, no-replay and durable-unchanged.

Its actual gesture finishes while the mutation is held before forwarding. The unchanged capacity helper must first prove native 429/capacity_busy before releasing that request. The same native denial, owned feedback, one request, unchanged durable follows/history, nine-second capacity watchdog and finally cleanup remain required. All other capacity callers retain their previous scheduling. The helper never creates a response, retries a gesture or infers a successful write. The title's existing exact Reading text matcher and shared select helper are unchanged.

No account product fix is justified yet. The next genuine hosted run must identify the first failed checkpoint and still satisfy the original assertions.

## Evidence boundary

- 55 focused tests cover the exact composed keyboard handler with explicit DOM stand-ins; actual shortcut-helper behavior; all 28 reader checkpoint failures; the actual account-step body and all 13 checkpoint failures; and held-gesture cleanup
- Seven relevant Node suites pass:85 top-level test results, including the dependency-backed exact-lock title-selector suite; some files contain additional internal seam cases
- Python CI projection/gate suite:42 tests pass, including fixed inventories and rejection of missing, duplicate, reversed, private and falsely successful records
- Existing composed frontend semantic seams:132 cases pass
- Canonical attachment parity:676 files identical

The original assertion/wait calls remain ordered and unchanged: UI10 has79 original insist calls plus one new recorder guard,16 until calls; UI11–16 has106 insist,43 until and10 stable calls. All25 UI10 and26 UI11–16 original step-call expressions remain identical.

A local Chromium focus-only attempt exited before opening a page because the executor denied socket creation. It was not retried. No genuine browser, application-service or database acceptance is claimed. A fresh hosted run remains necessary. No publication or hosted workflow was started by this work.
