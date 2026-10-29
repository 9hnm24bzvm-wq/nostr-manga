# Six-persona source checkpoint — browser acceptance pending

This checkpoint preserves the implementation and non-browser qualification work for the 21 six-persona audit findings. It is **not a release approval**. The main authenticated browser acceptance run remains pending; no claim that all 94 acceptance criteria have passed is implied.

## Qualified current code

- Full applicable frontend aggregate: **5,896 passed, zero failed and six unchanged historical skips** (5,902 total). All 1,725 captured inputs remained unchanged. No exclusion or historical source pin was weakened.
- Strict composed frontend: **zero errors and 21 retained warnings**. The 679 canonical frontend files remained unchanged; 285 integration files were checked.
- Static frontend: **487 files, 7,905,047 bytes**. The build's 1,174 captured inputs were unchanged, its process group closed, and emitted-CSS/source-association checks passed.
- Backend: **227 normal unit cases**, **three explicitly executed owned-database library cases**, and **47 actual HTTP scenarios** passed. The final matrix explicitly combines unchanged tested executables with the corrected staff test and a diagnostic auth retry; every current source/binary relationship is recorded.
- All four optimized Linux x86-64 binaries built successfully: server, administrator CLI, browser-fixture provisioner and standalone node. All 388 captured backend/protocol inputs were unchanged. Dependency versions and lockfiles were not changed.
- Administrator privacy/account-switch hygiene passed 30 independently rerun client/mounted cases and actual HTTP/CLI authorization, recovery and concurrency checks. Persistent recovery markers contain identifiers/fingerprints only; private command fields remain in the active view.
- Moderation history date/operation filtering and the explicitly reviewed CSV download passed 35 client/mounted cases and actual HTTP/SQL tests, including authorization loss, UTC boundaries, formula/control sanitization and whole-row export limits.
- Optional remote signing passed 49 focused cases: 24 owned loopback WebSocket/cryptographic, 13 finite native-account coordinator and 12 mounted component cases. Separate actual-dialog browser tests passed Cancel/Escape focus restoration and stale-account/late-response fences with clean teardown. These are not a full-account or external-signer browser claim.

The original failed attempts remain preserved. A query-schema negative test was corrected to assert the framework's exact plaintext400 contract rather than decode it as JSON. An existing auth race exceeded its unchanged lock timeout while deliberately holding a throttle row; one quiet retry passed with the same binary, assertions and timeouts. These are explicitly recorded, not hidden passes.

## Remaining gate and limits

The main account/browser journeys—including actual filtered CSV download delivery—must still be checked against these exact artifacts. Synthetic verification or role setup must be labeled; it is not organic email-verification evidence.

Ordinary account paths use native site login. Advanced external identity and remote-signing actions remain optional and separate; linking an identity does not grant publishing authority. Account-library CSV/XML import/export remains unavailable, with no legacy identity fallback. The new **staff-only moderation-history CSV** is a different, explicitly reviewed private download capped at 250 rows and 256KiB.

Unknown team/credit write outcomes remain blocked, with read-only inspection or support-assisted recovery rather than unsafe automatic replay. Six historical skips, 21 frontend warnings and retained Rust warnings remain disclosed.

No production deployment, main-branch merge, automated CI run, real-user privilege change, external credential use or real publication is implied. Credentials, private keys, runtime databases and private browser evidence are excluded from source backups. See [verification details](six-persona-verification-summary.md), [source provenance](six-persona-source-provenance.md) and the [administrator runbook](native-administrator-runbook.md).
