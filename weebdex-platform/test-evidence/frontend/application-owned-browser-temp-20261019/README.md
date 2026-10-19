# Application browser batch temp-root proposal: focused finite qualification

Date: 2026-10-19 UTC.

Frozen source commit: 4f7d80c93d29f706f403104d15755a710b906094.
Frozen source tree: 7c56a3bcc04b6b4361ff7ca466dc993f39f53c53.
Base commit: e4942906d31acd3ec2ea5cc2006e00f58f2103ba.
Base tree: d7cb8076302bc27ec2e58586412dc7919cd0b966.

This evidence-only package retains the original focused local checks for that exact three-path source change. The manifest pins those source files and the receipt bytes.

## Result and limits

The focused Python suite collected 23 tests: 22 passed, zero failed, one skipped. The skipped AF_UNIX witness reaches the synthetic 83-byte old-base / 128-byte socket-path assertions, then reports permission denied at socket creation. Its retained skip text does not distinguish EPERM from EACCES. No long-path bind failure or short-path bind success was physically observed by this suite.

The source-parity check passed: the new helper's BrowserTemp/constants body matches reviewed preflight source c1a5793883c4ee523884f18010c29b6a89842c9e byte-for-byte, and all three changed Python files parse. The original fixture implementation, process owner, Node browser worker, preflight, sealer, workflow files and runtime pins were unchanged by this source commit.

These checks use finite local files, injected process boundaries and short owned Python stand-ins. The actual Fixture.spawn code path is exercised with Popen intercepted; the positive run_batch phase record is deliberately synthetic. A separate real Python child is registered and joined by the existing outer process owner before its temporary root is removed. No genuine browser, application fixture constructor, native service, dependency install, application journey, hosted workflow or publication was executed.

## Integration

The outer application-batch owner allocates one fresh short 0700 temp root and fsyncs an exclusively created 0600 private admission journal before supervisor Popen. TMPDIR is then overridden to that registered root. All fixture children inherit this TMPDIR through the fixture's unchanged environment allowlist; Node and Chromium inherit it as well. The application browser still requests sandboxing.

This is one root per batch, shared across that batch's phases, rather than a root per browser phase. Cleanup starts only after the supervisor and its complete owned descendant tree are physically stopped and ownership tracking is clear. The existing cleanup/phase projections remain authoritative: forced or uncertain teardown cannot promote a batch to passing.

The exact-root, ownership, no-symlink traversal, identity and bounded cleanup rules come from the proven preflight helper. The root is never a symlink shortcut and cleanup never searches or deletes global temp directories. Child symlinks and special entries are unlinked themselves without opening their targets. Cleanup is capped at five seconds within the existing 600-second teardown reserve, with the helper's 4096-entry / 32-level traversal bounds. Ownership handles remain held until the final private journal is persisted.

The private journal records admission/removal identity and observed entry/byte accounting. The byte count is teardown observation, not a newly claimed runtime disk quota. The private root path, file handles and journal identity are absent from public batch evidence. Admission refusals are mapped to the existing public RuntimeError category; cleanup uncertainty uses the existing cleanup-error and verified-stopped gates without expanding the public schema.

The finite controls cover long inherited TMPDIR override before Popen, registration and private-journal durability, actual fixture-to-Node environment forwarding and secret filtering, collision/nonreuse, symlink/inode/ownership refusals, FIFO unlinking, deadline refusal, live/unknown tree refusal, normal and forced batch outcomes, and cleanup/journal uncertainty.
