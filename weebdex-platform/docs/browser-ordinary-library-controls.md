# Ordinary library browser-control correction

Verified hosted run 38554048951 reached the title route without page errors or
browser/proxy crashes, but the local and account follow checkpoints failed at
`trigger-visible-unique`. The account-recovery journey failed at its first
shared follow precondition. These receipts alone did not reveal the cause.

## Source-proven harness mismatches

The exact composed `MangaActions.svelte` retains one title `div.min-w-45`, its
native Bits UI Select trigger, and an always-mounted Spinner. The Spinner emits
an SVG with `role="status" aria-label="Loading"`. Inactive scaling is a visual
transform, not an accessibility exclusion. The pinned Playwright 1.62.1 injected
naming implementation computes `Add to Library Loading` (or `Reading Loading`),
so the previous exact role-name expression could not select that real control.

The helper now keeps the title structure, native button/select attributes and
visible-only scope, then matches the exact status text. Missing and duplicate
visible native triggers still fail strict operations. This also preserves the
signed migration negative count. Popup/selection checkpoints, actual option
clicks and persisted status assertions are unchanged. No product, markup, CSS,
canonical frontend source or accessibility behavior is modified.

The ordinary dismissal binding previously supplied only `getPublicKey`, while
`connectSigner` first requires both `getPublicKey` and `signEvent`. It therefore
rejected signer compatibility before opening the synthetic dismissed prompt.
The binding now includes a refusal-only `signEvent`; dismissal still happens in
`getPublicKey`, signing is never reached, and the existing empty-outbox assertion
remains required. The application compatibility guard remains intact.

The custom-member checkpoint has a second independent selector mismatch. Native
wrapping labels include their descendant option text in Playwright's
`getElementLabels`: for example `List Choose listLocal example`, so exact
`getByLabel('List')` fails. The shared `nativeSelectForLabel(scope, label)` helper
uses the exact immediate-label `:text-is` engine with a direct child select.
It keeps the existing panel scope and strict duplicate failures. Ordinary List
and Series controls now use it. The actual membership mutation and durable
list/series/position assertions remain unchanged. The initial failed follow
also leaves no saved series to choose; both harness issues must be corrected.

## Regression seams and limits

`test_browser_ordinary_library_source_seams.mjs` composes the actual three overlay
layers and checks the retained trigger/Spinner topology. It executes the
hash-pinned Playwright injected accessible-name and text implementations over
an explicit source-derived JSDOM fragment, including the scale-zero SVG. It
checks all statuses, hidden decoys, missing/duplicate controls and literal
status escaping. It executes pinned label/text selector engines for List,
Series, verified recipient, target type and two distinct dynamic saved-status
labels, preserving exact selection and missing/duplicate failures. It also executes the actual `connectSigner` and `signerRequest`
functions with the exact journey binding, confirming one dismissed prompt,
zero signing calls and no selected owner. Old incomplete bindings are explicitly
shown to fail before the prompt.

JSDOM is 27.0.1, already pinned by the browser-client lockfile. This test adds no
dependency or install. Its DOM layout/driver and writable-store interactions
are stand-ins. It is not Chromium, IndexedDB, a full frontend mount, or hosted
browser acceptance. Existing local/signed diagnostic and account-recovery seams
continue to check interruption/failure propagation and unchanged inventories.

Run from the repository root with existing verified dependencies:

```
WEEBDEX_FRONTEND_ROOT=/absolute/exact-lock/frontend-staging \
WEEBDEX_BROWSER_DOM_NODE_MODULES=/absolute/browser-client/node_modules \
node --test weebdex-platform/tests/ci/test_browser_*.mjs
python3 weebdex-platform/scripts/verify-frontend-parity.py
```

The naming test defaults to the same pinned local Playwright installation as the
browser runtime. Set `WEEBDEX_PLAYWRIGHT_CORE_ROOT` to another existing verified
1.62.1 package only when necessary; its injected-source hash must match. No
browser is launched by these seams. No hosted rerun or publication is implied.
