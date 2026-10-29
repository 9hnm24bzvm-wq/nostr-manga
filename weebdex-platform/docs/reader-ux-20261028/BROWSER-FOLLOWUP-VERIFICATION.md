# Reader and single-account browser follow-up

## What changed

The ordinary site uses the existing browser account. External protocol credentials remain optional, explicitly advanced tools—not another login required for ordinary reading, library, comments, ratings, reports, profile or contribution screens.

The final browser pass found additional integration issues. The follow-up corrects:

- Idle library controls no longer expose a false Loading status.
- Title read badges use their exact visible-chapter observation. A current reader can recover once from a different consumer's confirmed cancelled catalogue lookup; unknown cleanup, ordinary errors, changed accounts and exhausted budgets never trigger that recovery.
- Translation choices display language names and release dates, not UUID fragments or an unsupported recommendation. The existing chapter/edition policy is unchanged. Choosing English resumed the saved second page in the authenticated browser run.
- My Reports clears stale private rows before its child starts the next account-scoped search, avoiding a false empty result.
- Native draft covers use correctly assembled queries under the retained site's URLSearchParams behavior. Related titles can be searched by name; title and cover language controls display names. Existing permissions, exact mutation receipts and uncertainty fences remain in force.
- A completed, explicitly reviewed device-to-account copy refreshes the same account's list panel without resending any copy. Refresh failures preserve the confirmed receipt and offer a read-only retry.
- Profile handles show a valid readable username rather than a resource UUID. Page-size menus include the actual saved value; language counts and preference labels no longer imply extra selections or forced settings. Narrow title-picker controls have separate labels.

No backend source, migration, server binary or fixture-provisioner binary changed in this follow-up.

## Evidence and boundaries

The earlier authenticated browser run exercised real signup, expired-signup recovery, secure logout/relogin, account-library adoption and saved-page resume, comments, rating persistence, following a legacy author link, profile/preferences, list ordering, read/unread updates and backup review. It also produced the defects above. Downstream verified-reader state was an explicitly labelled synthetic fixture precondition: browser email-link consumption was **not** completed. Separate isolated HTTP/PostgreSQL authentication tests cover the real server verification flow.

That immutable browser fixture was stopped cleanly. Its authenticated successes do not automatically qualify later source changes. Current mounted components, actual client transports and bounded lifecycle tests cover the follow-up corrections, including cancellation, late account changes, failed reads and uncertain writes. The cover recovery tests use synthetic transport, not a second live cover upload.

This reader-follow-up checkpoint passed strict qualification (0 errors, 21 existing warnings), the production build, generated CSS, and 5,646 applicable tests: 5,640 passed, none failed, and six historical tests were skipped. Exact hashes are in `browser-followup-receipt.json`. Its guest smoke confirmed the title-control correction and exposed analogous inactive spinner DOM roles on homepage cards; no failed save or actual screen-reader announcement was established. A separately qualified card-spinner presentation successor follows this checkpoint. Its later source must not inherit these test results.

## Known limits

- Account CSV/XML file import and export are unavailable here. The UI says so and does not fall back to a legacy identity or device library. Reviewed device-save adoption and device backup are separate supported actions.
- Optional external-protocol tools have their own authority and availability requirements. Ordinary browser-account access does not grant publishing, moderation, group leadership or other elevated permissions.
- No claim of production deployment, real second-device testing, actual mobile hardware testing, exhaustive accessibility audit, destructive account closure, final chapter publication or a complete scheduled-upload browser cycle is made.
- The applicable suite retains six explicitly historical skips and five excluded standalone modules. Generated CSS and current semantic qualification are checked separately; historical unavailable-source checks do not inherit current acceptance.
- GitHub publication remains subject to its unresolved approval. Prepared trees are not a published commit or branch. No main merge or CI Actions run is implied.

Private browser screenshots, account/session material, runtime databases, mail links, credentials and keys are excluded from public/checkpoint evidence.

## Reproducing test dependencies

The temporary DOM dependency directory was lost during environment recovery. The final follow-up uses JSDOM 27.0.1 and fake-indexeddb 6.2.4 from a newly integrity-locked 48-package resolution; it is not claimed byte-identical to the lost transitive dependency tree. The new package manifests are preserved in `docs/fixtures/reader-test-dependencies-20261028/`. Install that lock into a separate test-only directory using `npm ci --ignore-scripts`; do not replace the application's locked frontend dependencies. The actual frontend compiler remains Svelte 5.55.1 with the existing locked build dependencies. The separately pinned 41-file Svelte review-source subset was restored with exact hash checks.

All earlier source-association metadata remains immutable. The added reader-followup association reverses exact current bytes to the fully qualified spinner checkpoint before older comparisons. Reversibility proves source association, not inherited execution or deployment acceptance.
