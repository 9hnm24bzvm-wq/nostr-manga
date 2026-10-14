# Signed signer-control setup after a failed local-save step

## Observed boundary

Verified hosted run `38554048951`, attempt 1, used reviewed source tree
`0103a59fdbbab71750ddbda27b3ac9b58843ccbb` (local base `01a593b7`). Its
closed `phases.json` records the same boundary in `signed-library`,
`signed-social`, `signed-source-controls`, and `signed-recipients`:

- mapped title navigation passed
- mapped follow trigger visible/unique failed
- the later library navigation, local-record checks and missing-signer refusal
  were pending
- signer binding installation passed
- signer selection denial failed; successful connection and recipient checks
  were pending

This does not demonstrate a signing or recipient-policy defect. The upstream
mapped follow locator is a separate shared-helper investigation. No downstream
signed publication, social action, source moderation, or two-recipient behavior
is established by these failures.

## Source-proven cascade

The actual `step()` implementation in `scripts/browser-acceptance.mjs` records a
failure and returns, so the next outer step still runs. Within the signed
anonymous-device step, a failed mapped follow aborts before its later `/library`
navigation and panel expansion.

The production frontend overlay mounts `LocalLibraryPanel` before app-route
children, including title routes. Its outer native `details` and nested
“Optional Nostr publication and verified recovery” `details` both start closed.
The following signer step installed its binding without navigating or expanding
these controls, then tried to click the hidden connect button. Binding
installation alone never establishes that the rendered signer control is ready.

## Narrow correction

The existing `signer-selection-denied` checkpoint now explicitly navigates to
`/library`, expands both real native details through their summaries, and waits
for the exact connect button before setting synthetic denial and clicking it.
This makes the signer-selection check independent of the preceding local-save
step completing. A setup failure still throws at that checkpoint. A prior
mapped-follow failure remains failed.

No canonical frontend, application mutation, signer implementation, allowed
recipient, policy assertion, diagnostic identity, or evidence gate changes.
The exact denial notice and empty-outbox assertions are retained. The locator
still fails on a missing or ambiguous visible connect button.

## Local verification

- `node --check scripts/browser-signed-journeys.mjs`: passed
- `node --test tests/ci/test_browser_signed_signer_setup.mjs tests/ci/test_browser_signed_select_controls.mjs tests/ci/test_browser_library_follow_diagnostics.mjs`:
  19 tests passed
- `python -m unittest discover -s tests/ci -p test_journey_diagnostics.py -v`:
  11 tests passed
- `git diff --check`: passed

The new nine-case seam executes the actual setup, denial, and synthetic signer
getPublicKey callback branch with explicit native-details/locator stand-ins. It checks the actual composed app
layout, initially closed panel markup, recovery after a preceding title failure,
every setup interruption, absent/duplicate controls, the retained empty-outbox
assertion, native empty-string boolean attributes, wrong-page/frame ownership
refusals, and the inability of navigation alone to satisfy the denial assertion.

These are local source/driver seams, not browser acceptance. No browser,
application, signer, service, fixture, database, package installation, hosted
workflow or publication was run for this correction. Hosted rerun remains
required to establish actual signed behavior after the shared follow fix.

## Native-select labels exposed during source review

The same review found an independent label mismatch in unentered downstream
controls. LocalLibraryPanel wraps native selects inside labels for List, Series
and the saved-series status. ProtocolIdentityPanel does the same for Verified
recipient and Target type. The label's descendant text includes option text, so
an exact `getByLabel` request for only the leading label text can fail.

All eight signed native select calls now use the shared `nativeSelectForLabel`
helper, which selects a direct native select under a scoped label whose immediate
normalized text exactly matches. No prefix matching, first-match fallback or
option-value change is introduced. Library scope, recipient selection, source
target selection and the saved-series status choice remain explicit.

`test_browser_signed_native_select_controls.mjs` executes every actual signed
selection expression with explicit strict-locator stand-ins. Its three cases
check all eight exact panel/label/value combinations, the actual option-bearing
panel markup, and rejection of missing or duplicate controls. The shared helper's
source seam separately executes the pinned Playwright native-label and immediate
text matching engines. Neither test establishes actual publication, recipient
or saved-library acceptance.

Combined local verification with ordinary shared-helper corrections `766abf0`
and `03fb4d7`: all 62 browser-source seam tests, 11 Python journey-diagnostic
contract tests, and 676 canonical frontend file hashes passed. The dependency
stage was the existing exact-lock frontend and JSDOM stage; nothing was
installed. This combined result remains source/DOM/driver evidence only.
