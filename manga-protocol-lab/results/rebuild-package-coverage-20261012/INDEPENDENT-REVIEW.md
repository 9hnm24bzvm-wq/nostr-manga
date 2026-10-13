# Independent packaging coverage review

**Approved within the explicit reviewed-manifest contract.** Read-only review of source, exact diffs, real CLI tests and frozen evidence; no duplicate tests, builds or application traffic were run.

Reviewed production SHA256: `04fd9f8c00153dde5b109891025af59b953ea8f891b12a5f25b57a44b8fb8b1b` (`scripts/package.py`). Test SHA256: `56dbd25c521277c669fab1bd3ce5b8d3dbf38eb37d2b220edc74fb08d8628f92`. Candidate manifest SHA256: `0c75135c26efc00c9e2b6d859177df6e360a4247642d7197b5fbd3569fcfb966`. Both before copies match the stated baseline hashes.

## Correctness and boundaries

- `package.py:70–129` bounds the JSON manifest, rejects duplicate keys, empty/oversized maps, unsafe normalized paths and malformed hashes. Runtime/private paths and key/database suffixes cannot be authorized through this option. Historical diagnostic exceptions are restricted to source/document suffixes; generated browser exceptions cover static assets and their public vendor tree.
- `package.py:175–183` extends normal selection only for exact required diagnostic/public paths. It does not scan either excluded tree wholesale. Ordinary calls retain their previous selection. Missing required selected paths fail before ZIP creation (`229–233`).
- `package.py:247–297` streams selected bytes, records their hashes, verifies the temporary ZIP's CRC and content hashes, then compares the required identities with those copied bytes. It does not merely hash an already incomplete selected-file list and call that completeness. A changed required file cannot reach atomic replacement.
- Existing source/ancestor/output symlink checks, final no-follow regular-file opening, modes, temporary cleanup and atomic replacement remain intact. Failure cases preserve the previous output. These checks are not a blanket claim of resistance to arbitrary concurrent hostile filesystem races.
- The CLI receipt reports whether coverage was checked and the exact supplied manifest identity and counts. `checked=false` is not accepted canonical coverage. Even `checked=true` establishes only that every entry in the supplied independently reviewed manifest was copied faithfully.

## Test quality and provenance

The frozen `green-cli-003.log` reports 12 actual CLI test methods passing in 0.964 seconds: six existing methods and six additions. They invoke the executable interface, reopen the produced archive, check literal members/hashes and unchanged prior output, and cover omitted/altered required files, exact reviewed historical/public inclusion, excluded neighbors, privacy restrictions, malformed/duplicate paths and existing symlink failures. No private helper or implementation counter is an acceptance oracle.

The red evidence is appropriately separated:

1. `red-cli.log` initially fails because the baseline does not recognize the new option. It is not a validation-path regression red.
2. `baseline-wrong-success.json` separately records three real legacy CLI successes with CRC-good archives that omitted an existing diagnostic, omitted a missing requirement, or copied altered required bytes. That supports the original subset-only coverage defect; this independent review did not rerun those temporary fixtures.
3. `red-browser-assets.log` reaches the new validator and rejects a required static browser vendor asset. The same public CLI test passes in the final green after the narrow exception.
4. `restored-coverage-refusal-002.log` names all five actually absent historical diagnostic paths. No replacement archive is emitted.

## Claims that remain unavailable

The historical 392-input manifest and acceptance receipt were independently rehashed unchanged. The restored original archive still has only 387 matching canonical members and five omissions. The new packaging check neither reconstructs those files nor establishes later-source parity.

Privacy continues to depend on reviewed source/evidence content as well as path restrictions: a filename/hash is not a semantic secret detector. The archived checkpoint-assembly helper was not changed here. Any future full checkpoint must use this coverage option or an equivalent independent coverage gate, rather than relying solely on its own selected-member checksum list. No fresh project-wide gate, recovery upload, runtime availability or sustained-performance conclusion follows from this patch.

Exact checked evidence identities are preserved in `INDEPENDENT-REVIEW.json`.
