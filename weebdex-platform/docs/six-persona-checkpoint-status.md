# Six-persona source checkpoint — work in progress

This checkpoint preserves implementation and regression work for the 21 findings in the six-persona audit. It is **not a release approval** and does not replace the preceding qualified build.

## Qualified predecessor

The results below describe the preserved checkpoint before the new history-filter/export successor. They are not inherited passes for later source changes.

- Frontend semantic check: **0 errors**, with **21 retained historical warnings**.
- Combined integration-focused checks: **139 passed**, none skipped, including the retained query-string override, native account clients, source composition and dependency-integrity checks.
- The latest static frontend candidate built successfully: **487 files, 7,899,421 bytes**. Its integration inputs matched the captured source; the emitted CSS check passed. This build includes the final administrator privacy and account-switch hygiene corrections.
- An exact backend checkpoint passed **227 normal unit cases**, **all three owned database unit cases**, and **47 actual HTTP scenarios**. It covers identity association, administrator bootstrap and role boundaries, moderation, contributor consent, signed metadata concurrency, authentication and upload/readback boundaries. The privacy successor was rebuilt and rerun against the same complete matrix. One initial schema-migration timeout was retained and passed an unchanged-source diagnostic retry after disposing of only proven-passed, stopped synthetic database data.
- The optional remote signer passed **49 focused cases**: 24 owned loopback WebSocket/cryptographic cases, 13 finite native-account coordinator cases and 12 mounted component cases. Separate actual-dialog browser checks passed Cancel/Escape focus restoration and stale-account/late-response controls, with clean teardown. The test used an explicitly finite native-account boundary, not an external relay or a real user's key.
- The final full applicable frontend aggregate passed **5,885 tests, zero failures and six unchanged historical skips** (5,891 total). All 1,725 captured inputs remained unchanged. The earlier failed aggregate is preserved; all affected harness/source-comparison repairs also passed 162 focused cases. No historical pins or exclusions were weakened.
- All four optimized binaries—the server, administrator CLI, browser-fixture provisioner and standalone node—built successfully from unchanged captured sources. Its 20 previously absent standalone dependencies were downloaded from the official registry with explicit approval, matched every existing lockfile checksum, and did not change versions or execute package hooks.

- The bounded administrator privacy and account-switch hygiene successor now passes **30 independently rerun client/mounted cases**. Persisted recovery markers contain only request identifiers/fingerprints; full private command fields remain in the current view. Its actual HTTP/CLI authorization, recovery and concurrency scenario, strict check, frontend build and full aggregate also passed.

## Current bounded successor

The remaining moderation-history acceptance clause is being implemented: inclusive UTC date and operation filters plus an explicitly reviewed, same-authority CSV download capped at 250 rows and 256 KiB. The export excludes raw private effects and neutralizes spreadsheet formulas. Its 35 focused client/mounted cases and actual HTTP/SQL scenario passed. Strict checking reports zero errors and 21 retained warnings; the refreshed static build passed with 487 files and 7,905,047 bytes. Final full aggregate and optimized successor binaries remain in progress. This is not ordinary account-library CSV/XML export.

## Still required

Qualification of the history-filter/export successor and main-account browser acceptance against its exact artifacts remain gates. Unfinished or failed checks are not counted as passing. Later checkpoints may supersede this status.

No production deployment, main-branch merge, automated CI run, real-user grant, external signer connection or real publication is implied by this source backup. Synthetic test setup and finite test boundaries must remain distinguishable from organic user flows.

## Scope and recovery

Ordinary account paths use the site's native account. Advanced external protocol identity and remote-signing actions remain explicit, optional and separately authorized. Account CSV/XML import and export are unavailable; there is no fallback to an unrelated legacy identity. Local backups and explicitly reviewed device-to-account adoption remain separate operations.

See [source provenance](six-persona-source-provenance.md), [administrator runbook](native-administrator-runbook.md), and the included audit documents. Dependencies, build caches, credentials, private keys, runtime databases and private browser evidence are not included in the source checkpoint. The previous qualified encrypted recovery archive and its separate password are not a claim that this newer WIP build is qualified.
