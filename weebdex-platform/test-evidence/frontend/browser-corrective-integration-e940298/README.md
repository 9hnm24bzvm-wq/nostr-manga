# Corrective checkpoint after hosted run 38615879103

Local source and strict checks pass for source commit
`f445a1b217fe7529856a88fc89377f74f23f72f2`, tree
`ba110a8f99c8620918aa56d7a0a48864b35a13e5`, based on public
[e94029800c1ffde6f1d7e90d36d121ef54c12651](https://github.com/9hnm24bzvm-wq/nostr-manga/commit/e94029800c1ffde6f1d7e90d36d121ef54c12651).
These two evidence-only files do not change the 1060-input frozen source snapshot.

## Reviewed changes and acceptance boundaries

- Upload candidates are created as the same reactive row used by queued mutations
  and rendered controls. The actual pinned Svelte compiler/runtime reproduces the
  former raw-row failure after awaited responses and checks corrected failure,
  success, replacement and late owner/switch/freeze outcomes against the real batch
  path. Services and the reduced DOM are explicit stand-ins, not full browser UI.
- Publication captures the owned review synchronously with `$state.snapshot` before
  clearing reactive state. Actual Svelte/runtime plus original outbox and finite
  structured-cloning IndexedDB stand-ins check rollback, exact bytes and CAS.
  Crypto, consent, source/recipient, fingerprint and concurrency guards remain.
- Upload metadata uses one-shot numbered mode and retained controls. Editorial
  requires native create 201, and reads private draft covers at the native route.
  Original cover 200 remains necessary with stricter original PNG identity,
  dimensions and positive safe-integer version proof; malformed 200 is rejected.
  Original assertions remain in order, with only the proven create 200→201 change.
- Ordinary Double-spread setup reopens page2 after selecting Double, so the later
  second-member/reload/quality assertions test their stated prerequisite. Entity
  pagination selects the actual `Page 1` accessible name. Existing assertions stay.
- Passive signed-conversation response observation adds no requests and joins its
  owned parser. Reader/entity/capacity and actor readiness have finer closed finite
  diagnostics. Actor readiness and capacity root causes remain unresolved; no
  timeout, retry budget, security, cleanup or success condition is relaxed.
- All additive inventories and actor observations remain closed. The source plan
  binds 16 current inputs; the actual composed manifest binds 110 targets/69 source
  inputs, with 601 repairs including 413 shared repairs. Canonical frontend source
  and the whole backend remain unchanged.

## Local verification

All 36 Node CI suites passed 546 top-level cases, all 56 Python CI tests passed,
and all 676 original frontend comparisons remain identical. A fresh actual composed
projection passed strict checking with 0 errors and 15 existing warnings. Both
artifact validations and all 10 negative controls passed, with 110 transformed
sources and 592 source maps. All 42805 protected cache entries remained identical
in bytes and metadata; no protected write attempt was recorded. The in-process
Node guard plus inventories is not OS syscall enforcement.

The first strict generation stopped before type checking with two unresolved
imports because this fresh worktree lacked its local Noble cache alias. Its
failed exit/output digests are preserved. Only a separate corrected fresh
`retry1` generation is claimed passing; no source checker or production dependency
was changed, and no package was installed.

### Fresh-worktree local preflight

Before using the local strict script, make this ignored alias point to the
already verified cache from this execution workspace:

```sh
PLATFORM=/workspace/scratch/3eafc9756ff3/browser-corrective-integration-e940298-worktree/weebdex-platform
VERIFIED_NOBLE_CACHE=/workspace/scratch/3eafc9756ff3/production-build-verification-3911a07/weebdex-platform/.build/local-library-noble-2.0.1
test -d "$VERIFIED_NOBLE_CACHE/node_modules/@noble/curves"
test -d "$VERIFIED_NOBLE_CACHE/node_modules/@noble/hashes"
# For a different new worktree, set PLATFORM to its own weebdex-platform path.
mkdir -p "$PLATFORM/.build"
ALIAS="$PLATFORM/.build/local-library-noble-2.0.1"
if [ -L "$ALIAS" ]; then
  test "$(readlink -f "$ALIAS")" = "$(readlink -f "$VERIFIED_NOBLE_CACHE")"
elif [ ! -e "$ALIAS" ]; then
  ln -s "$VERIFIED_NOBLE_CACHE" "$ALIAS"
else
  echo "Unexpected populated Noble alias path; inspect before proceeding" >&2
  exit 1
fi
```

Use the existing hash-pinned frontend and JSDOM cache roots for the source suites,
read-only dependency guards, and a fresh ignored projection output for each run.
The alias is a local prerequisite, not a package restore or cache write. Hosted CI
uses its existing genuine pinned restoration instead of this local alias recipe.

## Historical result and next run

The [prior run](https://github.com/9hnm24bzvm-wq/nostr-manga/actions/runs/38615879103)
failed: 15 unique phases passed, 10 failed and 14 remained pending; 26 of 36 actual
executions passed. Full-suite acceptance and pixel parity remain false/unproved.
Its exact three official sanitized JSON files and verification are preserved in
`hosted-browser-38615879103`. The
[official artifact metadata](https://api.github.com/repos/9hnm24bzvm-wq/nostr-manga/actions/artifacts/11921810806)
reports expiration at 2026-10-15T18:59:15Z.

That prior run's genuine disposable-PostgreSQL compact-feed regression passed
correctness, terminal/fixture cleanup and unchanged DB budgets. Its one fixed
fixture observed the old query succeed in 3450 ms and the batched first page in
1163 ms, with complete first-page equality, canonical samples, all 505 IDs, bilingual
506 and visibility-withdrawal checks. This is historical one-fixture evidence,
not a general performance claim or a new measurement.

Nine full native workflow/controller/launcher/supervisor/test files, the native
preparation validation function/import and the entire backend tree remain exact
bytes from the public base. New hosted CI must rerun that genuine native gate
before its 39-phase/11-batch/51-planned-execution browser suite. No new local Rust,
PostgreSQL, browser or hosted run was performed for this checkpoint. Full PR28
runtime/role-grant attestation remains separate. No publication or deployment is
claimed by these local checks.

The receipt contains finite facts and output digests, excluding raw logs, signed
download URLs, credentials, private keys and runtime databases. Historical evidence
and the original Build19 plan remain unchanged.
