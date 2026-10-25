# Corrective checkpoint after hosted run 38584846100

Local validation passes for source commit `026f5630c9212f1e9896c4e4279b02c7f2d9e9ea`,
tree `18eded7b24a7ba8d534251b69fbb21642712835c`, based on public
[e1ea103f2d8c0d8091eb63acf11f240c1e350591](https://github.com/9hnm24bzvm-wq/nostr-manga/commit/e1ea103f2d8c0d8091eb63acf11f240c1e350591).
These two evidence-only files do not change the 1,058-input frozen source snapshot.

## Reviewed changes

- Ordinary upload admission now stages reads/hashes outside the shared write
  mutex, retains exact session/revision/owner guards and stale/freeze fences,
  and uses the guarded captured session for replacement identity/version.
  Ordinary import controls follow exact retained native labels; upload action
  event lifetimes are joined even when a click or action fails.
- Signed event comparisons use the closed seven-field semantic envelope while
  retaining raw transport digest checks, actual Rust serializer witness and
  genuine signing regressions. 44 finite downstream checks expose later failures.
- Busy Group/User control identity and 71 finite actor checks; reader title
  traversal and 37 checks; explicit home retry gestures and 28 checks. Original
  durable-result, consent, security, ownership and no-replay assertions remain.
- Compact-feed SQL batches selected parent/group projections within one
  statement. 83 historical migrations remain unchanged; exact PR28 additive 86
  prerequisite and new 87 are included. No timeout, pool, cross-request cache or
  visibility relaxation is introduced. Full PR28 runtime/role integration is
  outside this checkpoint.
- A hosted-only bounded native PostgreSQL regression now precedes browsers.
  Its closed nested preparation result is not-run, failed or passed. Browser
  fixtures require genuine native success with owned and PostgreSQL cleanup
  plus the validated finite native measurement. Raw native logs remain private;
  the public artifact allowlist stays preparation/phases/summary JSON.
- Byte-identical official sanitized evidence from the prior failed run is kept
  separately. Historical receipts and original Build19 plan are unchanged.

## Local verification and acceptance limits

All 28 Node CI suites passed 381 top-level cases, all 53 Python CI tests passed,
all 72 Rust library tests passed with zero ignored, and the offline locked Rust
all-target check passed. All 676 original frontend comparisons remain identical.
A fresh actual composed projection passed strict checking with 0 errors and 15
existing warnings, 110 transformed targets and 592 source maps. Both artifact
validations and all 10 negative controls passed. 42,805 protected dependency cache
entries were byte/metadata identical, with zero protected write attempts.
The first integrated strict attempt exposed four possible-undefined upload
references; the reviewed captured-session correction and fresh final projection
resolve those errors. Existing strict warnings and the Rust unusedgif warning
remain disclosed.

No native PostgreSQL, browser, hosted CI, installation or public publication was
performed for these local checks. The required real native regression remains
unrun; no query performance or improvement is measured. The future browser
scope is 39 phases/11 fresh batches/51 planned executions. Full-suite acceptance
and pixel parity remain false/unestablished. The
[prior run](https://github.com/9hnm24bzvm-wq/nostr-manga/actions/runs/38584846100)
failed with 10 unique passes, 10 failures and 19 pending phases. Its feed 503 was
classified query-canceled; that alone does not uniquely prove its root cause.

The receipt stores finite counts and output digests, not raw logs, signed URLs,
credentials, private keys or runtime databases. Dependency protection is an
in-process guard plus before/after inventories, not OS syscall enforcement.
