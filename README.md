# nostr-manga

Recovered manga-protocol implementation and WeebDex platform integration.

This is the first GitHub source-preservation baseline of the existing project, not reconstructed development history. Historical receipts are retained as historical evidence. See [checkpoint/RECOVERY-README.md](checkpoint/RECOVERY-README.md) and [current runtime evidence](weebdex-platform/test-evidence/final/http-system-acceptance-current.md).

## Layout

- `manga-protocol-lab/`: original Rust protocol/server, Python tools, browser client, migrations, tests and documentation
- `weebdex-platform/`: integration backend, preserved supplied frontend, additive frontend overlays, deployment templates and tests
- `platform-planning/`: original 21-ticket dependency graph, source/API inventories and progress snapshots
- `checkpoint/`: export coverage, source/artifact identity and recovery boundaries

Keep both application directories adjacent: the platform Cargo manifest depends on `../../manga-protocol-lab/server`.

## Verified baseline

The final native source is candidate023. Normal executable SHA-256 is `60c0ba20bc0ac3aef27dd70a0b9fad8fe7dfe8ff92913182d0dd38cb761bd0e6`; browser fixture provisioner SHA-256 is `7b82dc67f1a5b00a1af89b70e74e48c11b99ff89991017ab5965326e01a0f013`. These executable bytes are recovered separately, not committed here.

The supplied frontend has 676 byte-identical original files. The additive integrated frontend is Build19/source534. Its 401-file frozen static build is recovered separately. Original protocol migrations total 52; this export preserves their bytes.

Recorded current runtime evidence: 14 HTTP/PostgreSQL cases passed, 67 platform unit/contracts passed, 85 original unit cases passed with two intentionally ignored, two provisioner unit cases passed, and the follow-up original correctness gate executed 85 of 90 integration targets with 104 distinct cases passing and zero final failed/ignored cases. Four large-scale functional targets remain deferred and one audit diagnostic is excluded. The 85 original unit passes/two ignored cases are a separate group. See [the follow-up index](weebdex-platform/test-evidence/final/original-correctness-supplement-001/index.json). The pinned Node/fake-IndexedDB backend seam passed; it is not real browser execution. These bounded groups are not proof that every authored integration or performance test was run. All 38 browser phases remain unexecuted because the available environment blocks Chromium Unix-domain IPC. No production-readiness or performance claim is made.

## Build and run

Read [frontend preservation](weebdex-platform/docs/frontend-preservation.md), [production-readiness boundaries](weebdex-platform/docs/production-readiness.md), [original build instructions](manga-protocol-lab/docs/build.md), and [browser host/transfer requirements](weebdex-platform/docs/browser-e2e-transfer.md) before installing dependencies or running services. Source and pinned lockfiles are tracked; runtime/toolchain/dependency caches are excluded. Build reproducibility procedures do not prove bitwise-identical output on arbitrary hosts.

## Data and credential exclusions

No actual credentials, provider/PAT tokens, private signing-key files, runtime databases, cookies, browser profiles, raw runtime logs, toolchains, dependency caches or manga image datasets are committed. Existing deliberate public test/demo key expressions and synthetic password-negative cases remain source fixtures and must never be used as production authority. Metadata and attribution for excluded image fixtures remain.

## Provenance and rights

Existing license, attribution and provenance notices are preserved wherever supplied. The supplied frontend archive did not establish an upstream LICENSE or a redistribution grant; public publication was explicitly approved by the project owner, but that does not establish third-party rights. No project-wide license has been invented. Review upstream rights before redistribution or production publication.

## Git workflow

GitHub is the canonical source and issue tracker from this baseline onward. Use a separate branch/worktree for each change, make incremental commits with exact test evidence, and open draft pull requests for review. Do not fabricate historical commits or treat local planning snapshots as current issue state. Never force-rewrite `main` to publish a checkpoint.
