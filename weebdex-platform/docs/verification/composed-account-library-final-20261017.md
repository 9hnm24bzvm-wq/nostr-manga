# Final composed frontend qualification, 2026-10-17

## Result

The isolated validation branch started at integration commit `a4817ce2d1017cb3828e8095704b935ae4ed996f`. The final tested source is `5fec8044aa5ee2e3c009a61f84ddb7484726d586`.

- 627 / 627 cases passed across 45 test files, including the account/library/public-API/relation seams, compiled Svelte DOM cases under jsdom 27.0.1, and the actual production IndexedDB adapter under fake-indexeddb 6.2.4.
- Both full composed strict profiles passed: gate 0 had zero errors / 15 warnings; gate 1 had zero errors / 12 warnings. All 679 canonical frontend files were unchanged.
- The strict checker uses the real published Lucide declaration entrypoint and the exact production alias semantics. No diagnostic suppression or synthetic dependency declaration was added.
- All four complete existing dependency roots matched their before/after manifests. The 191 retained read-only guard receipts contain zero protected write attempts. No package download, installation or cache copy occurred.
- Before/after source hashes matched for all 1036 frontend, overlay, script, test and contract files. The separate 912-file product-input manifest supports association with subsequent backend-only integration changes.

## Narrow corrections while qualifying

The old composition regression expected 40 registry paths and the older group-claim fence label. The composed application has exactly 42 paths, including the actual AppSidebar and author-route additions, and the exact reviewed group-claim label. The test now asserts those precise additions and preserves the existing whitelist-tampering failures. Initial count/label failures are retained.

The actual Vite build owner supplied a staged Lucide package alias, then corrected it to exact root matching. The earlier string alias also matched four canonical `/icons/*` imports, which the full strict graph correctly rejected. The exact root regex leaves native package-export resolution intact for those subpaths. The checker mirrors only this reviewed regex, preserves its source/flags in evidence, and rejects broader or flagged regex aliases. Regressions resolve the real runtime and typed exports for all four canonical icons. The earlier graph failure is retained.

Runtime alias commits `c3945444eba7a2656f9ed9287eee6293d9d50bc4` and `e426015abebfd58e396811d5ecc1d44201f023e4` are present locally as exact cherry-pick equivalents `5971ea4` and `f49460c`; the integrator should keep each equivalence once. The actual Vite owner separately passed both final build profiles on matching configuration bytes, SHA256 `03a34343251e65b83fdc71388fcd2fabb1395d17f411182e0e56b766c8308160`.

The final ordered checker/test successors are `c569ae4`, `6139f5d`, `beb8020`, `c68b41a`, and `5fec804`, with canonical runtime alias commits included separately. No frontend component, overlay or backend bytes differ from the original integration base in this validation lane.

## Evidence and boundaries

`test-evidence/frontend/composed-account-library-final-20261017/` retains reproducible finite/strict commands, the test-file list, raw final and historical failure logs, full strict summaries/diagnostics/child receipts, source/product/dependency manifests and read-only guard reports. The product manifest file SHA256 is `7edafca3f60f752eaefafea59f1faff8f19737a2fba585724ad2b726f1a7e0cf`.

The successful real public relation adapter/API/PostgreSQL fixture is separately qualified and retained with backend commit `3e2240b` and frontend evidence `ee0bee1`. These local DOM/IndexedDB checks do not establish native-browser persistence, crash recovery or rendered end-to-end acceptance. Existing local-default/publication readiness fences remain unchanged. Production parity and deployment are not claimed.
