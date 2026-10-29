# Six-persona fix verification — current checkpoint

This source checkpoint is **not yet a final release approval**. Optimized artifact completion and the main account browser run remain pending.

## Completed checks

- Full applicable frontend suite: **5,885 passed, zero failed, six unchanged historical skips** (5,891 total). All 1,725 captured inputs were unchanged during execution. No new exclusion was introduced.
- Strict composed frontend: **zero errors, 21 retained historical warnings**. All 679 canonical frontend files were unchanged; 285 integration files were checked.
- Final static build and emitted-CSS/source-association checks passed. The build contains **487 files, 7,899,421 bytes**.
- Integrated Rust checkpoint: **227 normal unit cases**, **three explicitly executed owned-database unit cases**, and **47 actual HTTP scenarios** passed. These cover ordinary account, administrator, moderator, contributor, identity, publication, upload and compatibility boundaries.
- One final upload scenario initially timed out during schema migration, before its business assertions. After bounded cleanup of only passed, stopped synthetic database data, one unchanged-source retry passed. The original failure, its database and all receipts were retained; no test assertion or timeout was weakened.
- Remote signer: 49 focused cryptographic/coordinator/mounted cases passed. Separate actual-dialog browser tests passed cancellation, Escape, focus restoration and stale-account/late-response checks. This was an owned synthetic relay with a finite native-account boundary, not an external signer or a main account browser claim.
- Administrator privacy/hygiene: 30 independent client/mounted cases and the actual HTTP/CLI scenario passed. Persistent recovery markers retain only an identifier and fingerprint; private command fields remain in memory. Exact actor/command settlement, stale-response fences and last-administrator protection remain enforced.

## Exact static candidate

- Manifest SHA-256: `4b4632f05d840b68f49036bb1e8bf713bd8fa7de6550648547b60d3875335ff3`
- Index SHA-256: `ffab9bc323b810eac36b75557f3fcb6b7fd4c556c6c1657f06a12c89a481a6df`

The final optimized server, administrator CLI, fixture provisioner and standalone node build is in progress. Explicitly approved acquisition of 20 missing official registry archives verified every existing lockfile checksum; no dependency version was changed.

## Limits and remaining acceptance

Main-account browser acceptance against these exact final artifacts is still pending. Earlier browser findings and earlier successful builds remain separate evidence, not inherited acceptance. Email simulation and synthetic role setup must be labeled explicitly.

Account CSV/XML import and export remain unavailable. Advanced external identity and remote-signing tools remain optional and separate from normal site login; linking an identity does not grant publication authority. Unconfirmed team/credit writes remain blocked, with read-only inspection or support-assisted recovery rather than unsafe automatic replay. The six historical skips and 21 frontend warnings remain visible limitations, not hidden passes.

No production deployment, main-branch merge, real-user privilege change, external credential use or real publication is implied.
