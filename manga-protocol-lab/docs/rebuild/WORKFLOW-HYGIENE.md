# Workflow helper and status-document inventory

Initial read-only inspection: 2026-10-10 21:36–21:38 UTC. The final functional
gate was then running with frozen source inputs; no helper was executed and no
graph was written during that inspection.

## Retirement completed, 2026-10-10 21:54 UTC

After the combined attempt ended unsuccessfully and its source freeze ended,
all three legacy helpers were preserved byte-for-byte at the indexed `.py.txt`
paths below. Every copy was checked before any active original was removed.
`results/rebuild-workflow-retirement/001-verified-moves.json` records the exact
lengths/hashes and unchanged Python/browser runtime inputs. No legacy helper was
executed, and no Rust or browser runtime was changed for this retirement.

The narrow post-retirement gate passed **55 Python tests** (9.03 seconds) and
**6 packaging CLI tests** (0.29 seconds), in `002-python.log` and
`003-packaging.log` under that evidence directory. This does not establish full
parity: the combined optimized gate must restart after the independent
storage-fixture isolation and HTTP read-ownership corrections are frozen.

## Dependency and reference inventory

| Path                                                                  | Actual inputs/side effects                                                                                                                                    | Active callers found                                                                                                                  | Completed disposition                                                                                                                                                                            |
| --------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `scripts/tickets.py`                                                  | Reads `.scratch/federated-manga-postgres/graph.json`; requires per-ticket `file`, `stories`, dependency/status/evidence fields; rewrites the adjacent README. | None. No invocation/import in current scripts, tests, source, manifests or run documentation. Current audit mentions are not callers. | Preserved unchanged as `docs/history/retired-workflow-scripts-20261010/tickets.py.txt`; active copy removed after the gate ended. No replacement writer was needed.                              |
| `scripts/package-load-supplements.py`                                 | Reads hardcoded `results/library-node` October 8 groups and `RAW_DATA_ARCHIVES.json`; creates old-prefix ZIPs and rewrites the index.                         | None beyond audit discussion.                                                                                                         | Preserved unchanged as `docs/history/retired-workflow-scripts-20261010/package-load-supplements.py.txt`; active copy removed.                                                                    |
| `scripts/split-benchmark-package.py`                                  | Reads/writes `deliverables/manga-protocol-prototype.zip`, copies a full ZIP, writes hardcoded raw partitions and `source-bundle-index.json`.                  | Only historical prose: `docs/history/README-through-checkpoint26.md:89`. No current command/test/import caller.                       | Preserved unchanged as `docs/history/retired-workflow-scripts-20261010/split-benchmark-package.py.txt`; active copy removed. Keep archived prose unchanged under its existing historical banner. |
| `scripts/package.py`                                                  | Current explicit allowlist and reviewed individual evidence, path/privacy checks, hash/CRC verification and safe replacement.                                 | Current operator documentation and actual CLI tests.                                                                                  | Retain. Its recursive `scripts` allowlist no longer includes the retired helpers. This changes source-package inventory, not runtime semantics.                                                  |
| `scripts/test.sh`, `scripts/test-postgres.sh`, `scripts/deep-test.sh` | Current canonical/focused verification; deep-test delegates.                                                                                                  | Current README/build/operator paths.                                                                                                  | Retain; no substitution with the graph helper.                                                                                                                                                   |

Search scope: current `server/src`, `server/tests`, `src`, `tests`, browser source
and tests, `deploy`, `runtime`, `bench`, `scripts`, `docs`, recovered tracker,
toplevel README/CONTEXT, and relevant manifests. Historical result logs, compiled
outputs and vendored dependencies are not current callers. This inventory cannot
prove that an unknown external user's private script never invoked a helper.

Original byte lengths and SHA256 values are fixed in
[the retirement index](../history/retired-workflow-scripts-20261010/README.md).
The verified moves removed only the recoverable active copies. The default
packager excludes `docs/history`; recovery checkpoints must include the archived
originals explicitly. The narrow supplement records the new file inventory and
unchanged Python/browser runtime hashes. The next full gate must use the new
inventory; the earlier unsuccessful frozen run does not cover it.

## Recovered tracker: preserve requirements, retire live-status interpretation

- `.scratch/federated-manga-postgres/graph.json`: 39 tickets, recovered header
  `updated_at=2026-10-08T22:59:00Z`; tickets 37–39 lack `file`; 19–39 lack `stories`.
  All declared issue paths exist. This is structurally incompatible with the old
  writer even without executing it. Preserve the bytes; do not invent missing
  fields merely to make the obsolete writer pass.
- `.scratch/federated-manga-postgres/README.md`: only 30 listed tickets and an older
  frontier. It calls 24/27/29 in progress and 25/26/28/30 pending while the graph
  calls 24–36 done. It needs a prominent **recovered planning snapshot, not current
  acceptance** notice; preserve the historical table itself.
- `.scratch/federated-manga-postgres/spec.md` and `issues/*.md`: original
  requirement/decision material remains useful for parity. Do not declare the
  requirements obsolete just because execution statuses are historical.
- The ordinary source packager excludes `.scratch`; a separately indexed,
  byte-preserved `docs/rebuild/requirements-baseline/` copy now preserves all 39
  original files. `INDEX.md` explains the historical-status boundary and
  `PROVENANCE.json` identifies every original path/hash. The copied requirements
  remain distinct from current completion claims.

