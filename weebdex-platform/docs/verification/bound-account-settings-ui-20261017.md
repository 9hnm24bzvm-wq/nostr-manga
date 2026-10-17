# Explicit account preference save in the native form

This isolated frontend batch consumes validated backend settings commit `203b01d36c79401ae84d35a397628c8950125e30`. It adds a normal optional `Save to account` action to the existing Preferences form footer, visible only with a connected public identity. It does not introduce a global settings panel, accordion or automatic account synchronization.

The existing form's default Save flow is retained. The account action opens a normal dialog, captures the actual bound native-account UUID/name from request-bound account access, reads the real account preference version/template/server timestamp, and lists the changed preference categories before the explicit account save. The account's own preview revision is used; the device/native form version is never substituted. The body is exact signed POST `/api/v1/settings`, with `expectedAccountId`, `settings.version`, the actual preview `updatedAt`, and the current application-owned template UUID.

Only known fields actually changed against the form's captured baseline are sent. Unchanged fields, hidden/unselected reference arrays, unknown private fields and unavailable-filter counts are omitted. The backend's qualified known-field merge retains unavailable native references and unrecognized private state. Changed group/uploader form objects contribute only their explicit UUID IDs. Legacy native IDs, duplicate selections, unsupported languages, invalid booleans and noncanonical page sizes fail before a write. The pinned language/field vocabulary is tested against the actual immutable backend schema artifact, rather than inventing defaults or a second hosted-service schema.

A signed-account response is not installed into the cookie Session, device-local preferences or form baseline. Confirmed saves show the actual account result; the ordinary form Save remains an intentional separate action. Closing, leaving, form edits, signer-generation changes and service/catalogue changes invalidate pending work. Account rebinds are protected by the signed expected-account precondition. Denied, stale, malformed or response-lost writes require a fresh preview, without automatic replay or optimistic confirmation. Caller changes to returned settings snapshots cannot mutate the client's captured revision/reference state.

## Evidence

`test-evidence/frontend/bound-account-settings-ui-20261017/` retains the exact finite command, raw output, pinned source hashes and strict summaries.

- 14 actual settings-client/changed-field/schema tests and 12 actual component-script/form-attachment tests pass. These cover real target/version/time capture, unknown/private omission, no-op revision, malformed settings, account rebinding, CAS/authorization failures, response loss, returned-view isolation, concurrent requests, closing, form changes during preview/save and after confirmation, signer changes, and no automatic account work during default preference editing.
- The composed combined batch passes 315 tests, including the list, upload-sort, rating, signed transport/publication, local UUID library and bootstrap regressions.
- Real production preparation passes for both public gate values with exactly five public feature exceptions when enabled. The new exception is the native form-footer account action; existing HTML/classes/styles outside the narrowly reviewed extension remain retained.
- Both composed production-source strict gates pass with zero errors and the existing 15 warnings. All 679 supplied frontend files remain unchanged. No dependency install was performed.
- Independent source review found no blocking defect in the frozen client/schema/component authority, merge, form-generation and response-installation seams.

The stand-ins and source checks are finite regressions, not rendered browser/IndexedDB or deployed-service proof. Genuine browser/signing prompts remain unverified. No real account or private user preference was read or changed, and no production/parity completion is claimed.
