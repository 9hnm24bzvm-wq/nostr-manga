# Reader simplicity and ordinary account tools

## Decisions

- Existing site comments render immediately, using their actual persisted native thread. Guests can read and are asked to sign in only for ordinary account actions. No account-type chooser, synthetic publisher thread, automatic signer selection or implicit signing is introduced.
- Both native catalogue and protocol-imported canonical titles/chapters own native site threads. External source conversations remain separate content under **Comments from other sites**. Their publishing controls are explicitly advanced, with review and signing requirements preserved. Former signed native wrappers remain available as `AdvancedComments`, not ordinary route defaults.
- Fresh reader panels start closed; Settings is labeled. Basic reading controls remain visible and advanced controls are in a native disclosure. Existing persisted panel and reading preferences are not migrated or reset.
- Backup previews lead with saved titles, progress and list names. Support identifiers are optional details; privacy, replace/merge/removal, foreign-device, unresolved-content and cancellation warnings remain visible. Restore authority still uses original opaque frozen handles and original merge semantics.
- Private account list tools use the real native account adapter: bounded pages, explicit create/edit/reorder/delete, list/revision conflict checks and owner/view guards. Unknown writes are never replayed. Checking an uncertain delete/update explicitly reconciles the selected list; checking uncertain creation reads complete bounded list metadata and warns if the reviewed name now exists.
- Guest adoption is opt-in preview then explicit copy, at most 25 selected device saves per group. Reviews show destination, title/list names, progress, preserved conflicts and unavailable saves. Existing account statuses, positions and same-name lists are preserved. Known device positions convert one-based to the account API's zero-based positions; unknown positions are never invented. Confirmed partial receipts are shown; stop/close/account changes prevent the next command. Device source records are never deleted or modified.
- `LocalLibraryPanel` mounts account tools only in qualified public-UUID account mode. Legacy account-to-device import is hidden in public mode. Legacy CSV/XML settings routes require separate account-mode guards.

## Verification scope

Focused mounted-component, core client, finite-adapter and synthetic backend tests have passed during development. Coverage includes cancellation, stale account/view responses, partial or unknown outcomes, bounded pages, no automatic retry and preserved guest preferences. These are not live-browser or production-service proof. Aggregate, release and final browser validation remain pending for this checkpoint. Historical predecessor-dependent qualifications are not rewritten or inherited by current-source tests.

## Relevant sources

- `platform-integrations/frontend-reader-simplicity-overlays.mjs`
- `platform-integrations/public-api/{native-forum,native-user-forum}/{Comments.svelte,AdvancedComments.svelte,index.ts}`
- `platform-integrations/local-library/{ProtocolIdentityPanel,LibraryManagementTools,LocalLibraryPanel,BackupReviewSummary,AccountLibraryTools,GuestAccountAdoption}.svelte`
- `platform-integrations/local-library/guest-account-adoption.ts`
- New focused tests: `tests/ci/test_{fresh_reader_simplicity_dom,account_library_tools_dom,guest_account_adoption}.mjs`
- Updated focused integration tests: native discussion/profile discussion, library route ownership and browser library relocation; browser journey label/advanced disclosure selectors.

