# Browser E2E readiness, 2026-10-17

## Current result

The positive replacement journey remains **unrun**. Its explicitly approved
outside-command-sandbox invocation failed in the execution wrapper before Python
started. This is an execution-bootstrap failure, not an observed application or
browser failure. No runtime, application, or cleanup pass follows from it.

The reviewed runner and plan remain unchanged on the frozen
`8d963f3fd2aacaca75edba7d8a4f091ae4786aea` checkout. A new file-only validation
rehashes all 2,167 pinned inputs and both 432-file frontend trees, confirms the
frozen checkout is clean, checks the disk floor, and stops before importing the
process owner or allocating a fixture. The expected runtime and acceptance
directories remain absent. This validation is not a runtime retry.

The supported environment catalog currently exposes no connected computers or
saved coding environments. The documented cloud browser is available, but it
does not establish the exact supervisor-owned fixture, common loopback namespace,
request guard, dependency pins or lifecycle required here. It was not substituted
for the reviewed runner. No browser flags or security settings were changed.

Retained local evidence outside the checkout:

- `replacement-positive-browser-approved-blocked-resume-20261017.json`
- `replacement-positive-browser01-approved-bootstrap-failure-20261017.json`
- `replacement-positive-browser01-approved-bootstrap-independent-review-20261017.json`
- `browser-e2e-supported-recovery-file-only-20261017.json`

Existing approval covers the unchanged positive journey. Repeating its approval
question would not repair the execution bootstrap. Supported infrastructure
recovery is the next runtime dependency; restricted-path access or an alternate
execution route is not a recovery step.

## Completed source-level qualification

The new `test_public_upload_observer_boundaries.mjs` executes 31 finite stand-in
tests. It covers actual-status/native-terminal separation, duplicate terminals,
replay and nonce rejection, the 128-request/32-mutation/96-successful-read caps,
cookie/route/body fences, owner-only listener cleanup, privacy of receipt fields,
canonical event envelopes, reentrant/concurrent signing, the exact 64 KiB JSON
boundary, response ownership after failure, late-signer aborts and abort-aware
stream cleanup. Its fake verifier does not establish cryptographic validity.

The combined public-upload regression passes **303/303**, with no skipped or
cancelled tests. Run from `weebdex-platform`:

```sh
node --experimental-vm-modules --test tests/ci/test_public_upload*.mjs
```

The first combined invocation lacked `--experimental-vm-modules`; four cases
failed because the VM module constructors were unavailable. The original log is
retained. The corrected invocation, using the existing tests' required option,
passed. This is a test-command correction, not an application repair or a change
to Chromium flags. The retained receipt records both outcomes.

## Runtime acceptance checklist

After supported execution recovery, the sole runtime owner must perform fresh
same-scope revalidation before the exact approved call. If an input changed,
review its actual effect rather than silently substituting it.

1. Recheck the exact source/tree, runner/plan/review hashes, all dependency pins,
   complete frontend membership/modes/bytes, output absence and 4 GiB disk floor.
2. Start one new owned disposable fixture. Obtain its origins only from its new
   private ready receipt. Require real protocol readiness and the current-run
   complete operational HTTP receipt, including the genuine early-response413
   gate, before submitting a browser command.
3. Execute the real public identity key-import controls, the first session's
   begin/language/image/cancel-dismiss/confirmed-cancel sequence, and the second
   session's metadata, three images, reorder and removal.
4. Require exactly one real replacement POST with HTTP200 and native completion.
   Compare the independent before/after session: stable target ID and position,
   unchanged neighbours/draft/order/authority, actual version plus one, and
   freshly read stored original bytes matching the distinct replacement PNG.
5. Require a different document time origin after hard reload, real key reimport,
   unchanged durable metadata/order/hashes, decoded remaining images and actual
   new-document signed-preview GETs with HTTP200/native completion. Immediate
   acknowledged-local preview decode cannot stand in for these signed reads.
6. Commit once through the actual terms/approval controls. Require the genuine
   stored group event, exact chapter/publication event identity and ordered fresh
   original reads. Preserve accepted/pending projection status honestly.
7. Require all 21 natural mutations, zero application page errors, successful
   finalized runner diagnostics, actual context/browser/proxy closure, normal
   supervisor exit, verified descendant absence, removed synthetic keys and
   refused connections to all four exact owned loopback ports. Distinguish
   physical cleanup from complete lifecycle success.

## Broader coverage boundary

The current hosted scheduler contains 39 unique phases in 11 batches, totalling
51 scheduled executions with per-fixture Startup and deliberate repeats.
`public-upload-recovery` is a separate opt-in phase and is not in those batches.
A successful positive replacement run would therefore not establish a full
historical forty-case pass. Broader acceptance needs a reviewed, current-source
execution plan and actual results for its stated phase set.

Public chapter editing, post-decode concurrent revocation, injected lost-response
replacement reconciliation, readable catalogue projection and pixel parity are
not qualified by the new finite tests or this readiness review. Other workers'
new source changes need their own final-source integration and regression checks.
