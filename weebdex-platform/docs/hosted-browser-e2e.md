# GitHub-hosted real browser E2E

## Review status

Prepared on an isolated branch. No workflow has been published or executed by
this preparation. All 38 real browser phases remain pending. Validator,
scheduling and short owned OS-process seam checks are separate from browser execution and are never acceptance
evidence. Original runtime/application source has no change; the only existing
test-runner change is a narrow, explicitly pinned browser dependency path seam.
All 676 original frontend files remain byte-preserved.
The three pre-existing reconstructed announcement route additions are separately
pinned in `scripts/ci/frontend-additions.json`; no other frontend additions are allowed.

Tracking: [WD-018 real hosted browser execution](https://github.com/9hnm24bzvm-wq/nostr-manga/issues/18). Related locale/mobile/PWA, source/rendered parity and
security/cleanup acceptance remain separate open requirements.

Dependency correspondence verified 2026-10-13: both official Playwright/core
1.62.1 npm archives match the isolated lock's SHA-512 integrity, and all five
retained package/core entry SHA-256 pins match those archive bytes. Verification
only read and hashed the archives; it did not extract or execute downloaded code.
Archive SHA-256 values:
- Playwright: `1982556a882b246ccb7c16337fab5e4e790292b69f835a2db1011dddc440ed98`
- Playwright core: `954be1e183d0ddb9748fe0d2d08b0b66a9210c74dd75c397aeb70303b9f08a00`
The loader still fails closed if installed entry bytes differ. Independent
review of the implementation passed; workflow publication remains dependent on
the verified source baseline and authorized workflow-file write support.

## Authorized publication and runner

The workflow runs on a push to exactly `ci/hosted-browser-e2e`, plus optional
`workflow_dispatch`. Publication to that branch therefore starts the test and
must wait for independent review and publication approval. A draft PR can be
opened normally without requiring that unreviewed PR be marked ready. There is
no deployment, merge, `pull_request_target`, secret, private runner or persistent
credential creation. Token permissions are only `contents: read`; checkout does
not persist credentials. One job and one non-cancelling concurrency group run
the batches sequentially.

The current connector has run, job, log and artifact readers, but no dispatch
tool. Its generic fetch is GET-only and rejects `/actions/workflows`. The
installation listing does not expose its permission grants, so workflow-file
write permission is still unverified. Do not enlarge App permissions, generate a
token, or route around a rejected workflow write. The reviewed dedicated-branch
push is the ordinary proposed trigger.

GitHub documents standard public Ubuntu runners as fresh 4-core/16-GB/14-GB VMs,
with free standard public-runner minutes. This workflow explicitly skips if the
repository is private and uses `ubuntu-24.04`, avoiding the moving `latest`
label. Artifact storage is a separate limit; only small JSON projections with
one-day retention are uploaded, and no cache is uploaded. The six-hour job
limit is respected by a 345-minute job, 90-minute preparation step, and a
four-hour test observation budget with per-batch readiness/cleanup reserves.
If the budget cannot fit another complete bounded batch, unrun phases remain
pending and the full-suite result fails honestly.

Official references, checked 2026-10-13:
- [Runner resources and filesystem behavior](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
- [Billing](https://docs.github.com/en/actions/concepts/billing-and-usage)
- [Actions execution and storage limits](https://docs.github.com/en/actions/reference/limits)
- [Manual dispatch needs the workflow on the default branch](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/manually-run-a-workflow)
- [Artifact retention](https://docs.github.com/en/actions/tutorials/store-and-share-data)

## Reproducible preparation

Use a fresh checkout, not old absolute paths or recovered runtime roots. The
workflow pins official checkout/setup-node/upload-artifact commits, Node
24.19.0, Rust 1.90.0, Yarn 1.22.22 and Playwright 1.62.1. Isolated `.ci/browser`
npm lock integrity pins official Playwright/core and Yarn archives; hooks are
disabled. Playwright selects bundled Chromium revision 1234, not an arbitrary
system browser. Before every browser launch, the loader verifies the reviewed
Playwright entry bytes, exact core browser revision file, isolated tools lock,
complete installed Playwright/core file inventory, and freshly recorded browser
executable digest. No inherited `NODE_PATH` is used. IPC success is established
only when actual Chromium controls execute successfully.

Native dependencies use the existing reviewed strfry/secp256k1/LMDB/flatbuffers
commit pins and recursive upstream submodule pins. PostgreSQL 18.6 is rebuilt
using the existing official archive SHA-256 bootstrap. The disposable runner's
normal OS compiler/OpenSSL/zlib/zstd prerequisites are not a hermetic lock.
No system PostgreSQL service is started or substituted. Rust's native platform
and guarded provisioner are rebuilt with the retained Cargo lock. Because its
guard embeds `CARGO_MANIFEST_DIR`, rebuilding on the destination is important.
Unrelated nginx builds and the extensively tested original suites are omitted.

Frontend dependencies/build outputs live in a fresh external `.build` copy. The
original 676-file membership, sizes and hashes, plus the separate three-route
addition allowlist, are checked before/after all
preparation. All selected source/integration inputs are frozen and compared
around each owned build command. The existing artifact recorder receives a
receipt derived from the actual successful command exit and joined process
group, and produces the new exact complete manifest/index/CSP pins. Historical
Build19/candidate023 binary hashes are never applied to rebuilt bytes.

## Scheduling and genuine evidence

The plan retains the exact 38-phase set. Each fresh fixture first passes real
combined operational/original projection readiness and then startup. The eight
signed phases retain 600-second bounds; all other journeys retain 300 seconds,
startup 150. Signed restart/namespace/recipient controls retain their 430-second
reserve. Related signed controls stay together; every new fixture has distinct
fresh identities/stores, and no browser profile or signer is transferred.
Compact pagination is rechecked on the fresh reader fixture before activation;
baseline real reader/cover assertions precede disposable last-cover deletion.
The genuine fixed 15-second PG fault requires its registered watchdog's final
terminal provenance. Synthetic images are the only dataset; licensed artwork
and real manga image caches are not restored or downloaded.

Only the supervisor queues/owns Chromium. No guessed ports or external browser
namespace are used. Only newly ready private supervisor origins are consumed.
Network routing and the owned external-deny proxy remain unchanged. Every
passing phase requires its acceptance report, actual steps, no page errors,
exit zero and clean owned runner completion. Every fixture must prove closed
ports, removed keys, no owned survivors and clean terminal teardown before
another starts. A failed phase stops that fixture; an independent fresh batch
may proceed only after verified cleanup. Failure is retained even if a repeated
prerequisite later passes. An interrupted or lost receipt never proves cleanup.

Only the fresh `ci-sanitized-evidence/preparation.json`, `summary.json` and
`phases.json` are uploaded. Original projected files are never uploaded directly.
They project fixed statuses, counts, verified source/build hashes and failed
step indices. Free-form labels and error bodies are never exported. A final
closed-schema validator checks every field/value, known phase/batch membership,
summary consistency and actual generation hashes before the upload step. Unknown
keys (including duplicates at any depth), extra files, URLs, private paths,
personal text or arbitrary error strings
fail closed and prevent upload. They exclude passwords, signer/service/operator private keys, mail
links, cookies, storage states, request/response bodies, raw logs, screenshots,
HAR, traces, databases, runtime roots and public/private user profiles. Raw
private logs stay on the disposable VM. Full-suite success requires every one
of the 38 phases; subset dispatch success is explicitly labeled subset-only.
Only after every input document passes does the gate write new canonical JSON
from validated values, discarding all original formatting and representation.
Failed preparation documents contain no unbound digest or success-only fields.

Pillow and CMake install only the specific official hashed binary wheels in
`requirements-hosted.txt`, with no sdist fallback or dependency hooks. Python
3.12/x86_64 is required explicitly. The Playwright entry/core/revision/lock check
runs immediately after script-disabled npm CI, before invoking its CLI.
If the supervisor's terminal cleanup times out or leaves owned descendants,
the dedicated outer owner reuses the reviewed birth-identity/pidfd/subreaper
methods for narrowly owned forced cleanup. Such a fallback permanently fails
the batch and stops further fixtures; no late cleanup turns it into a pass.
Stop-marker and terminal-receipt errors still enter the owned cleanup path,
retain the partial journal, and block all subsequent fixtures.
