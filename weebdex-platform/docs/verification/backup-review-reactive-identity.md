# Backup review reactive identity repair

The previous hosted run reached the merge-review click, then timed out waiting for confirmation while a UI error alert was visible. Its sanitized report did not disclose the alert text.

## Reproduced local defect

The two backup handles are opaque authorization objects, registered by identity in `deviceBackupPreviews` and `deviceBackupRestores` WeakMaps. The Svelte panel previously assigned them to deep `$state`. Actual Svelte 5.55.1 runtime `state`, `set` and `get` functions wrap even a frozen plain object, so its WeakMap identity changes. Executing the original panel functions with that runtime reproduces the preview-authority rejection in the panel's normal error handler. A preview-only repair separately reproduces failure at prepared-review confirmation.

Both declarations now use `$state.raw`, preserving identity and reactive top-level replacement. No authority lookup is weakened, no snapshot is accepted as authority, and neither foreign-device acknowledgement nor explicit confirmation is bypassed. The library, store, restore core, and the upload journey are unchanged.

## Qualification and limits

`test_backup_svelte_identity.mjs` loads the exact official Svelte 5.55.1 runtime dependency graph, verified against `svelte-review-runtime-pins.json`. The tag resolves to upstream commit `37ab33c0a3359d75984a6d68e6ebfda10054ad2c`. Its proxy hash matches the project's already retained Svelte runtime pin. The only external runtime module is `esm-env`, supplied as fixed production build flags. Runtime modules and real library functions execute in the same VM realm, avoiding a cross-realm prototype false negative.

The test uses the actual panel function bodies and source-bound, compiler-evidenced lowering of exactly the two opaque assignments through real Svelte `set/get` signals. It is not a full compiled-component, DOM, IndexedDB, browser, or hosted acceptance test. Existing atomic transaction and store adapters remain explicit finite models. The original-panel counterfactual reconstructs and verifies complete prior panel SHA-256 `f62dd91277fbb7f6e511601601072bd1657b56b68c40c18c8dec4847fc16ec5a`; the historical source is not relabeled.

Controls cover both identity boundaries; foreign acknowledgement and its reset; no commit before confirmation; forged/copied and replayed handles; close/dispose invalidation; replacement and merge. The older transaction-core suite retains stale-state, scope, owner, and mid-transaction invalidation tests. The old test's identity-only `$state` adapter now exposes `raw` for initialization but is not presented as a runtime test.

To run the focused controls, supply `WEEBDEX_SVELTE_REVIEW_RUNTIME_ROOT` pointing to the exact verified upstream source graph and `WEEBDEX_DEVICE_RESTORE_BASELINE_ROOT` pointing to the retained exact historical baseline. Run Node with `--experimental-vm-modules --test` on the new test, `device-backup-restore.test.mjs`, and `public-backup-feature-browser.test.mjs`. Each new control has a five-second deadline.

No historical frontend artifact or semantic evidence is modified. A future authorized fresh build inventories the current integration source and creates new evidence. The four other feature cases and the independent upload remain required. This local defect is consistent with the observed backup alert, but the hosted run has not yet verified the repair; the independent signed-preview abort is still unresolved.