## Exact current-document updates

Keep a small current summary and scope each old section by its original time and
build. Do not replace historical 38/42/58/70 results with 81: they are different
runs. The latest verified client is build `72e656def51db26f`, 81 model checks,
plus narrowly scoped native scalar-name save/reload evidence. The latest combined attempt ended unsuccessfully with exit 101; its replacement
remains pending. Future terminal results must be recorded without turning a
partial run into acceptance.

| Path                                              | Misleading/stale surface                                                                                                                                                                                     | Minimal update                                                                                                                                                                                                                                                                        |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `docs/rebuild/ACCEPTANCE-MATRIX.md`               | Title says current; top still describes the 19:32/20:07 audit and no frozen combined pass. Lines 54/64 limit actual browser evidence to snapshot5; line 75 still requires later confirmation of checkpoint5. | Add a current evidence/status block and relabel the existing matrix as the historical 20:07 snapshot. Link 81-check UI evidence, final browser scope and the completed new integrated gate when available. Preserve old rows/counts rather than retrospectively changing their proof. |
| `docs/rebuild/FEATURE-COVERAGE.md`                | “Latest acceptance” is still 20:38; earlier 38-test/snapshot5/pending statements are already clearly chronological but lack the latest parity/client entry.                                                  | Add the latest parity/gate summary and rename 20:38 to prior acceptance. Keep the 19:15/19:32/19:55 chronology unchanged.                                                                                                                                                             |
| `docs/rebuild/STATUS.md`                          | Top says fixes are active/source changing since 21:05; a later section labels the 20:38 gate current.                                                                                                        | After the new gate, replace only the current summary with exact accepted revision/evidence and remaining deployment/load limits; retain older sections under dated historical labels.                                                                                                 |
| `README.md` and `CONTEXT.md`                      | Current reconstruction summaries still describe final combined acceptance generically as open.                                                                                                               | Refresh the short status and pointers after the terminal new-gate result. Do not copy every test count into multiple places.                                                                                                                                                          |
| `docs/rebuild/PARITY-REFACTOR-CLIENT-OPERATOR.md` | Workflow-hygiene row still offers retire-or-repair and documentation corrections as open choices.                                                                                                            | Explicit retirement is now complete and linked to this inventory/index and the source-delta/packaging evidence. No new runtime parity claim follows.                                                                                                                                  |
| `docs/rebuild/PARITY-REFACTOR-BACKEND.md`         | Opening/individual outcomes await the new combined gate; T37–39 still foreground the older picker supplement rather than final Unicode/recovery UI evidence.                                                 | Record the actual new gate outcome, and link the client audit/final browser evidence with native signer/mobile/quota limitations retained.                                                                                                                                            |
| `browser-client/README.md`                        | Early verification text introduces the old 42-check reconstruction run; later sections correctly name 70/81 and final scalar browser evidence.                                                               | Optionally label that early paragraph “historical reconstruction gate” and link the latest supplement once, without rewriting its original count. No code change required.                                                                                                            |

`docs/rebuild/PARITY-REFACTOR-TESTS.md` already distinguishes its 58-test run,
subsequent 70-check storage revision and native final UI supplement. Its focused
counts should remain intact; only the new integrated acceptance link needs to be
added after that gate. `docs/START-HERE.md` already routes to current rebuild docs;
it needs no new independent count ledger.

## Exact historical-document labels

These are dated evidence, not unfinished current work. Add or refresh a short
historical banner pointing at the current rebuild status; retain the original
body, outcomes and limits:

- `docs/RELEASE-20261008.md`
- `docs/FEATURE-COMPLETION-20261008.md`
- `docs/AUDIT-FOLLOW-THROUGH-20261007.md` (its body still says the backend is incomplete)
- `docs/execution-status-20261007T103400Z.md` (old “verification pending” is not a live blocker)
- `docs/requirement-evidence-matrix.md` (explicit October 8 scope, but often linked as current)
- `docs/library-ac-performance.md` and `docs/library-node-performance.md` (old measured revisions; no current capacity certificate)
- `docs/process-diagnostic-20261008.md` (retain the old failed/diagnostic workload qualification)

Already historical but with outdated “current” links:

- `docs/FEATURE-AUDIT-20261007.md`: banner points to the October 8 release/matrix;
  point current-status navigation to `docs/rebuild/STATUS.md` without erasing the
  reproduced baseline failures.
- `docs/coverage.md`: already labels SQLite history, but points to the recovered
  ticket graph for current status; replace that navigation with rebuild status.
- `docs/history/README-through-checkpoint26.md`, `CONTEXT-through-checkpoint26.md`,
  `server-README-through-checkpoint26.md`, `docs-START-HERE-through-checkpoint26.md`
  and `recovered-entrypoints-20261010/*`: preserve as archived snapshots. Their old
  commands/counts are historical text, not instructions to modernize in place.

Preserve `RAW_DATA_ARCHIVES.json`, `RAW_DATA_README.txt`, `SHA256SUMS.json` and all
old results as historical inventory/evidence. Do not run either packaging helper
to regenerate missing raw samples or rewrite their indexes. Current recovery
package manifests must have their own identity and inclusion scope.
