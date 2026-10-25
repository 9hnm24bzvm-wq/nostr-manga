# Nullable chapter and cover edits

The pinned MangaDex OpenAPI 5.13.1 schema (SHA-256
`004031ccdfc4b66727dd1f85d09b5300cf60235bbab2de6f6ea4816094bd6d33`)
permits null for `ChapterRequest.title`, `volume`, `chapter`, and
`CoverEdit.description` and `locale`. This repair implements those explicit
clears. It does not claim complete provider-schema conformance.

## Storage and read semantics

- An omitted field does not update its column. A checked string retains the
  existing validation and trimming behavior. Explicit null is a clear, not an
  omitted field and not an ignored value.
- Chapter title, volume and chapter columns, and cover description, are nullable:
  clear writes SQL NULL. Their existing public DTOs expose JSON null after both
  mutation and subsequent GET.
- Cover language is NOT NULL in the existing schema. Explicit `locale:null`
  writes the empty-string unspecified-language sentinel; the existing public
  DTO already maps this sentinel to `locale:null`. Non-null locale strings still
  require a supported language; an empty locale string is still rejected.
- Cover `volume:null` keeps its pre-existing empty-string storage convention and
  JSON-null public projection. No schema migration or default-language change is
  needed. Chapter translatedLanguage remains non-nullable.

An opt-in tri-state validator distinguishes omitted, null and string values;
ordinary `validation::string` remains strict. Presence-aware SQL replaces
COALESCE only for the fields requiring real clears. The shared transaction
writers serve both native JSON and signed public adapters. Creation/upload
contracts are unchanged.

Version checks, account-intent validation, authority and parent/source visibility
checks, immutable identity/media fields, and transaction-owned audit behavior
are unchanged. Protocol-backed chapter edits remain instance-local overlays;
source metadata and signed events are not rewritten, including on reimport.

## Verification

[Sanitized evidence](../test-evidence/nullable-edits-20261022/verification.json)
associates exact source and test binary hashes with these runs:

- Library tests: 148 passed, 2 existing ignored. The focused nullable subset's
  8 passes overlap that library run, rather than adding eight distinct cases.
- Four separately owned guarded HTTP/database lifecycles passed: comprehensive
  nullable semantics, chapter-only clear, cover-only clear, and the existing
  chapter edit/claim/removal authority regression.
- Comprehensive coverage includes real transaction rollback, per-field clears,
  omission before/after clear, restoration, mutation/GET agreement, stale version,
  wrong account, missing authority, wrong resource scope, hidden parent, no
  refused-write audit mutation, and signed event/source-metadata preservation
  through reimport.
- Original production source at `5cef929` plus test-only additions failed the two
  new normalizer assertions as expected. Its separate HTTP controls returned
  400 Invalid title and 400 Invalid description instead of 200; the transaction
  control rejected the null title. These were real compiled/runtime controls,
  not source-pattern checks or missing-function compile failures.
- All seven guarded lifecycles verified stopped PostgreSQL, closed ports, no
  remaining or unconfirmed owned processes, no cleanup errors and no forced
  shutdown. Raw logs and private runtime material are excluded.

Reproduce with Rust 1.90.0, locked/offline dependencies, two build jobs,
`CARGO_INCREMENTAL=0`, and dev/test debug information disabled. Build
`--lib` and `--test public_chapter_writes`; run each ignored integration test
separately through `scripts/run-auth-tests.sh` with an owned disposable database
and the pinned native fixture tools. The guard does not reset its database
between test cases, so do not run these fixtures together in one lifecycle.

Existing wider local length allowances (chapter/cover numbering up to 32,
cover description up to 50,000) and optional cover volume on edits are outside
this repair. Upstream's length-8 numbering, length-512 cover description and
required CoverEdit.volume remain separate compatibility differences. No
primaryCover, externalUrl, scheduling, browser or public deployment work is
included.
