# Build19 browser E2E handoff

## Current evidence and execution limit

Build19/source534 is unchanged. Its complete static output contains 401 regular files / 7,008,710 bytes. Manifest SHA-256 is `56dab08d19e57db750a5bbfcb4acb90a17605995bc29aa02f638ea2cc14af429`; index SHA-256 is `4a7f1cc552277aba231db044ca0d2616d70c4f3fe02ebc8d4974b4a5589d004f`; bootstrap CSP is `'sha256-H3VNEClwf0njzAHy9TDF13XtJzCF1acNCLiLIpJ3mnA='`.

A real combined fixture passed 32 operational HTTP checks and projected the real canonical 1002/1001 seeds. The first Chromium launch then failed before a page or user-control step: `process_singleton socket() EPERM`, followed by SIGABRT. Both default exec and the supported `require_escalated` route permit AF_INET but deny AF_UNIX socket creation. No listener or connection was created by those diagnostic probes. Do not retry the unchanged launcher in this same restricted environment or add browser flags/security-policy changes to evade the restriction. Browser journey results remain unestablished. See `test-evidence/frontend/browser-ipc-capability-diagnostic.json` and `browser-batch-build19.json`.

## Required supported host

The current supervisor is Linux-specific: it requires `PR_SET_CHILD_SUBREAPER`, pidfds, descendant identity checks, `/proc`, a writable private fixture root and supported Unix-domain IPC for Chromium. It requires Python3 with Pillow, Node, official Playwright 1.62.1, system Chromium, and the previously verified PostgreSQL/strfry binaries with their runtime libraries. These are prerequisites, not instructions to change system security settings. A connected browser service may be used only through its documented tools and an explicitly supported reachable fixture arrangement.

All services and Chromium must run as children of one owned supervisor in the same reachable loopback namespace. A separate execution call in the current platform cannot reach the supervisor's loopback ports. Never reuse old guessed ports or expose a public tunnel. Obtain origins only from the newly ready private `supervisor.json`.

The runner currently pins Playwright at `/opt/codex/runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright` and Chromium at `/usr/bin/chromium`. It does not depend on inherited NODE_PATH. If a supported destination uses other package paths, stop and review a narrowly explicit path/pin adaptation first; never silently load an arbitrary dependency tree.

## Transfer scope and layout

Do not transfer any prior `test-runtime` root, database, cookies, mail tokens, private signer/service/operator keys, browser profiles, traces, HAR, storage states or revealed API credentials. Start a new disposable fixture and generate fresh synthetic secrets there.

Transfer the preserved application source, `platform-integrations`, reviewed `scripts/browser-*.mjs`, `tests/runtime/browser_fixture.py`, `browser_pg_fault_watchdog.py`, fixture documentation, current hash manifests and build receipts. Keep `weebdex-platform` alongside `manga-protocol-lab`; the supervisor derives PostgreSQL from `../manga-protocol-lab/vendor/pgsql/bin` and strfry from `../manga-protocol-lab/vendor/strfry/strfry`. Transfer those reviewed official binaries/runtime libraries and current native platform/provisioner executables only with exact hash verification. Do not substitute an arbitrary installed server or strip their licensing/provenance.

The static UI can be transferred independently in `artifacts/frontend-static.tar.gz`, SHA-256 `8ea410b4636a0f1e7bf4e226c60c31f81197b2e0954007972ded8153580721a7` (5,160,835 bytes /412 verified regular members). Its `frontend/` payload corresponds exactly to the 401-file artifact manifest. No frontend dependency install or rebuild is needed to serve it.

Signed browser phases additionally require the verified `.build/local-library-noble-2.0.1` archive/package inputs and `test-evidence/frontend/noble-restore/package-provenance.json`; the original signing modules verify their exact existing hashes before import. Do not reuse an unrelated mutable node_modules directory.

## Exact launch and phase ownership

Use the current `test-evidence/frontend/browser-startup-plan-build19.json` as the machine-readable argv and hash handoff. Rehash every selected executable, supervisor/watchdog source, complete UI manifest membership/bytes and runner module after transfer. The plan records the actual reviewed source revisions; historical candidate019/020 plans and receipts must not override the final candidate023 provisioner SHA `7b82dc67f1a5b00a1af89b70e74e48c11b99ff89991017ab5965326e01a0f013`.

The final candidate023 normal platform SHA is `60c0ba20bc0ac3aef27dd70a0b9fad8fe7dfe8ff92913182d0dd38cb761bd0e6`. Launch the full supervisor with those binary pins, the Build19 manifest/index pins, `--second-protocol-recipient`, `--ui-regression-controls`, `--pg-home-fault-controls` and `--lifetime-seconds3600`. Use synthetic image input by default; licensed artwork is not part of this handoff.

Only after genuine operational and protocol readiness, atomically submit one `browser-command.json` in that private runtime root. Startup uses the reviewed Node runner, `phase:"startup"`, `timeout_seconds:150`, `BROWSER_PHASE:"startup"` and a fresh safe BROWSER_RUN_ID. The supervisor injects the exact fixture file/origins and owns the child. Ordinary and UI10/UI11–16 phases have 300-second supervisor bounds; the exact eight signed phases have 600-second bounds. Do not submit a second command while a phase remains active or repeat an uncertain launch.

For each phase, read both the runner's `test-evidence/frontend/browser/<phase>-<run-id>/acceptance.json` and the supervisor's `browser-phase-<phase>.json`. Distinguish launcher failure, executed failed step, true pass and unrun dependency. A passing phase requires actual user-control/persisted-result assertions, no page errors, successful exit and confirmed owned-child cleanup. Record honest limitations; a screenshot or 32 HTTP checks cannot establish all 475 retained visible anchors or globally identical rendering.

Schedule signed restart/namespace/recipient controls early enough to retain their 430-second readiness reserve. Seed named pagination before compact-feed checks and before reader activation. Run baseline cover checks before the empty-tabs phase deliberately removes a disposable cover. The genuine homepage dependency-failure phase requires the registered fixed 15-second watchdog and final terminal recovery/cleanup proof. Preserve deliberate destructive fixture isolation. All 38 phases cannot be assumed to fit one 3600-second fixture; use bounded batches and fresh fixtures after verified teardown.

After each fixture, require closed ports, reaped owned leaders/descendants, removed ephemeral keypairs and clean explicit watchdog status/exit facts. A missing terminal receipt is an unresolved lifecycle, never proof of cleanup. Preserve existing failures and report meaningful new results promptly.

## Final candidate023 recovery pin refresh

The current startup plan and both executable pins were rechecked against `test-evidence/final/backend-current-build.json`, its candidate023 source manifest, successful normal/provisioner build receipt and all-target check. The complete 401-file Build19 UI and all 676 original frontend files were rehashed without changes. The refresh receipt is `test-evidence/frontend/browser-handoff-candidate023.json`. Historical launcher/fixture evidence is unchanged; all 38 browser phases remain blocked in this environment. No build or runtime was performed for this handoff correction.
