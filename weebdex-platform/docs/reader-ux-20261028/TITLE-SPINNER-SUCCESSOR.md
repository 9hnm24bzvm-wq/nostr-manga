# Title loading-status accessibility correction

The title's Reading status and notification controls previously kept an inactive
spinner in the accessibility tree. Its CSS scale was zero, but its `role=status`
and `aria-label=Loading` still announced a pending operation after completion.

The two spinner nodes now exist only while their respective operation is pending.
The actual pending announcement, disabled guards, visible classes, surrounding
layout and every business-script byte are unchanged. This is not a change to
account authority, storage, permissions, request replay or backend behavior.

The original 157-row source association and its helper remain immutable. A
[separate exact presentation successor](../fixtures/title-spinner-source-association-20261028.json)
restores the affected source before historical comparisons. Historical tests do
not confer browser acceptance on the new output.

## Qualification

- 39 focused tests passed, including eight actual composed device/account,
  Reading/notification, acknowledgement/refusal loading-status cases.
- 390 affected integration and comparison regressions passed, without skips.
- Fresh strict composed check: zero errors, 21 disclosed warnings; all 679
  canonical frontend source files remain unchanged.
- Refreshed production build passed; generated CSS passed 1/1 and final source
  association checks passed 8/8. All emitted CSS bytes are unchanged.
- Complete applicable final-source aggregate: **5,578 tests; 5,572 passed,
  zero failed or cancelled, six unchanged historical skips** (362.96 seconds).
  A preceding interrupted invocation is retained without claiming a result.

The backend and its release server/provisioner are byte-identical to the
previously verified single-account build. The live browser fixture retains its
private pre-correction frontend snapshot; it is not silently replaced. Organic
signed-in browser validation and remote source publication remain pending. No
production deployment, main merge or GitHub Actions run is implied.

## Single-account release boundaries

The ordinary integrated library, social, profile and contribution paths use the
native browser account. This is not a claim that every possible feature exists
or has completed end-to-end browser acceptance. The
[account surface inventory](account-surface-matrix.md) lists the ordinary paths.

- **Account CSV/XML file import and export remain unavailable.** The UI says so,
  and legacy-ID handlers are blocked rather than writing to a different identity.
  Account lists, explicitly reviewed device-to-account adoption and device backup
  are separate supported operations.
- **Advanced external protocol operations remain separate.** Their keys,
  admission and publication authority are not a second login required by ordinary
  controls, nor a silent fallback when native authorization fails. Existing
  publication/moderation permissions and server configuration still apply.
- **Browser coverage is incomplete.** Fresh guest observations and component
  tests do not establish organic signup, email verification or every signed-in
  workflow. The synthetic verification precondition is separately labeled, not
  presented as organic email-verification evidence. Signed-in browser validation
  is pending user interaction; remote publication is pending approval.
- **Residual checks are explicit.** There are 21 semantic warnings. The unchanged
  applicable-suite policy retains six historical skips and five module exclusions:
  generated CSS is tested separately; two positional historical semantic CLIs and
  two unavailable historical-source initializations do not inherit current
  qualification. No new exclusion hides this correction.

No clean-machine rebuild, complete visual parity, production deployment, main
merge, hosted Actions execution or universal feature-completeness claim is made.

## Exact built output

The refreshed static artifact contains 475 files (7,735,899 bytes), with index
SHA256 `1fee97b0073912c5d44923416d6643b517c96786f20280c70fe796e54dc9e991`.
Its complete artifact-manifest SHA256 is
`0efb09c72ee61847bd864d3385a90973c1aa1ce35d9e70e4afe4cf869dd3292f`.
The new source-association SHA256 is
`db72f81d232b9ab4da1a7f9b2f314bda81ac1ff95342e0d9bd36448ed8704979`.
These are build/source checks, not an assertion of the pending browser journeys.
