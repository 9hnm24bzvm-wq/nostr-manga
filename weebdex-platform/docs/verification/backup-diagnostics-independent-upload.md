# Backup failure diagnosis and independent upload qualification

This is a local test-harness proposal, not a demonstrated backup repair or a new hosted result.

## Established result

Hosted run 39371783371 tested source tree `7648422542f4404bceaca1facbf5a57424cec8f5`. Build preparation and the native PostgreSQL gate passed. All six feature checks were attempted: three search cases and signed chapter entry/refusal passed; the first backup case failed and its dependent replacement case could not establish a separate result. Upload was not attempted after the feature phase failed. Physical fixture teardown was verified. The retained sanitized report and available job log do not identify the failed backup operation.

## Closed backup diagnostics

The two backup checks now carry case-specific fixed `stage` and `category` values. Stages distinguish opening, initialization, fixture resolution, seed verification, export, preview/review, confirmation, durable comparison and reload. The replacement check additionally distinguishes its merge prerequisite and cancellation/replacement operations. Categories are `check`, `timeout`, `operation`, and `prerequisite`; success requires exactly `complete`/`none`. Unknown errors receive a fixed fallback. Exceptions still reach the existing private recorder unchanged; no raw exception, URL, owner, record, key or database value is added to public evidence. Python admission enforces exact case/order/status/stage/category fields, including the replacement-only prerequisite combination.

## Independent required upload fixture

Only the exact public feature/upload pair is split. Each phase receives a fresh owned fixture, process tree and browser-temp root. A failed feature case cannot supply database, key, browser-context or queue state to upload. Upload may proceed only after normal, unforced, verified physical cleanup, closed ports, removed fixture keys and zero owned processes. Uncertain or forced cleanup, interruption and exhausted budget still stop the schedule. A feature failure remains a failed phase, batch and selected scope even if upload subsequently passes.

The existing 1,750-second worst-case admission remains inside the unchanged 30-minute browser step and 140-minute job:

- Feature fixture: 150 seconds readiness + 180 seconds phase + 35 seconds observation + 300 seconds cleanup = 665 seconds.
- Upload fixture: 150 seconds readiness + 600 seconds phase + 35 seconds observation + 300 seconds cleanup = 1,085 seconds.
- Each cleanup envelope reserves 20 seconds for bounded fallback, temporary-root cleanup and observation. An overdue cleanup is not admitted as verified.

The complete selected pair is budgeted before its first fixture, and later admission counts elapsed time. Public evidence retains both fixture cleanup receipts and both phase verdicts. The maximum fixture-receipt count increases by one for this split only; the phase limits and success criteria remain unchanged. Workflow triggers, artifact retention and QA-preparation-only encryption scope are unchanged.

## Qualification boundary

Finite tests exercise actual diagnostic callbacks/projectors and scheduling control flow with owned lifecycle stand-ins, including old-source counterfactuals and unsafe-cleanup refusal. They do not establish browser behavior, native performance, backup success or resolution of the existing preview abort. Publication and a further hosted run require separate approval.
