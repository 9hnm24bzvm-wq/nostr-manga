# Short owned Chromium temporary directory: finite source qualification

Date: 2026-10-18 UTC.

Frozen source commit: c1a5793883c4ee523884f18010c29b6a89842c9e.
Frozen source tree: c75586bee4cdaaa41826736b5d0475b9a3ee373f.
Base commit: 76bfc2d9b4d978412613e6d4721adf4ff004d13a.
Base tree: 6e29ce41014db3938d87eb44d62f1f32626e46ff.

These are fresh local finite results for that exact frozen source. They do not inherit a lost or earlier evidence directory.

## Result

- Node: 39 tests passed, zero failed, zero skipped.
- Python: 58 tests collected, 57 passed, zero failed, one explicitly skipped.
- The skipped test reaches the synthetic 83-byte old-base / 128-byte socket-path assertions, then this local execution sandbox reports permission denied at AF_UNIX socket creation before bind. The retained skip text does not distinguish EPERM from EACCES. Neither a long-path bind failure nor a short-path bind success was physically observed here. The test remains enabled for an environment that permits owned AF_UNIX sockets.
- Syntax parsing, whitespace validation and the unchanged-file checks passed.
- Installed tools for these local fixtures: Node 24.19.0 and Python 3.12.14. This is not an execution result from the pinned hosted Python/Ubuntu/browser environment.

Commands, raw output and exact source-file hashes are retained in the adjacent text receipts.

## Isolated change

Only three source/test files changed in the frozen source commit:

- scripts/ci/run-sandbox-preflight.py
- tests/ci/test_sandbox_browser_tmp.py
- tests/ci/test_sandbox_seal_ready.py

The owner now creates one exclusive /tmp/sbp-<12 hex digits> directory with mode 0700 before admitting its child. Its generated base is 21 UTF-8 bytes, with an explicit 32-byte admission ceiling and a reserved 64-byte Chromium directory/socket suffix, strictly below Linux sun_path[108] including room for the terminating NUL. A pre-existing directory or symlink is never reused. Root/parent admission and teardown compare retained directory identities, reject symlink roots, check ownership and recheck the exact root and ancestor chain before deletion.

This directory is registered on Owner before launch, and the child environment must match it. Ownership handles stay held until the existing private owner journal is persisted. That private journal adds admission/removal, device/inode/uid identity, visited-entry and observed-byte accounting. Observed bytes are the larger of logical size and allocated blocks for each entry visited at teardown; this is not a newly claimed runtime disk quota. The former owner did not impose a browser-profile disk cap.

Cleanup runs only after process ownership/closure and pipe EOF have been verified, within the unchanged cleanup and total deadlines. Traversal has finite 4096-entry and 32-level bounds. It opens only owned directories using NOFOLLOW and directory-relative handles; child symlinks and special entries are unlinked themselves without opening or following their targets. It does not search or delete global temporary directories. An ownership, identity, traversal or cleanup uncertainty increments the existing tracking-error accounting, prevents a clean public result and prevents seal-ready. A temp admission refusal before any child exists still emits a new failed public record and cannot produce a receipt.

The finite controls cover collision/nonreuse, parent/root symlinks, root inode replacement, mode drift, foreign-owned descendants, child symlink targets, owned FIFO entries, deadline/entry/depth refusal, child-environment binding and end-to-end owned Python stand-ins. The stand-ins deliberately synthesize worker records solely to exercise owner wiring and terminal gates.

## Preserved scope and limits

The sandboxed Node worker, runtime pins, public schema, process RSS/birth limits, PID/birth/pidfd/subreaper ownership, EOF/log caps, fresh-artifact gates, workflow files and sealer CLI are unchanged. Private worker.log and seal-ready.json remain under the same fixed .build/sandbox-preflight location. The sealer requires a real owner child before it attempts to consume a worker log, so an unstarted temp-admission refusal cannot lose its new failed-record marker by trying to read a nonexistent log.

No actual browser, application fixture, dependency install, external network request, hosted workflow, publication or security-setting change was performed. These local fixtures establish neither hosted browser capability nor application acceptance.

A small read-only application-harness check found no existing short-temp ownership utility and no checkout-local Chromium TMPDIR assignment. run-hosted-browser.py and tests/runtime/browser_fixture.py forward inherited TMPDIR/TMP/TEMP; browser-acceptance.mjs inherits that environment while requesting sandboxing. The checkout-local browser-final-* directory belongs to the application fixture itself, not an explicit Chromium temp override. Those application harness files remain unchanged.
