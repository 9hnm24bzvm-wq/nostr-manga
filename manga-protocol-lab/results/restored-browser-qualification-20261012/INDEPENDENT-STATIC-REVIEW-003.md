# Independent static review: browser fixture revision 003

**No remaining static blocker identified for the planned controlled qualification. All execution remains held and unexecuted.** This review read source/diffs and verified file hashes only. It did not import the helpers, run tests, install packages, launch processes or interact with a browser.

All 12 current members (97,930 bytes) match freeze `d08074e198665bba32dd8920e0c88ff4d0f3ca675359998f38218fcc4acd1440`. All 12 revision-002 members are preserved with their original hashes under `revisions/002`.

- Controller SHA256: `0fbf64cfc04944b18cad8cdb64d060731addfe3d6bc59a8ad65e9113d0dcd553`.
- Verifier SHA256: `dc696b2e039efaca1428215f485aa7bc20ac89dcf3af5d8d8f966374b139fcb2`.

## Three initial findings addressed statically

1. The original synchronous HTTP handler had only an inactivity timeout: trickled headers/body could defer owner/resource/stop checks indefinitely. Revision003 wraps dispatch in an absolute SIGALRM deadline, bounded by both five seconds and remaining owner lifetime. It marks the abort before raising, so HTTPServer exception handling cannot silently restore a passing result, and cancels/restores its timer/handler in `finally`. Owner expiry now exits unsuccessfully. Actual request/owner-deadline and signal-during-trickle process cases are prepared but have not run.
2. Private `PureSeams` generator/validation assertions were removed. Their useful checks now use real generator CLI output/refusal and signing HTTP requests. This matches the intended operator/process/HTTP seams. Large-case fidelity and signed-public proof rejection still need their planned execution.
3. The verifier now enforces its claimed fixture-only signer path: unaliased `.runtime-final/<bounded-name>/user.key`, current ownership, private directory/file modes, dirfd/no-follow access, regular single-link bounded key bytes and public-identity equality. An actual valid-owned versus identical-outside-file pair is proposed and unexecuted.

## Scope and security consistency

The controller binds only literal loopback, serves exact manifest asset paths, checks Host and signing Origin/custom-header, and restricts signatures to the intended fixture event kinds and exact routes. No generic HTTP command route, direct IndexedDB injection or automatic-publication helper is present. The bridge supplies only NIP07 to the unchanged app; explicit consent and default-local behavior still require actual UI observation.

Generated keys/database/config/process logs remain private runtime material. Reviewed source contains no embedded key literal, and the safe-evidence list excludes runtime and unreviewed captures. Same-launch subreaper ancestry and identity-pinned signals avoid guessed host/service kills; terminal receipts expose remaining children and asset/node drift. These are controlled-host protections, not guarantees against SIGKILL or arbitrary hostile concurrent filesystem changes.

The independent verifier constructs expected records separately from the producer. It checks exact fields, counts, duplicates, order/hash buckets, signed service proofs, publisher signature, final head stability and bounded streamed export contents. It truthfully has no runtime pass yet. Public-proof negative CLI/HTTP cases remain an explicit prerequisite before accepting its public verdict.

The unknown npm execution lifecycle is unresolved. No source review releases that hold. After supported lifecycle resolution and explicit release, the next evidence must be the small controlled process/HTTP/verifier checks, actual browser-host reachability, then the four-family tiny UI roundtrip and terminal cleanup. Review those results before the30202/500202 cases. No full parity, native extension, mobile, power-loss or sustained-capacity claim follows from this review.

Exact checked hashes and qualifications: `INDEPENDENT-STATIC-REVIEW-003.json`.
