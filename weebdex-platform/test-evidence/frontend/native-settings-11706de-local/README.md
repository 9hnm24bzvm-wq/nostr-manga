# Native Settings UI pass and retained compact-feed failure

Exact source `11706deb4dce8ae374e05e878e7f06f0ecb9bae4`, tree `ef3034fa06a80b8ff096c9a7e5df6ed40e00285d`. This actual isolated sandbox-enabled Chromium run used fresh offline/locked native binaries and the pinned unchanged 425-file frontend.

Startup passed 19 checks. All six local-library steps passed, including anonymous native Settings tabs, desktop/mobile route placement, actual scoped local saves/imports and account/device controls. The visual step emitted 16 synthetic captures and 20 geometry/hit-test observations, including 12 native task-dialog framing/dismissal checks. Six specifically named Settings/context captures were independently pixel-inspected in `visual-review.json`. Dialog pixels were not separately captured.

The next compact-feed phase failed `compact-feed-unique-sequential-pages`. Its preceding fixed gates and genuine native HTTP200 were observed; no database failure was reported. The distinction between duplicate requests and observer-measured concurrency was not saved before the assertion threw, so this evidence cannot establish the root cause. Signed-social, signed-context, device-navigation and device-progress phases did not run in this batch. The overall selected scope failed. No full-suite or whole-pixel-parity pass is claimed.

Final owned teardown closed all four ports, removed keys, left zero owned processes and used no forced cleanup. Lifecycle failure and nonzero supervisor exit remain preserved. A separate independently scoped run, if performed, must carry its own exact result; it cannot erase this compact failure.

Only sanitized source-bound JSON, verification and disposable synthetic screenshots are included. Raw URLs, logs, supervisor details, request bodies, credentials, keys and runtime databases are excluded. Historical failed first attempt and earlier evidence remain separate and unchanged.
