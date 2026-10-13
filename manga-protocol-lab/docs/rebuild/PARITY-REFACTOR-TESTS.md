# Independent global test-seam and parity audit

Audit opened 2026-10-10 21:00 UTC against the reconstructed October 8 tree.
The 20:38 optimized canonical gate is real evidence for its frozen inputs, not
proof that every architectural promise was implemented. The initial findings
and their completed focused corrections are recorded separately below. All nine
test-quality findings are resolved; a new combined optimized gate is still
required for the subsequently changed runtime.

## Standard and scope

Read the official Matt Pocock [TDD skill](https://raw.githubusercontent.com/mattpocock/skills/main/skills/engineering/tdd/SKILL.md)
and [codebase-design skill](https://raw.githubusercontent.com/mattpocock/skills/main/skills/engineering/codebase-design/SKILL.md).
Tests should enter through the same interface as real callers, assert behavior,
and survive internal restructuring. A public keyword alone does not establish
a useful seam. Deep modules hide complexity behind small useful interfaces;
moving helpers into more files is not itself a design improvement.

The audit covers every source-module test block, the Rust integration-fixture
inventory and SQL/counter candidates, the subprocess worker, browser model and
real-backend harnesses, Python/operator tests, packaging tests and canonical
runners. The recovered comparison tree is October 8 checkpoint26/schema52.
The lost later source cannot be compared byte-for-byte. Independent review
found the actionable gaps below. The focused corrections close those identified
gaps; they do not establish exhaustive failure coverage. No finite suite catches
every failure.

## Historical findings at initial inspection (resolved below)

| ID | Finding and source evidence | Required resolution |
|---|---|---|
| T1 | `scripts/test.sh:11` runs tests before build, while `browser-client/tests/service-worker.test.mjs:6–9` reads generated `public/sw.js`. Fresh build CLI tests exercise installation, not the generated fetch handler. | Build before the browser test suite; keep real generated fetch/bypass outcomes. |
| T2 | `tests/test_mangadex_cli.py:65–98` gives traversal a missing target and makes the symlink point to `/dev/null`. Missing/non-image rejection can mask broken path confinement. October 8 used a real external PNG. | Use valid external PNG targets, with an accepted in-directory control and observable CLI refusal/no packet. |
| T3 | The old actual-router freshness/local-link checks were replaced by synthetic API-index CLI cases only. No canonical command checks the real checkout. | Restore actual-checkout `api_index.py --check`; make local-link validation a CLI contract and test it through that CLI. |
| T4 | `relay_feed.rs:125–141` directly constructs private `ResetOnDrop`/`AbortOnDrop` guards. `serving_role.rs:279–291` constructs private atomic state and calls private transitions. | Drive actual role lifecycle/faults, then retire the old blocks only after replacement passes. Existing real feed reconnect and HTTP serving-fault tests cover much, but do not alone prove role-abort cleanup or sticky cause after unconfirmed shutdown. |
| T5 | Browser `fromTuple` is exported only for its direct test (`codec.test.mjs:88–92`); production uses it inside `readGraph`. `LocalLibrary.setMeta` has only test callers; backup tests inspect the private `relays` key. | Enter malformed tuples through `readGraph`; use actual relay preference operations. Remove unnecessary exports only after caller audit and replacement passes. |
| T6 | Browser `recoverPublic` branches (`publishing.js:270–306`) lack negative operation-level tests for correctly signed wrong-owner/kind/future/expiration heads, malformed pointers and bad/missing graph objects. | Test the real recovery operation against externally supplied signed peer responses and content faults, preserving an accepted control. Native Rust recovery tests are not JavaScript behavior evidence. |
| T7 | Browser 30k/500k codec tests (`codec.test.mjs:57–75`) check counts/uniqueness, not every returned ID and field. This weakness predates reconstruction. | Compare the complete decoded records with the input, allowing only documented bucket order. |
| T8 | `SaveBundle::object_count` and `ReaderDrain::generation` have only test callers; `ManagedServing::start/suspend` are test-only Rust conveniences, although the underlying supervisor operations are documented. | Review whether the explicit SDK contract justifies each surface; otherwise use existing actual caller operations and remove redundant wrappers. Do not confuse legitimate native SDK entry points (`run_job`, `plan_write_relays`) with dead APIs merely because the application embeds no SDK host. |
| T9 | `library_wire` gzip checks and HTTP gzip setup use the production compressor/decoder pair. The independent bounded-response gzip vector tests byte transport, not `decode_body`. | Add a frozen independently generated input to the actual wire decoder and decode compressor output independently. Existing round trips remain useful supplementary composition checks. |

Original architecture promises and their implementation evidence are tracked
in [PARITY-REQUIREMENTS.md](PARITY-REQUIREMENTS.md). At initial inspection,
separate bounded read classes, shared dispatch, foreground/background storage
fairness, consistent head reads and the 16-versus-64 cleanup pacing mismatch
required focused reproductions. The subsequently added public lifecycle and
worker-progress fixtures are reviewed below; a private constant comparison
would not establish these behaviors. Structural once-per-event work cannot honestly be proven by adding
a test-only counter; use externally observed routing/progress under bounded
resources, plus explicit implementation/performance review where necessary.

## Justified source-module seams

The following retained test groups use actual application interfaces or public
codec contracts: `auth`, `manifest`, `series_metadata`, `mangadex_compat`,
`capabilities`, `discovery`, `enrollment`, `blossom`, `service_proof`,
`service_announcement`, `endpoint_policy`, `inventory`, `transfer`,
`transfer_rejection`, `offline_keys`, `background_workers`, `bounded_response`,
`library_admission`, `library_storage`, `library_view`, `library_manifest`,
`library_wire`, `library_client_discovery`, `library_client_bundle` and
`library_client_outbox`, with the T8/T9 resolutions and explicit codec qualifications below.

- `pub(crate)` bounded responses are used across real handlers; an externally
  supplied lease becoming reacquirable is a legitimate ownership outcome.
- Admission tests acquire/process real requests and observe refusal/recovery;
  they do not inspect private semaphore counters. Defaults and validation are
  real operator configuration contracts.
- Worker snapshots are operator-visible lifecycle reports, not private counters.
  They deliberately do not claim progress or health.
- Frozen NIP01, canonical CBOR, UUID/calendar and descriptive-metadata vectors
  provide independent expected values. Malformed/deep/seeded codec cases test
  public rejection or non-panic behavior, not an internal parser helper.
- Existing library graph round trips, deterministic order, duplicates, budgets
  and generation summaries test actual codec contracts. Round trips alone do
  not prove independent interoperability; the frozen chapter vector and actual
  browser/Python/Rust interoperability provide distinct evidence.
- Native durable-outbox tests use reopen, attempts, outcomes, real child kill
  and retry operations. A fixture rename simulates an interrupted on-disk
  transition; it is fault setup, not an outcome oracle.

## Integration oracles and external faults

The fresh SQL-candidate review found no remaining private ledger/table-count or
query-text acceptance oracle in the reviewed active Rust fixtures. This is a
contextual review, not a "zero SELECT" rule. Remaining SQL primarily arranges:

- transaction/advisory locks and causal `pg_blocking_pids` barriers;
- expiry, historical migrations, damaged derived indexes and injected failure
  triggers;
- corruption-target generations or stale worker authority captured before a
  fault;
- actual PostgreSQL connection/process termination and confirmation that killed
  fault sessions are gone.

`http_peers`, `http_upload`, `http_library_gc_crash`, `http_sync_jobs`,
`library_prepared_references`, `library_read_isolation`, `publication_fences`,
`enrollment_fences`, export/health fixtures and staging reclamation distinguish
these arrangements from outcomes. Outcomes use signed HTTP/Nostr content,
actual node/SDK operations, backend import/deletion, exact original bytes,
permission failures, public status/audit and exact-capacity fill/refusal/reuse.
`http_remembered_identity`'s atomic fake-service key is an external peer control,
not a production counter assertion. `peer_blob_reads` is an existing operator
status metric used to check no unnecessary source transfer, not a new test hook.

`multiprocess.py` and `pg_fixture_worker` now use actual event and sync operations:
twelve independent writers, distinct completed batches, authoritative failure
when PostgreSQL stops, pool recovery and exact fresh-process reads. No SQL
ledger or Moka Arc-identity outcome remains in that workflow.

## October 8 removal/replacement map

| Earlier check or group | Current replacement and qualification |
|---|---|
| Private filter SQL shape/truth-table | `http_discovery`: actual signed ID sets for all advertised fields, NULL/language/injection-shaped literals and grammar/type/depth/length refusal. |
| Value-only metadata, paired signature/UUID/calendar, production-generated manifest vectors | Actual bytes parsers with independent frozen vectors; first-pass evidence under `results/rebuild-seams`. |
| Private bounded-response short timeout/disconnect counters | Actual HTTP slow readers, count/byte saturation, independent upload progress, disconnect and full 120-second deadline. |
| Private storage ownership/receipt/observation checks | `storage_lifecycle`, `storage_health_lifecycle` and HTTP lifecycle fixtures: real filesystem ownership, cancellation, generation replacement, exact bytes and health-visible behavior. |
| Generic queue/counter/Arc model assertions | `postgres_foundation`, actual sync queues, publication fences, restore and multiprocess lifecycles. |
| Capacity/credit/refund SQL totals | `capacity`, `credit_lanes`, `blob_receipts`, recovery/retention/HTTP fixtures: exact admitted bytes, overflow, reuse and stale-generation non-refund. |
| Group audit/account rows and enrollment budget/migration rows | Authenticated HTTP audit, real publication/read permissions, failed-transaction visibility, next-slot/lifetime admission, migration and restart. |
| Export/backfill/health/ref-index/private deletion rows | Real nginx bytes/status, public pending/idle/held operations, exact retention/reclamation, approved repair, crash and repeated-restart capacity. Exact plan/inode strategies are not inferred. |
| Peer/backend/moderation/inventory/review/remembered-identity/sync rows | Current same-named HTTP fixtures now observe actual signed content, operator outcomes, replay/refusal, backend presence/absence and public capacity. |
| `library_ref_index`, `reclamation_reference_index`, `retention_index` | Preserved outside Cargo under `server/diagnostics/rebuild-oct8`; lifecycle replacements prove bounded work/retention, not a specific plan/index/buffer count. These are the only removed whole Rust integration fixture names versus October 8. |
| Migration-registration filename assertion | Preserved optional packaging diagnostic; runtime checksum/prefix/schema and restore checks do not prove that every new SQL file was registered. |
| Managed-group plaintext key-row scan | Explicit optional security diagnostic with **no equivalent public proof**. HTTP authorization, key-file modes and backups do not certify absence of secrets in all DB representations. |
| Python model suites | Old standalone auth/query/state/accounting models were retired in favor of actual server workflows; deleting model assertions is not itself lost runtime behavior coverage. |
| Python migration/backup helpers and mocked alias transport | CLI exit/files/no-overwrite/privacy/concurrency checks plus real two-node HTTP backup/restore/alias workflows. The T2/T3 discriminators were restored in the focused corrections below. |
| Browser lastPublication metadata and SW call flags | Public signed-job/clock behavior and returned offline/bypass responses. T1 build-before-test ordering was corrected below. |

Preserved code and mapping records live under `server/retired-tests`,
`server/diagnostics`, `docs/rebuild/retired-tests` and `retired/rebuild-*`.
Preservation is not execution. A renamed test or a similar filename alone is
not evidence of equivalent coverage.

## Runner and evidence boundaries

- `scripts/test-postgres.sh` resets a real database for each `server/tests/*.rs`
  correctness executable and uses `--include-ignored`; `audit_gaps` now requires
  explicit selection. Ordinary Cargo commands can skip the
  important fixtures. Module tests, all-target Clippy, multiprocess PostgreSQL
  restart and durability-setting readout are also included.
- The two source ignores are a child crash harness invoked by its parent and
  the explicitly separate half-million-record Rust codec stress case.
- `audit_gaps.rs` was originally in the canonical glob despite containing
  observation-only responses. It is now excluded from the default correctness
  loop and available through the explicit diagnostic command below. Historical
  executable totals must not be described as feature-conformance proof counts.
- Browser `npm test` is Node/fake IndexedDB/DOM behavior. The real-backend Node
  harness is not a rendered browser. `local-browser.cjs` and the opt-in large
  backup experiment are separate; actual browser evidence must identify the
  exact build and flows, especially native Web Locks, quota/eviction and signers.
- The 20:38 canonical manifest records unchanged inputs and optimized node
  `5b9464e8b635302eb8df55eaeaabf3e40eab647ef2188a925cd2d07730022e97`.
  Later edits require a new gate. Checkpoint extraction/CLI smoke is recovery
  evidence, not application behavior, deployment safety or traffic capacity.
- Existing passing regression migrations are not retrospectively TDD. New
  defect fixes must retain an actual causal red → green slice; test-arrangement,
  compilation and environment failures must be labeled accordingly.

## Focused corrections and evidence

Focused corrections followed the initial read-only review. The following are
new results, not inherited October 8 evidence. These component results are not
a new frozen whole-project acceptance gate.

| Finding | Resolution and evidence |
|---|---|
| T1 | Canonical browser command now builds before testing. `client-final.log` ran build → 58 tests → format check, all passed, build `64e9eceae64e4ac0`. |
| T2 | Real valid PNGs now exist outside the selected originals directory for both traversal and symlink cases. An accepted in-directory control precedes refusal/no-output checks. The CLI's path refusal is also checked, so missing-file/image errors cannot mask confinement. |
| T3 | A new public CLI regression first failed because `--check` accepted a missing local contract. Link checking was added to that CLI, with no-rewrite and valid-link controls. The Python suite also invokes `--check` against the real checkout. `api-links-red.log`, `operator-parity-final.log`. |
| T4 serving | Real HTTP/PG/strfry/nginx replacement passed in 5.43s before the private state block was retired: helper loss reports `failed_closed`/`reader_health`; externally removing the recovery executable reports `shutdown_unconfirmed` with the same cause; restoring it does not erase the fault or reactivate the role; explicit restart recovers. `serving-public-first.log`. The old private test is preserved under `server/retired-tests/rebuild-parity-20261010`. |
| T4 feed | The actual application now uses owned `Feed::new/run`. `relay_queries` now cancels that real worker and observes readiness loss/discontinuity, in addition to PostgreSQL loss/reconnect. The private block is preserved in `docs/rebuild/retired-tests/relay-feed-private-drop-guards.rs`. `results/rebuild-admission/parity-owned-work-final-green.log` includes its 1.55s pass and five other real fixtures, including retained SQL/read ownership after cancellation. `parity-lifecycle-final-green.log` passes all seven actual read-admission and dispatcher tests; `parity-admission-dispatch-final-clippy.log` passes. The focused provenance is recorded in `results/rebuild-admission/parity-admission-dispatch.json`. |
| T5 | All obsolete metadata calls, including the optional browser harness, now use `setRelays`/`relayPreferences`. Tuple rejection enters `readGraph` with independently assembled valid-hash graph bytes. The migrated suite passed58 before removing `setMeta` and the `fromTuple` export, then passed58 again afterward. `client-migrated-first.log`, `client-final.log`. |
| T6 | Sixteen tests now call real `recoverPublic` with externally supplied, correctly signed service/head responses. They cover exact valid recovery, owner/kind/time/expiration, malformed pointers, missing/corrupt/hash-valid-invalid roots, missing/corrupt chunks and immediate descriptor-size refusal. |
| T7 | The existing 30k/500k tests now compare every recovered ID and field against the original input after sorting the documented bucket output. They pass in the full58-test client run. |
| T8 SDK/lifecycle | `SaveBundle::object_count` is retained as immutable host-facing SDK progress metadata, not a DB counter; its contract is explicit in `library-client-delivery-contract.md`. Native `run_job`/relay planning also remain real SDK entry points. Test-only managed-serving `start`, `suspend`, the generation accessor and unused receipt field were removed after callers migrated to actual `prepare`/`activate`/`shutdown_verified`. Raw `stop` is private. Existing nonce/generation verification remains; the Python supervisor's documented CLI operations remain. All6 affected real lifecycle/export targets passed after removal,9 tests total, and targeted Clippy passed. |
| T9 | A frozen Python gzip byte vector enters `library_wire::decode_body`; compressor output is additionally decoded without the production decoder wrapper. `wire-gzip-green.log`, one passed. The earlier paired tests remain supplementary checks. |

### Newly reproduced recovery resource bug

The negative recovery work found a genuine implementation bug: each chunk was
downloaded under a fixed 1,000,000-byte allowance, even when its signed descriptor
declared far fewer bytes. The client accumulated subsequent chunks before its
final graph validator rejected the size. Both oversized and short declared-size
tests failed causally: the peer observed a third object request when only the root
and first offending chunk should have been fetched (`recovery-red.log`).

`recoverPublic` now passes the descriptor's byte bound to `publicObject` and checks
the exact size immediately, including cached duplicate references. The existing
256MiB root aggregate policy and graph/hash validation are unchanged. Both cases
then passed (`recovery-green.log`), followed by all16 recovery cases in the full
client run. This is actual red → green; no large hostile workload or artificial
production counter was used.

### Remaining Oct8 mapping and runner corrections

Additional CLI cases restore exact rich titles/categories/alternate titles,
stable direct/wrapped input, changed metadata producing new identity without a
revision link, unknown reading-status rejection and malformed source/hash maps.
These are regression restorations, not invented runtime failures.

A final Python codec review also separated two formerly masked negatives:
nonminimal byte-string length now has its complete payload, with a valid minimal
control and a separately truncated minimal encoding; collection-budget refusal
now includes a complete valid collection at either side of the configured limit,
rather than only a truncated hostile header. All12 public codec cases passed
(`python-codec-discriminators.log`). No codec runtime behavior changed.

`audit_gaps` is now excluded from the default PostgreSQL correctness loop. It
remains an explicitly labeled characterization command:
`MANGA_TEST_FIXTURE=audit_gaps bash scripts/test-postgres.sh`.
It must not be counted as an asserted feature-conformance fixture. Both shell
runners passed `bash -n` after their edits.

Focused commands and logs are under `results/rebuild-parity-tests`:

- `PYTHONPATH="$PWD/vendor/pydeps:$PWD/tests:$PWD" python3 -m unittest discover -s tests`: **55 passed**, 9.72s (`python-all-confirmed.log`). This final rerun includes normalization and the complete codec discriminator corrections.
- `python3 -m unittest discover -s scripts/tests` with the same dependency path: **6 passed**, 0.39s (`package-final.log`).
- `(cd browser-client && npm run build && npm test && npm run format:check)`: **58 passed**, 18.70s model test phase, formatting passed (`client-final.log`).
- The real serving slice used isolated PostgreSQL55436, a separate database and actual strfry/nginx, with `CARGO_INCREMENTAL=0`, the shared target and `-j2`. No application load measurement was run.

Post-removal serving results are `<fixture>-final.log`: `serving_process`4 cases
in4.62s; `public_export_positive`2.73s; `public_export_holds`5.06s;
`public_export_health`3.23s; `public_export_queue`3.05s;
`http_public_serving`5.72s. `serving-clippy-final.log` passes with `-D warnings`.
The initial broader rerun exposed a fixture synchronization error: reachable
nginx had not yet reported operator state `active`, so killing it correctly
produced `activation` rather than `reader_health`. The failed log is retained
(`http_public_serving-callers.log`); an actual public active-status barrier fixed
the arrangement (`http_public_serving-causal.log`,5.37s). This is not a runtime
defect or a claimed product TDD slice.

The later combined local-storage gate includes these browser changes:
`results/rebuild-local-storage-parity/acceptance.json`, build
`17e9c89ef2b0da1d`,70 model tests passed. It supersedes the58-test client snapshot
for that combined storage revision, while remaining separate from actual-browser
and final optimized server acceptance.

### Follow-up review of added parity fixtures

The new `read_admission_lifecycle`, `relay_dispatch_lifecycle`,
`http_live_dispatch`, `library_head_snapshot`, `library_history_worker`,
`library_validation_reuse`, `relay_source_ownership` and `storage_fairness`
fixtures were also inspected.
They use real application-used resource/lifecycle APIs or HTTP outcomes. The
dispatcher source adapter answers an actual `EventSource::fetch` request and
observes fanout/cancellation; it does not expose a private fetch counter. Library
cache/head/history fixtures use external database locks/search-path views/clock
faults, then actual commit/read/snapshot-conflict or maintenance-progress outcomes.
The storage slice uses real foreground admission, cleanup, reads and cancellation
with external lock faults. Source-ownership coverage aborts a real fetch while
PostgreSQL is externally blocked, then observes bounded read admission and its
release after the existing database deadline; no private permit count is used. These arrangements are legitimately implementation-aware
without making private rows the outcome oracle. Their exact red/green evidence
and runtime qualifications are documented in the corresponding parity reports;
this static review does not replace combined runtime acceptance.

The native-browser UI supplement in
`results/rebuild-browser/final-parity-ui.json` identifies build
`72e656def51db26f`: existing synthetic records survived upgrade, a 128-scalar
Unicode list name saved and survived reload, and a 129-scalar ASCII name was
rejected without adding a list. Its pre-fix causal red was a DOM/spec-model
reproduction, not a native-browser reproduction. This UI evidence does not
cover signer extensions or remote publication.

Local-storage, dispatcher, admission, head-race and fairness corrections still
require the final combined optimized gate. These focused passes do not qualify
native browser storage pressure, every signer, power loss, target traffic or
every possible failure.
