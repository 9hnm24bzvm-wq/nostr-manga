# Current reconstruction handoff

## What this contains

Rust/PostgreSQL instance backend, strfry integration, local-first browser library client, operator tooling, seam tests, and explicit recovery/feature evidence. Use [rebuild/ACCEPTANCE-MATRIX.md](rebuild/ACCEPTANCE-MATRIX.md) for executable coverage and remaining gaps. The later lost workspace's tests are not current acceptance.

## Build and verify

Follow [build.md](build.md), using the pinned [toolchain](rebuild/TOOLCHAIN.md). On a prepared environment run `bash scripts/test.sh`. This is the canonical current-runtime gate; `scripts/deep-test.sh` delegates to it. `scripts/test-postgres.sh` is the narrower real PostgreSQL/strfry suite. The browser README distinguishes model, actual-browser and backend interoperability evidence.

The old split demo/release instructions are preserved under history. Do not assume an old runtime bundle matches reconstructed source. A final source/release artifact must identify its exact hashes and test results before use. Fixture configurations and keys are for isolated loopback tests only.

## Recovery and operation

- [Verified encrypted checkpoints](rebuild/CHECKPOINTS.json): recovery passwords were delivered privately, never in source
- [PostgreSQL backup/restore](postgres-backup-restore.md)
- [Group-key backup/recovery](group-key-backup-recovery.md)
- [Managed original serving](managed-public-serving.md)
- [Storage audit and repair](storage-audit-repair.md)
- [Security and deployment boundary](security-review.md)

Each archive is uploaded, downloaded, decrypted and manifest-checked. A recoverable partial checkpoint is not a claim of full software acceptance. Restore instructions must be tested against the exact final artifact.

## Deliberate boundaries

The functional library browser is included; a full MangaDex-style catalogue/reader/social/moderation website remains a separate phase. Instance deletion policy cannot promise erasure of public copies. Group hosting gives its operator key custody. Independent security review, production deployment, 100TB storage behavior and requested monthly traffic targets remain unqualified.
