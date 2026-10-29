# Six-persona source checkpoint — work in progress

This checkpoint preserves implementation and regression work for the 21 findings in the six-persona audit. It is **not a release approval** and does not replace the preceding qualified build.

## Verified so far

- Current frontend semantic check: **0 errors**, with **21 retained historical warnings**.
- Current combined integration-focused checks: **139 passed**, none skipped. These include the retained application's query-string override, native account clients, source composition and dependency-integrity checks.
- A candidate static frontend build completed successfully: **487 files, 7,893,201 bytes**. Its 369 integration source inputs matched their pre-build inventory afterward. The emitted CSS check passed.
- Separate owned HTTP/database tests have exercised native identity association, administrator controls and bootstrap, moderation, contributor consent, signed metadata compare-and-swap, authentication and existing compatibility paths. These scoped results do not substitute the final combined-source rerun.
- The optional remote signer passed **41 focused cases**: 24 owned loopback WebSocket/cryptographic cases, 13 finite native-account coordinator cases and four mounted component cases. The latter use finite dialog primitives and are not real-browser focus evidence. A separate fixture using actual dialog components passed build/start/stop checks; real-browser acceptance is recorded separately.

## Still required

Final backend boundary corrections, the full applicable frontend aggregate, the final combined HTTP/database rerun, rebuilt release executables, and browser acceptance against exact final artifacts remain qualification gates. Unfinished or failed checks are not counted as passing. Later checkpoints may supersede this status.

No production deployment, main-branch merge, automated CI run, real-user grant, external signer connection or real publication is implied by this source backup. Synthetic test setup and finite test boundaries must remain distinguishable from organic user flows.

## Scope and recovery

Ordinary account paths use the site's native account. Advanced external protocol identity and remote-signing actions remain explicit, optional and separately authorized. Account CSV/XML import and export are unavailable; there is no fallback to an unrelated legacy identity. Local backups and explicitly reviewed device-to-account adoption remain separate operations.

See [source provenance](six-persona-source-provenance.md), [administrator runbook](native-administrator-runbook.md), and the included audit documents. Dependencies, build caches, credentials, private keys, runtime databases and private browser evidence are not included in the source checkpoint. The previous qualified encrypted recovery archive and its separate password are not a claim that this newer WIP build is qualified.
