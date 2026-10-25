# Historical native Settings browser attempt

Exact source `4fe4ffa153a26a0e40b6024bb781ed7eb6012af3`, tree `784ac9b9dcbd3b32128ff956e3db2b43469e3593`. This is an actual isolated, sandbox-enabled local Chromium run against freshly offline/locked-built native binaries and the pinned frontend. It is a failed attempt, not acceptance of the current design.

Startup passed all 19 checks. The local-library phase executed six steps and failed steps 0, 2 and 4. Its first failure was a strict locator matching the legitimate reader-header X and the mobile Sheet primitive X. The later same-phase failures are retained in the closed diagnostics. The subsequent compact, signed-social, signed-context, device-navigation and device-progress phases were not run.

Eight desktop and four initial mobile synthetic screenshots are preserved, with exact bytes and source/frontend/native bindings. Native Settings desktop mains and the initial mobile reader were independently inspected; mobile Settings and the remaining layouts were not reached in this attempt. Task dialogs have actual geometry/hit-test observations, not separate pixel captures. No full pixel parity is claimed.

Final teardown verified all four owned ports closed, keys removed, no owned processes, and no forced cleanup. The failed lifecycle and nonzero supervisor result remain failures. No clean teardown is counted as a passing test.

A separately reviewed test-only correction selects the exact native Sidebar.Header close control, retaining the hidden-Manage and dialog-dismissal assertions. It is source commit `52ea70ecbf330afd069446521f9a062875c34df5`, integrated as `7261ff249c24d9eb43a416e074452f1e9544cefd`; the fresh snapshot is `11706deb4dce8ae374e05e878e7f06f0ecb9bae4`. This evidence does not claim its rerun passed.

Only sanitized JSON and disposable synthetic captures are included. Raw logs, supervisor reports, URLs, request bodies, credentials, keys and runtime databases are excluded. `verification.json` records independent source-bound schema/projection, artifact, cleanup and screenshot verification.
