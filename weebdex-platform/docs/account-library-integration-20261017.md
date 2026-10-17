# Account and library integration, 2026-10-17

This is a selective source integration on recovered commit `8d963f3f`. It is not full platform parity or production qualification.

## Branch audit and integration

- The recovered base already contains the signup-only native welcome, create/import private-key wizard, key backup acknowledgement, optional external signer, native Library/Public identity settings, and local-by-default library selection.
- The later `origin/rebuild/onboarding-source-recovery-20261015` source contains an additional closed-enrollment policy check and explicit session-only key connection. These two source modules and their focused policy/wizard regressions were selected. Historical test results from that branch were not reused as acceptance evidence.
- The divergent settings branch does not add a distinct account/library UI implementation to this base. Its older local-library differences would remove the recovered public-catalogue/UUID safety fences; those differences were not merged.
- Upload recovery, reconstructed public API adapters, and immutable original frontend bytes are unchanged.

## Behavior

New-account signup still offers create, private-key paste/import, and optional external signer choices only after accepted site account creation. Reading does not open this guide. Backup acknowledgement remains required before selecting a locally generated/imported key. A public key can now be copied during the backup step, before any instance discovery, so a failed discovery does not block an operator-approval request. Private keys remain session-only, with explicit reveal/copy and clipboard-history warnings.

Instance enrollment policy is read without a key or signature. Closed enrollment displays operator-approval guidance and offers explicit local session connection without claiming that admission succeeded. Join rechecks current policy before key activation/admission. Closing or superseding the guide fences deferred work. Deferring setup returns it to the initial state and discards an unfinished draft.

Device library mode remains the default. Signing in does not move saved titles or change a selected library mode. Public publication remains an explicit reviewed action. The reconstructed UUID-v2 saved-library write/publication path is still incomplete and remains fenced; backups and inspection are retained. This integration does not claim to finish that missing path.

## Fresh validation

`test-evidence/account-library-integration-20261017/summary.json` records source hashes and exact scope.

- 71/71 focused finite checks passed. They execute production registration, local key/registry/cryptography, enrollment-policy, and wizard-control code with explicitly synthetic auth, lifecycle, clipboard and service boundaries.
- Both default and reconstructed-public-catalogue strict composed checks passed with zero errors and 15 retained warnings per mode. They checked 679 unchanged canonical frontend files and 69/84 reachable integration files, respectively.
- Two pre-existing finite fixtures were repaired to supply the reconstructed preview-fence import contract, without removing the production fence.
- Full fake-IndexedDB/JSDOM wizard and route-ownership suites were not rerun: required pinned packages are unavailable and their install was blocked pending quota approval. The exact install retry was also denied. No alternate fetch was attempted.
- Native browser, backend enrollment HTTP acceptance, and a production Vite build are not established by these results.
