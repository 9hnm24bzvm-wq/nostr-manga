# Six-persona source checkpoint — work in progress

This checkpoint preserves implementation and regression work for the 21 findings in the six-persona audit. It is **not a release approval** and does not replace the preceding qualified build.

## Verified so far

- Frontend semantic check: **0 errors**, with **21 retained historical warnings**.
- Combined integration-focused checks: **139 passed**, none skipped, including the retained query-string override, native account clients, source composition and dependency-integrity checks.
- The latest static frontend candidate built successfully: **487 files, 7,894,884 bytes**. Its integration inputs matched the captured source; the emitted CSS check passed. A later, narrowly scoped administrator privacy improvement still requires a fresh build.
- An exact backend checkpoint passed **227 normal unit cases**, **all three owned database unit cases**, and **47 actual HTTP scenarios**. It covers identity association, administrator bootstrap and role boundaries, moderation, contributor consent, signed metadata concurrency, authentication and upload/readback boundaries. The later administrator privacy change does not inherit this qualification.
- The optional remote signer passed **49 focused cases**: 24 owned loopback WebSocket/cryptographic cases, 13 finite native-account coordinator cases and 12 mounted component cases. Separate actual-dialog browser checks passed Cancel/Escape focus restoration and stale-account/late-response controls, with clean teardown. The test used an explicitly finite native-account boundary, not an external relay or a real user's key.
- The first full applicable frontend aggregate completed with **5,847 passed, 18 failed and six historical skips**. The failures were traced to outdated test composition/source comparisons. All affected corrected suites then passed **162/162**, without changing historical pins or adding exclusions. This focused result is not a replacement for a full final-source aggregate.
- Optimized server, administrator CLI and browser-fixture provisioner binaries built successfully from the captured backend checkpoint. The separately locked standalone node build requires missing checksum-bound dependencies; it has not been qualified.

- The bounded administrator privacy successor now passes **29 focused client/mounted cases**. Persisted recovery markers contain only request identifiers/fingerprints; full private command fields remain in the current view. Exact backend/HTTP, rebuild and full aggregate checks are still required for this successor.

## Still required

The bounded administrator recovery-marker privacy successor, its affected backend/frontend checks, a fresh full applicable aggregate, corresponding rebuilt release artifacts, and main-account browser acceptance remain qualification gates. Unfinished or failed checks are not counted as passing. Later checkpoints may supersede this status.

No production deployment, main-branch merge, automated CI run, real-user grant, external signer connection or real publication is implied by this source backup. Synthetic test setup and finite test boundaries must remain distinguishable from organic user flows.

## Scope and recovery

Ordinary account paths use the site's native account. Advanced external protocol identity and remote-signing actions remain explicit, optional and separately authorized. Account CSV/XML import and export are unavailable; there is no fallback to an unrelated legacy identity. Local backups and explicitly reviewed device-to-account adoption remain separate operations.

See [source provenance](six-persona-source-provenance.md), [administrator runbook](native-administrator-runbook.md), and the included audit documents. Dependencies, build caches, credentials, private keys, runtime databases and private browser evidence are not included in the source checkpoint. The previous qualified encrypted recovery archive and its separate password are not a claim that this newer WIP build is qualified.
