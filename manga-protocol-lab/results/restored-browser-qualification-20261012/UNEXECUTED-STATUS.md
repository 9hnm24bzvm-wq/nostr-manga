# Browser helper source freeze, 2026-10-12

All new source, generator outputs, verifier paths and test cases are UNEXECUTED.
No syntax/import/test/fixture/browser launch has run. Small stdlib file hashing
for this manifest does not constitute executing or qualifying the helper.

The npm installer session88480 has no verified terminal receipt after its deadline;
all new process and browser launches remain held by the parent. Do not infer
cleanup from elapsed time or inspect/signal a guessed host/namespace PID.
The earlier restored service smoke remains separately frozen and valid.

The two plans now agree on a45-minute controller maximum,24GiB sampled disk
floor,1GiB memory stop floor,32MiB private process-log cap and1GiB storage/staging.
The browser worker and protocol limits are unchanged. The probe emits a fresh
nonce. The independent verifier is a fixed local CLI, not an HTTP control route.
It is not yet qualified; public-proof negative seams remain a prerequisite
before accepting its result in the tiny real-service/browser case.

Initial child source hashes and the changed controller/PLAN/allowlist bytes are
preserved in revisions/001. Revision002 is preserved completely in revisions/002. This revision003 supersedes both earlier live source selections.
No production application, original benchmark, accepted binary or browser asset
was edited. This is a source candidate for static review, not launch clearance.

Next serial steps after the lifecycle hold is explicitly resolved: static review
corrections, the small actual CLI/HTTP/process test set including public-proof
negative cases, fresh cloud-browser reachability, then the four-record actual UI
roundtrip. Review that evidence before the30202 and500202 record cases.

## Static-review corrections, revision003

The controller now arms an absolute5-second main-thread request timer, capped
by the remaining owner lifetime, so a slow trickle cannot renew it. Timeout sets
the abort reason before interrupting HTTP I/O; normal ownership cleanup follows.
All timeout/owner-expiry exits are failed, even when cleanup succeeds.
Private-helper assertions were replaced by actual generator CLI and signer HTTP
checks. Proposed process cases send slow headers during the request deadline,
controller deadline and SIGTERM path. The verifier now restricts private key
reads to an unaliased owned .runtime-final/<run>/user.key via directory/file
NOFOLLOW descriptors, with directory/key ownership, mode and link checks.
A proposed real-service CLI pair accepts that valid identity at its owned path
and rejects the same valid identity copied outside it. Nothing has executed.
