# Restoration coverage and packaging correction

2026-10-12. This report records a post-restoration change. Historical checkpoint documents, manifests and acceptance receipts remain unchanged.

## Result

The recovered encrypted archive authenticated successfully: **65,463,487 bytes, 1,262 members**, SHA-256 `7d91fc83e82697c508318a4251b1ac5ff5f0c974b9cbb79904c2040bf6be3067`. However, it contains only **387 of the 392 canonical inputs**, all matching their expected hashes. Five historical diagnostic files were omitted; no alternative same-hash copies exist in the archive. The previous unqualified “full” source-coverage claim was too broad. Successful authentication proves the received bytes, not completeness against an independent source inventory.

The [coverage audit](coverage-audit.json) records exact missing hashes. The missing paths are:

- `server/diagnostics/managed-group-key-storage-check.rs.txt`
- `server/diagnostics/rebuild-oct8/README.md`
- `server/diagnostics/rebuild-oct8/library_ref_index.rs`
- `server/diagnostics/rebuild-oct8/reclamation_reference_index.rs`
- `server/diagnostics/rebuild-oct8/retention_index.rs`

The original [392-input identity manifest](../rebuild-pool-lifetimes-final/canonical-source-after.json) remains intact. Its SHA-256 is `a26e31ca2d65abfb8bdb500249ebab4c3c68f9c55a90823e686b6c8388b2e763`. The [authorized restoration receipt](../../../recovery/incoming-4c499/restoration-verification.json) has SHA-256 `cb8f37c0005ffab38eebf96daa844ec62225a4a4f1b6a61882be7e2eda5c743a`. No missing file was recreated or substituted.

## Why verification missed the omissions

The archived `checkpoint-tools/checkpoint.py` checks its reviewed selection, builds a manifest from that selection, and verifies every selected archive member against the generated manifest (`checked_files`, lines 26–46; `verify`, 48–65; `pack`, 67–94). All five missing paths were absent from its reviewed allowlist. There was no comparison with the independent 392-input inventory. The archived provenance's unchanged-source statement did not establish that every source input was selected.

The maintained source-packaging CLI had the same coverage weakness: it selected an allowed subset and verified its own generated checksums. Its default directory list omitted `server/diagnostics`, and its component exclusions included `diagnostics`. This is consistent with the lost paths, but the original allowlist-generating script was not recovered, so direct use of this selector by that assembly process is not established. Encryption, CRC and selected-member hashes all passed while independent coverage remained incomplete.

## Narrow correction and actual tests

Only [scripts/package.py](../../scripts/package.py) and [its CLI tests](../../scripts/tests/test_package_cli.py) changed after restoration. The optional `--canonical-input-manifest` accepts a bounded, independently reviewed path/hash map. Required paths must be selected, and their expected hashes must match the actual copied archive bytes after ZIP verification, before atomic promotion. Missing or altered requirements leave an existing output untouched.

Exact required historical diagnostic source and generated browser assets may be included without scanning their excluded directories wholesale. Unsafe paths, credentials/runtime roots, sensitive file suffixes and symlinks remain refused. These restrictions do not replace human review of file contents. Calls without the option explicitly report `canonical_coverage.checked=false`; even a checked receipt proves coverage only against the supplied reviewed manifest.

The final [CLI log](green-cli-003.log) records **12 passing test methods in 0.964 seconds**, including the original six. Evidence is deliberately separated:

1. [Legacy wrong-success observations](baseline-wrong-success.json): three real baseline CLI calls returned success and CRC-good archives despite an omitted existing file, a missing requirement, or altered required bytes.
2. [Initial red tests](red-cli.log): baseline rejected the new option. These are feature-absence failures, not claimed validation-path failures.
3. [Generated-browser regression](red-browser-assets.log): the new validator rejected a required static vendor asset; the unchanged CLI test passed after the narrow exact-path correction.
4. [Actual restored-tree refusal](restored-coverage-refusal-002.log): the historical 392-input manifest now fails for exactly the five missing diagnostic paths, without emitting an archive.

The [independent review](INDEPENDENT-REVIEW.md) approved the frozen patch and evidence. It did not run a second test suite. [Candidate identities](candidate-source.json) bind both changed files and their preserved originals.

## Runnability and remaining boundaries

Static searches found no active Cargo, module, test, example or script reference to the five missing paths; they are distinct from the present admitted-read diagnostic implementation. This reduces the apparent active-build impact but does not recover the historical artifacts or prove a fresh build.

The separate [restored-binary smoke](../restored-4c499-smoke-20261012/README.md) verified the exact three executables, shared-library resolution, expected CLI behavior and small published-browser module checks. The accepted application node remains `4c499f724af4db1843f54c51e47746114f777de15b446979a3e8e8a0e7a3e23a`. Its [02:00 UTC full gate](../rebuild-pool-lifetimes-final/canonical-acceptance.json) is preserved historical evidence, not a new gate for the modified packaging files. The smoke's dependency inventory is an initial restoration snapshot; runtime dependency restoration is tracked separately.

No fresh whole-project gate, application rebuild, mixed-load qualification, large-browser qualification or recovery upload follows from this correction. Performance consistency remains unresolved.

## Required next-checkpoint rule

Any future accepted-source checkpoint or GitHub release must require this independent canonical coverage gate, or an equivalent check of the final selected members, in addition to authentication and member integrity. The archived encrypted helper remains unchanged and subset-only; invoking it alone is insufficient. Using the old 392-input manifest must continue to fail while the five files are absent. A future formally reviewed present-tree manifest may state a revised scope and the post-restoration changes; it must preserve the historical omission record rather than silently shrink the old inventory. Secure GitHub destination/authentication setup and upload remain pending.
