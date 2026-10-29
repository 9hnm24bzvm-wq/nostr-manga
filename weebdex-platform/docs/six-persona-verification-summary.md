# Six-persona fix verification — qualified code, browser pending

**Main authenticated browser acceptance is still pending.** This is a reproducible source/build checkpoint, not release approval or a claim that every possible feature is supported.

## Current results

| Gate | Result |
|---|---|
| Full applicable frontend suite | 5,896 passed; 0 failed; 6 unchanged historical skips; 1,725 unchanged inputs |
| Strict composed frontend | 0 errors; 21 retained warnings; 679 canonical files unchanged |
| Static build and CSS/source association | Passed; 487 files, 7,905,047 bytes; 1,174 build inputs unchanged |
| Normal Rust unit cases | 227 passed |
| Owned-database library cases | All 3 explicitly executed and passed |
| Actual HTTP scenarios | 47 passed, including the final history-filter/export successor |
| Optimized binaries | All four built; 388 backend/protocol inputs unchanged |
| Administrator privacy/hygiene | 30 independent client/mounted cases plus actual HTTP/CLI passed |
| History filters/private CSV | 35 client/mounted cases plus actual HTTP/SQL passed |
| Remote signer | 49 focused cases plus bounded actual-dialog browser acceptance passed |

The backend matrix is an explicit source/binary-checked union, not an inherited broad pass. The query-schema fixture's plaintext400 correction and the unchanged-auth-binary lock-timeout diagnostic retry both preserve their original failures. No assertion or production timeout was weakened. Earlier successful builds and browser findings remain distinct records.

## Exact artifact checkpoint

Qualified code commit: `5dfeb5d36c6165018494d5717a9b410fa791098e` locally, published with identical tree as `8c5f6bcf9d968dc4750d8b5dc3a09f020d7d839c`. Later documentation-only checkpoints do not change these code/artifact bytes.

- Frontend manifest SHA-256: `dcd08134139f72f56a159f25619af4dba58373815e50b02a5051a92bffbda0cb`
- Frontend index SHA-256: `2c1a9c2aa8ef3093d60a1bc673e7f5f450fcfa3f4fdb2392f11eb851fb600ca8`

| Binary under `artifacts/bin` | Bytes | SHA-256 |
|---|---:|---|
| `weebdex-platform` | 55,689,392 | `0f03853a57e48bb09caf98efd0a151f7727871e120ebdd4ccbe80b3c3fa020c3` |
| `weebdex-admin` | 8,918,904 | `5305fb88612dcc94a05f6db3212c3db7b72f6dc73bf7f96a24f8f8a48efd26ba` |
| `browser_fixture_provisioner` | 33,191,024 | `dd9490b8a0bc953d693581cabe6c3843d7f8433167afe9a723873aa9bc5da732` |
| `manga-node` | 27,728,728 | `44112d36d6e7938e41d6f8de3203506f0b657367bdc0607e1da0314fe3903d7e` |

These are Linux x86-64 artifacts, not Windows/macOS executables. Run from the platform directory with the documented PostgreSQL/configuration prerequisites. When using `scripts/run-platform.sh`, explicitly set `PLATFORM_BIN` to the intended `artifacts/bin/weebdex-platform`; do not silently use an older `artifacts/backend` binary. Migrations are a separate explicit operation, not an automatic deployment.

Twenty previously missing standalone dependencies were acquired from the official registry with explicit approval and matched existing lockfile checksums. The remote-signing dependencies have separate exact archive/file provenance and licenses. No dependency version or lockfile was changed during acquisition.

## Honest boundaries

The main account browser run—including actual CSV delivery and full-account integration—remains unstarted at this checkpoint. The separate remote-signer browser fixture used an owned synthetic relay and finite native-account boundary. Synthetic verification/role setup must never be described as organic email verification or real-user authorization.

Account-library CSV/XML import/export remains unavailable. Staff moderation-history CSV is a separate private export, capped at 250 rows/256KiB with explicit review and sanitized spreadsheet cells. Advanced identity/signing remains optional and separate from site login. Unconfirmed team/credit outcomes stay fenced, without unsafe automatic replay. Six historical skips, 21 frontend warnings and retained Rust warnings remain visible limitations.

No production deployment, main merge, real-user privilege change, external credential use or real publication is implied.
