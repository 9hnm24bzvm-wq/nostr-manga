# One dedicated native upload diagnostic

This diagnostic is not the combined application suite and does not change production upload behavior or observer success criteria. It uses synthetic loopback data, no user credentials/accounts, six isolated adapter-versus-bare-fetch native Chromium cases, and one closed fixed-enum/count report capped at 64 KiB. Browser sandboxing remains enabled. Expected negative controls remain labeled failures at the observer boundary, not silently accepted product behavior.

## One-run publication gate

The existing `Hosted browser E2E` workflow is reused. No additional workflow is introduced. Exactly the push commit message `diagnostic(native-upload): approved bounded run 20261020` selects its dedicated job and skips the combined job. Every other push and ordinary manual dispatch retains the original combined job behavior. There is no permanent paths-ignore exemption.

Before any dependency installation or browser execution, the dedicated job verifies the public repository/branch/push identity, non-forced event, exact prior commit `4294b711c3b90c9ec3c507435aabdfc59e28f0c6`, single parent, clean checkout and exact nine-file consumer comparison changed set. An unexpected predecessor or scope fails closed; it does not fall through to the combined suite. This is a one-publication admission, not standing authority to run future diagnostics.

## Bounds and evidence

The dedicated job has a 15-minute cap including setup; its browser step has five minutes. Each case is capped at 20 seconds including cleanup and pending-task settlement. The harness's 260-second allowance is externally owned by a 270-second descendant-owning supervisor with at most 15 seconds of cleanup. The supervisor inherits no GitHub token, user key or unneeded runner environment. Private setup/runner logs stay local and are not artifacts. Forced cleanup or surviving owned processes blocks report upload.

Only the freshly validated canonical `report.json` is uploaded, with one-day retention. No raw URLs, headers, IDs, bodies, error messages, screenshots, private logs or encrypted diagnostics are uploaded. All cases are synthetic and have no production network purpose. Dependency setup uses the existing exact lockfile's official Playwright package and pinned official Chromium revision.

Local tests validate the publication gate, ownership deadline and static workflow bounds without launching Chromium. The earlier denied local Chromium launch is not retried or bypassed. Hosted native outcomes must be read as diagnostic evidence, not application-suite acceptance.

## Causal limits

The actual current-upload adapter, transport, validation and multipart sources are served as stripped JavaScript modules. Signature verification is an explicit synthetic fixture adapter, not qualification of cryptographic signing. These synthetic cases do not qualify the application Svelte router or document navigation. Expected aborts remain strict-observer failures and cannot establish upload success. The actual_native_events field is derived from observations, not merely selecting native execution.

Chromium keeps its normal sandbox, uses a denying loopback proxy, and admits renderer requests only to the exact synthetic loopback origin. The diagnostic's event ordering must not be generalized to the prior application failure without matching evidence.

The initial publication was rejected at YAML admission before any job ran. Both job conditions now use folded scalars. A complete-file PyYAML parser regression validates the corrected document and rejects a byte-exact reconstruction of the initially published malformed workflow. Local qualification requires PyYAML; its absence is a failing dependency, not a skipped parser test.

## Adapter-versus-bare-fetch comparison

Six isolated direct cases compare the actual source-bound adapter against bare fetch without request interception: adapter completed 404, adapter delayed-body 404, and delayed-body 404 consumed by a stream reader with releaseLock, a stream reader without releaseLock, text(), and arrayBuffer(). Both stream-reader objects remain strongly referenced until the native terminal observation; changing garbage-collection lifetime is not the intended comparison. The report separates application completion from strict native terminal health. A consumed 404 is not sufficient to accept an aborted request.

All pre-cleanup work shares one cancellable work deadline, reserving time within the 20-second case cap for cleanup. Pending polls are explicitly cancelled and awaited before detaching the strict request observer. An ordinary case failure is retained and the overall result remains failed; subsequent cases may run only after all tasks settle and context cleanup is verified. Unsettled tasks, unverified cleanup or exhausted deadlines stop further cases. No production code or observer acceptance criteria change.

The native-terminal-observed ordering mark records when the harness informs the renderer that a terminal event was observed, after awaiting the application result. It does not establish the actual native event chronology relative to application body-read or return events. Native terminal counts and outcomes remain observations; this marker alone cannot prove cancellation causality.

Server finish/close fields are bounded event counts sampled before cleanup, not proof that the client successfully consumed the response. Native text() and arrayBuffer() consumption need not invoke the instrumented JavaScript reader hook; an absent body-eof mark does not prove an unread body. Each consumer separately validates the expected synthetic response bytes.
