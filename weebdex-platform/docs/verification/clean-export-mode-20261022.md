# Explicit finite qualification from a source export

This candidate adds a no-Git source-input mode to the combined runner. It does
not claim a complete finite pass on the latest integrated application. Only the
30 new finite provenance/control tests and eight existing runner controls have
executed here. Four new controls exercise actual runner main/inventory paths
with explicitly substituted dependency setup and child-process results; those
are orchestration controls, not application acceptance.

## Provenance contract

The caller supplies both an external manifest path and its separately reviewed
SHA-256. The manifest cannot provide its own trusted digest. It contains an
explicit scope, declared origin commit/tree, selected object identities, and
canonical sorted file records with path, regular/executable mode, original Git
blob identity, byte count and SHA-256. Duplicate JSON fields, duplicate files,
case-ambiguous directory/file identities, unsafe/noncanonical paths, symlinks,
submodules and unsupported modes fail closed.

- `repository-tree` proves every file in the declared complete repository tree
  by recomputing its Git tree identity from verified bytes and modes.
- `selected-input-closure` proves each selected tree/blob, not the unselected
  repository. It must include every required source/config/test/fixture root and
  the specifically identified historical evidence inputs used by finite tests.
  The precise minimum selection is `REQUIRED_INPUTS` in the reviewed helper.
- Both modes treat the origin commit only as declared provenance. Neither proves
  ancestry, a hosted publication identity, or that the commit was pushed.

Verification runs before setup and after execution. All files anywhere in the
export must be listed, except the four exact runner-owned runtime roots: its new
`.build/combined-<output-name>` directory, noble/scure dependency aliases, and the
original browser-client node_modules container. These runtime entries have
separate pre/post byte/mode/symlink snapshots. No arbitrary `.build`, output,
cache, Git, or node_modules wildcard is excluded.

The unchanged complete physical dependency snapshots remain mandatory. In export
mode, the official Bits UI source directory is also included in those snapshots;
the external frontend package.json/yarn.lock must match the exported canonical
source and retain exact pre/post byte/mode/mtime identities. These controls do not
substitute external product source for the export. The existing process owner,
all discovered finite tests, required historical counterfactuals, explicit skip
reporting, resource limits and dependency guards remain in use.

Committed-source preparation first checks exact blob identities, types, count and
individual/aggregate sizes using `git cat-file --batch-check`, before requesting
any content. Both preparation and manifest verification enforce 20,000 files,
64 MiB per file and 256 MiB total source bytes. These are finite source-input
limits, not reservations or approval to run an equally sized batch. Oversized
metadata is refused without allocating correspondingly sized file data. An exact
SHA-pinned copy of the pre-correction helper at `ea28c02` proves the original
accepted 320 MiB manifest metadata and requested blob content before any
preflight. The current helper refuses both cases; the harness never allocates
those oversized bodies.

## Preparation and execution

Manifest preparation reads committed Git objects and does not copy a checkout,
install dependencies, run tests, or prove current working-tree bytes. Example:

```sh
python3 -B weebdex-platform/tests/ci/qualification_source_export.py \
  --prepare-from-git /absolute/source-repository \
  --commit REVIEWED_FULL_COMMIT \
  --scope selected-input-closure \
  --output /absolute/outside/source-export-manifest.json
```

Review and retain the returned digest separately. Export exactly the selected
committed bytes with the recorded 0644/0755 modes, without `.git` or added files.
The manifest, local prerequisite configuration and evidence output belong outside
that exported source tree. Then run its actual combined driver:

```sh
python3 -B weebdex-platform/tests/ci/run_combined_qualification.py \
  --config /absolute/outside/pinned-local-inputs.json \
  --output /absolute/outside/new-finite-evidence \
  --phase finite \
  --export-manifest /absolute/outside/source-export-manifest.json \
  --export-manifest-sha256 SEPARATELY_REVIEWED_SHA256
```

No automatic Git fallback is attempted. `strict`, `artifact` and `all` refuse
export mode before creating output or setting up dependencies. Omitting both new
arguments retains the default Git-based behavior. Results use `sourceProvenance`
for exports, not a fabricated `sourceCommit` or HEAD. A source/runtime/dependency
control drift cannot become success merely because child commands returned zero.

## Evidence and limits

`test-evidence/clean-export-mode-20261022/` retains each bounded unit run, exact
final source hashes, and the final 30 + 8 passing results with no skips. Controls
cover missing/extra/changed source, externally pinned and duplicate manifests,
case/path/mode/symlink issues, selected-tree completeness, manifest drift,
source/runtime/dependency-control drift, actual no-Git validation, unsupported
phases and incomplete argument pairs.

Read-only source preparation against earlier patch-A commit
`baddbf20f02a3b775b7fab52c6fa88f0e8e90ac7` produced a 2,679-file / 29,033,408-byte
selected closure and a 769,369-byte manifest. Its digest was
`15fef8ea8bfe9ce47781a1e1c330d09bb8861e24c94ecb7213629f728f948c6b`.
That was a preparation experiment, not this later candidate's source identity or
an executed complete exported-tree run. The earlier 71-check patch-A focused
export remains tied to its own immutable source manifest and must not be relabeled
as this candidate's full qualification.

The complete latest-source no-Git finite run remains pending coordinated batch
admission after current qualification repairs integrate. Strict/build/browser,
real application recovery, capacity, deployment and remote publication remain
separate qualifications. Missing future finite evidence inputs must cause a
failure and a reviewed selection update, never a silent test omission.
