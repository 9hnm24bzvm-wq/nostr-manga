# Current separately selectable signed-feature qualification

The current source adds `public-signed-features`: one actual report/draft/chapter
phase with a separate synthetic fixture and unexpanded upload grants. Current
scope is42 distinct phases,13 logical batches, at most14 physical fixtures and54
scheduled executions. The default `public-upload` pair is unchanged. Job/browser
caps remain140/30 minutes with normal sandboxing and sanitized one-day evidence.
See [prospective outcomes and finite qualification](verification/public-signed-feature-qualification.md).
No browser or hosted pass is claimed for this addition. Counts, budgets and run
statements in the older sections below describe historical source generations.

# Historical separate public-upload profile introduction

The current scheduler adds a separately built gate-on `public-upload` batch:
40 distinct current phases, 12 fresh-fixture batches and 52 scheduled executions.
Native Startup and all previous batches retain the default-off profile. The UUID
upload journey uses its own current operational-readiness gate and strict closed
result projection. See [profile and acceptance contract](verification/public-upload-hosted-profile.md).
This addition has only source/controller qualification; no new browser or hosted
execution is claimed. The actor-follow and earlier descriptions below retain
their historical counts and source associations.

# Supplemental actor-follow phase

The current source adds `ui14-actor-follow` in the fresh-fixture `actor-follow`
batch. All 38 historical phases are retained; the current complete scope is
39 unique phases in 11 batches (51 scheduled executions including Startup on
each fixture and existing deliberate repeats). The original Build19 handoff
remains historical. See [supplemental coverage and run boundaries](browser-actor-follow-regressions.md)
and `test-evidence/frontend/browser-actor-follow-plan.json`. This source change
has not run a browser or hosted CI. Rebuild the actual follow-fixed frontend
before accepting these journeys.

The earlier run history and 38-phase descriptions below describe their original
source revisions.

# GitHub-hosted real browser E2E

## Review status

Published on the dedicated CI branch in [draft PR #22](https://github.com/9hnm24bzvm-wq/nostr-manga/pull/22).
The [first actual hosted run](https://github.com/9hnm24bzvm-wq/nostr-manga/actions/runs/38458085821)
prepared the pinned stack and executed Chromium controls on ten fresh fixtures.
Startup failed on each; that initial run passed zero unique phases. All ten
physical fixture teardowns verified closed ports, removed
synthetic keys and zero owned survivors, without forced cleanup. Browser failure
correctly prevented clean lifecycle/full-suite success for that run.

The subsequently repaired [startup-only run 38486342057](https://github.com/9hnm24bzvm-wq/nostr-manga/actions/runs/38486342057)
on commit `5d4a1f85a4a4945add24206ba21283b50e4e659e`, tree
`e5f9ca67a53db94302bcd9af935447d50dacf881`, passed all 19 identified checks
with zero JavaScript page errors and a clean owned lifecycle/physical teardown.
The exact three-file sanitized artifact passed the hosted gate and independent
ZIP-digest, schema, summary and frozen-source checks. This proves startup on
that generation only. The other 37 phases and rendered pixel parity remain
unverified; later source revisions require fresh acceptance.

Validator, scheduling and short owned OS-process seam checks are separate from
browser execution and are never acceptance evidence. Subsequent independently
reviewed application repairs are retained as incremental source commits; the
supplied frontend files are not edited by their executed-source overlays.
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
review of the implementation passed. The verified remote source baseline is
`ffc4e1c2ea2378921d0f4da3a58fa3881f08a768`; workflow publication succeeded without
expanding App permissions.

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
installation listing does not expose its permission grants. The actual reviewed
workflow-file write and dedicated-branch push succeeded. Do not enlarge App
permissions, generate a token, or route around a rejected workflow write.

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

## Startup diagnosis and full-suite selection

Dedicated-branch pushes now select `all`, preserving the original 38 unique
phases in ten fresh-fixture batches. The 49 scheduled executions include each
fixture's startup and repeated reader/compact prerequisite controls. Explicit
dispatch still accepts its selected batch, including startup-only diagnosis on
one new fixture. A subset cannot become a 38-phase pass: its summary lists
unrun phases as pending. Restoring the full selection is not a completed-suite
claim. The original CSP, rendering, page-error, missing-asset and cleanup
assertions, dependency pins, permissions, budgets and evidence gates are
unchanged.

The successful startup diagnostic retained 36 enforced script CSP blocks and
56 console-error observations, including 20 classified as unknown. The supplied
root layout includes an external Google tag and inline tracking body that the
privacy policy intentionally refuses to authorize; the per-page inline/external
block counts are consistent with that enforcement. Sanitized evidence omits
blocked URLs, so it does not establish every event's exact identity. Unknown
console observations remain unresolved. Startup success does not imply a clean
console or an absence of CSP events, and none are suppressed to obtain a pass.

Passive startup listeners record response status/resource/MIME and failed-request
categories privately, capped at 256 observations. Private page/console/CSP
observations are projected into finite error classes and known categories, not
verbatim error messages. Nineteen fixed check IDs are mapped to the original
assertion order; each also records whether the private check name matched the
known original name. Only known synthetic route paths or fixed asset/API path
classes survive projection. Origin hosts, ports, URLs, query strings, unknown
route names, module filenames, identifier/error text and response bodies never
survive. Truncation is explicit, so absence from a capped diagnostic list is not
proof of no failure. No new assertion, request, CSP allowance or browser option
is introduced. The upload gate validates every nested diagnostic field, count,
fixed path and category, binds failed check indices to the phase projection and
page-error counts to the original count, rejects duplicate keys, and emits fresh
canonical JSON only after the entire evidence set passes.
# Native compact-feed prerequisite

After the fresh pinned build, a separate hosted-only stage runs
`scripts/run-feed-projection-tests.sh` through the retained owned-command
supervisor, with a 600-second command bound and a 12-minute workflow step bound.
The native test retains its own 180-second bound and unchanged PostgreSQL
five-second statement/one-second lock budgets. It uses a fresh disposable
loopback PostgreSQL database and must establish genuine HTTP200, canonical
projection parity, pagination and visibility assertions before browser fixtures
may start. A failed, absent or malformed native result blocks browser execution.

Only the closed `native_feed_regression` object is added to `preparation.json`;
the artifact allowlist remains preparation, phases and summary JSON. Native
stdout, raw owned-command receipts, database logs, paths and DSNs remain private.
The result distinguishes not-run, failed and passed, and a pass requires the
real command's zero exit, terminal ownership receipt, no cleanup signals or
surviving members, successful PostgreSQL cleanup and exactly one validated
finite measurement record emitted after every native assertion. Timing values
are observations, not a local or future performance claim. `backend/tests` is
included in the frozen source snapshot because this stage now executes it.
