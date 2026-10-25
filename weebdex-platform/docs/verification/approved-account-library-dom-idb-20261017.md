# Approved account/library DOM and IndexedDB qualification

Production source `0d0ad1e8b896b1fbbb4bff109142ab6ea9063f75` now passes both final composed strict gates: zero errors and the existing 15 warnings in each. All 679 supplied frontend files are unchanged. This closes the earlier post-delimiter strict gap; the pre-correction receipt remains historical.

After explicit approval for all pending actions, the exact previously attempted local dependency installation succeeded with scripts disabled: jsdom 27.0.1 and fake-indexeddb 6.2.4. The old npm argv, absolute prefix and pinned package manifest were verified before retrying once. The cleanup owner first confirmed 6,731,415,552 available bytes and the retained-file hash receipt. No alternate registry, dependency cache copy, credentials configuration or manual cleanup was used by this lane. The exact lockfile, hashes and commands are retained in `test-evidence/frontend/approved-dom-idb-20261017/`.

## Results and test corrections

The final combined command passes 482 cases: the previous 393 finite cases and 89 compiled DOM/IndexedDB cases. The initial newly unblocked batch passed 69 of 74 and exposed five obsolete harness expectations. No product code was changed to make these pass:

- The signer helper extraction now includes the actual `clearSelectedSigner` function and matches the updated optional external-signer guidance.
- The compiled Settings fixture explicitly provides its native/UUID availability boolean. Its generic Proxy had otherwise returned a truthy function for that new import, hiding an expected native-mode button. A separate rendered UUID storage-failure case now verifies that the unsupported account button stays absent while backup and recovery remain available.
- Closing a settings view after its exact publication job is durable now must reject the final UI continuation. The tests still check the original signed immutable job, recipient state, device/recipient CAS and zero transport posts; they no longer require the obsolete fulfilled-promise shape.

The initial failures are preserved as compressed historical output. The corrected 75-case batch passes, along with ten new UUID store tests and four newly compiled account collection view tests.

## Storage and rendered coverage

The new store suite executes actual `DeviceStore.open`, original IndexedDB requests/transactions, codec, mutation and reviewed-restore modules on pinned fake-indexeddb. It replaces no transaction with the earlier atomic memory model. It checks fresh anonymous UUID saves for all four record families, exact owner/scope/progress after close/reopen, same-revision CAS from two real adapter connections, abort rollback of records/revision/sidecars, immutable chapter parent, durable namespace fence, explicit service adoption with truthful origin-only archives, deletion/restore resurrection protection, populated legacy preservation and versionchange closure of old facades.

The new DOM suite compiles and mounts the actual following navigation, following view and transformed UserSearch with pinned Svelte 5.55.1 in JSDOM. Its following reader and resource normalization are production modules. It checks anonymous no-read behavior, native-style tabs, actual server rows and source-account propagation, later-page account-rebind error clearing, disabled pagination until explicit Refresh, and directory history-cache removal/disconnect clearing. Transport, leaf follow controls and generic dialog boundaries remain explicitly finite stand-ins.

This is actual compiled DOM and IndexedDB-adapter execution, not native-browser rendering, operating-system disk/crash durability, genuine cross-tab broadcasts, extension UI, focus/portal/touch or external relay acceptance. Reopen and concurrent-connection tests run within an owned fake-indexeddb factory. No real user account, service/database startup or external publication was used. The saved-library full-browser readiness flag remains false; separate genuine-browser and final integrated-source acceptance still apply.
