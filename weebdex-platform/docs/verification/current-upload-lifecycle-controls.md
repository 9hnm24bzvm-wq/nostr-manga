# Current-upload 404 and cancellation controls

Hosted run `39460130253` verified all six feature cases, including both backup cases after the reactive identity repair. The independent upload failed before mutations, at `group-trigger`, with a signed current-upload GET whose headers reported 404 and whose native terminal was aborted. Its producer-refusal snapshot was empty and correlation incomplete. That report does not establish why cancellation happened.

## Source-bound local controls

`test_current_upload_lifecycle.mjs` composes the actual full upload adapter, actual signed transport and actual strict browser observer. The existing adapter/transport fixture slices are hash-pinned; signer/service proof, storage, fetch streams and browser-native terminal events are explicit finite adapters. Ten controls include:

- A fully consumed valid fresh 404 returns no current upload and does not abort.
- Owner disposal, registered destruction, or selected-identity change before body consumption fail at the next transport checkpoint, with a producer refusal and cleanup abort. Disposing an owner is not itself an immediate controller abort.
- Disposal after successful completion does not cancel the completed read.
- Native body interruption and malformed JSON remain failures, not “no current upload.”
- The network-terminal schedule and application body consumption are separate. If native completion precedes consumption, later owner invalidation cannot retroactively turn it into native requestfailed. Both pending-native and already-finished schedules are tested.
- An unconditional-abort counterfactual is detected even after a valid 404, and the exact historical transport also preserves normal completed404 behavior.

These are not real Chromium navigation or native network-event results. The selected terminal schedules demonstrate implications, not the hosted event schedule. A native terminal may precede application consumption; “reader done” is not asserted to be the universal requestfinished boundary. The observer remains strict and unchanged.

## Native execution limitation and next discriminating check

An owned local loopback/installed-Playwright probe found Chromium installed but its launch failed at `process_singleton_posix.cc` with `socket() failed: Operation not permitted`. Chromium exited with SIGABRT; Playwright reported exit and temporary-directory cleanup, the loopback server closed in finally, and a subsequent process census found no remaining probe/Chromium process. No alternate route or sandbox bypass was attempted. No browser navigation experiment executed.

The key-import driver performs two history returns and checks for the Upload Chapter heading before the group action; that readiness does not assert native-request terminal completion. This is a source-backed timing seam, not proof that navigation caused the abort. The finite owner-invalidated cases produce a refusal record, unlike the empty hosted snapshot, so they do not explain that missing correlation by themselves.

The next useful experiment needs an environment where native Chromium can launch: run the actual current-upload transport with controlled fresh404 headers/body timing, perform the same history/owner transitions, and observe native requestfinished/requestfailed together with the bounded producer-refusal ledger in each document. Include same-document history versus document replacement and completed-response controls. Do not ignore aborts or weaken account/fence/current-session rules. No product repair is justified by the present controls alone.

Only tests and documentation are changed. No production, report schema, observer success criterion, publication, or hosted run is changed or performed. The hosted backup success remains valid; upload completion remains unverified.
