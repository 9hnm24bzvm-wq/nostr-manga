# Six-persona fix verification — current checkpoint

This source checkpoint is **not yet a final release approval**. The final history-filter/export successor and the main account browser run remain pending.

## Completed predecessor checks

These results bind the preserved source checkpoint before the history-filter/export successor. Later source changes require new evidence.

- Full applicable frontend suite: **5,885 passed, zero failed, six unchanged historical skips** (5,891 total). All 1,725 captured inputs were unchanged during execution. No new exclusion was introduced.
- Strict composed frontend: **zero errors, 21 retained historical warnings**. All 679 canonical frontend files were unchanged; 285 integration files were checked.
- Final static build and emitted-CSS/source-association checks passed. The build contains **487 files, 7,899,421 bytes**.
- Integrated Rust checkpoint: **227 normal unit cases**, **three explicitly executed owned-database unit cases**, and **47 actual HTTP scenarios** passed. These cover ordinary account, administrator, moderator, contributor, identity, publication, upload and compatibility boundaries.
- One final upload scenario initially timed out during schema migration, before its business assertions. After bounded cleanup of only passed, stopped synthetic database data, one unchanged-source retry passed. The original failure, its database and all receipts were retained; no test assertion or timeout was weakened.
- Remote signer: 49 focused cryptographic/coordinator/mounted cases passed. Separate actual-dialog browser tests passed cancellation, Escape, focus restoration and stale-account/late-response checks. This was an owned synthetic relay with a finite native-account boundary, not an external signer or a main account browser claim.
- Administrator privacy/hygiene: 30 independent client/mounted cases and the actual HTTP/CLI scenario passed. Persistent recovery markers retain only an identifier and fingerprint; private command fields remain in memory. Exact actor/command settlement, stale-response fences and last-administrator protection remain enforced.

## Exact preserved static predecessor

- Manifest SHA-256: `4b4632f05d840b68f49036bb1e8bf713bd8fa7de6550648547b60d3875335ff3`
- Index SHA-256: `ffab9bc323b810eac36b75557f3fcb6b7fd4c556c6c1657f06a12c89a481a6df`

The optimized server, administrator CLI, fixture provisioner and standalone node build completed successfully with unchanged captured inputs. Explicitly approved acquisition of 20 missing official registry archives verified every existing lockfile checksum; no dependency version was changed.

## Current successor

Inclusive UTC date/operation history filters and a private, same-authority CSV download are now under qualification. The download requires an explicit review and is bounded to 250 rows and 256 KiB, with formula/control sanitization and stale-account/view cancellation. All 35 focused client/mounted cases and its actual HTTP/SQL scenario passed. The complete backend matrix is qualified by an explicit same-product-source union: unchanged executables retain their exact passing records, the narrowly corrected query-schema test passed, and one unchanged-auth-binary diagnostic retry passed without changing assertions or timeouts. Strict checking and the refreshed static build passed; final aggregate and optimized successor artifacts are still pending. The preserved results above do not qualify these newer bytes.

## Limits and remaining acceptance

Main-account browser acceptance remains unstarted until the successor artifacts are qualified. Earlier browser findings and earlier successful builds remain separate evidence, not inherited acceptance. Email simulation and synthetic role setup must be labeled explicitly.

Account CSV/XML import and export remain unavailable. Advanced external identity and remote-signing tools remain optional and separate from normal site login; linking an identity does not grant publication authority. Unconfirmed team/credit writes remain blocked, with read-only inspection or support-assisted recovery rather than unsafe automatic replay. The six historical skips and 21 frontend warnings remain visible limitations, not hidden passes.

No production deployment, main-branch merge, real-user privilege change, external credential use or real publication is implied.
