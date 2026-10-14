# Startup readiness and runner exit follow-up

Hosted diagnostic [38498307038](https://github.com/9hnm24bzvm-wq/nostr-manga/actions/runs/38498307038)
ran `d62c0e8b3db57d3ee024676e8bdec67f425abadd`, tree
`f016ada9e111b15e3ec919e48bf5a242695e35c2`. It failed: 21 actual executions,
three uniquely passing phases, nine failed phases, and 26 pending phases.
The exact three-JSON artifact ZIP was 4,911 bytes, SHA-256
`2a8dba3b56ad82765ce13f4171420abf1611e82cff7f877ce8bfea4b9180c007`.
Its closed schemas, frozen-source snapshot, recomputed summary, and all ten
physical teardowns were verified. Acceptance lifecycles did not pass.

The `ui-home` startup failed at the original 1,200 ms seeded-title snapshot.
The read-only bounded probe then observed the actual expected heading,
meaningful body, correct route, and ready document. Both document and seeded
title API returned 200. This proves that particular fixed-time snapshot raced
the real render. The original result remains failed.

The next generation replaces only that seeded-title fixed sleep with a
condition-based wait, bounded across all renderer calls to seven seconds.
It requires the known seeded route, visible exact `Fixture Adventure` heading,
ready document, and meaningful body, then verifies those conditions again
before the ordinary snapshot. Permanent emptiness, wrong/hidden headings,
wrong route, renderer failure, and deadline exhaustion still fail. The HTTP,
meaningful-content, JavaScript-error, CSP, and missing-asset assertions remain.
The old read-only probe still cannot change an already captured snapshot.

Separately, `signed-a` startup and `ui11-device-navigation` saved all outer
steps as passed, but their owned Node processes exited nonzero. Those phases
remain failed. The previous projection cannot identify the exit code or
whether report finalization was reached. No cause is inferred from that gap.

A passive `uncaughtExceptionMonitor` and synchronous process-exit observation
now record fixed stage/category fields and bounded exit codes. No
`uncaughtException` or `unhandledRejection` handler is installed, so Node's
original fatal behavior remains. Exception messages, stacks, URLs, passwords,
tokens, page contents, and runtime records are not added to public evidence.
The public projection also records final-report presence and the supervisor's
actual exit code. A pass additionally requires consistent finalized, normal
exit evidence. Nonzero/signal exits, missing diagnostics, fatal observations,
and unfinished finalization cannot be promoted by passed step rows.

Owned deterministic tests use explicit browser stand-ins and actual short-lived
Node child processes. They test deadlines, expected-content predicates,
unchanged fatal exits, privacy projection, and forged/missing public fields.
These are harness proofs, not additional application or full-suite passes.
The full suite and all fixture lifecycle gates still require a fresh genuine
hosted run. Browser pins, permissions, batch selection, and teardown are unchanged.
