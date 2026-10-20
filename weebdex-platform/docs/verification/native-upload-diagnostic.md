# One dedicated native upload diagnostic

This diagnostic is not the combined application suite and does not change production upload behavior or observer success criteria. It uses synthetic loopback data, no user credentials/accounts, eight finite native Chromium cases, and one closed fixed-enum/count report capped at 64 KiB. Browser sandboxing remains enabled. Expected negative controls remain labeled failures at the observer boundary, not silently accepted product behavior.

## One-run publication gate

The existing `Hosted browser E2E` workflow is reused. No additional workflow is introduced. Exactly the push commit message `diagnostic(native-upload): approved bounded run 20261020` selects its dedicated job and skips the combined job. Every other push and ordinary manual dispatch retains the original combined job behavior. There is no permanent paths-ignore exemption.

Before any dependency installation or browser execution, the dedicated job verifies the public repository/branch/push identity, non-forced event, exact prior commit `7ee9ede26b45b16ef3b7b47220b30e924a7dbec9`, single parent, clean checkout and exact 15-file changed set. An unexpected predecessor or scope fails closed; it does not fall through to the combined suite. This is a one-publication admission, not standing authority to run future diagnostics.

## Bounds and evidence

The dedicated job has a 15-minute cap including setup; its browser step has five minutes. Each case is capped at 30 seconds. The harness's 260-second allowance is externally owned by a 270-second descendant-owning supervisor with at most 15 seconds of cleanup. The supervisor inherits no GitHub token, user key or unneeded runner environment. Private setup/runner logs stay local and are not artifacts. Forced cleanup or surviving owned processes blocks report upload.

Only the freshly validated canonical `report.json` is uploaded, with one-day retention. No raw URLs, headers, IDs, bodies, error messages, screenshots, private logs or encrypted diagnostics are uploaded. All cases are synthetic and have no production network purpose. Dependency setup uses the existing exact lockfile's official Playwright package and pinned official Chromium revision.

Local tests validate the publication gate, ownership deadline and static workflow bounds without launching Chromium. The earlier denied local Chromium launch is not retried or bypassed. Hosted native outcomes must be read as diagnostic evidence, not application-suite acceptance.

## Causal limits

The actual current-upload adapter, transport, validation and multipart sources are served as stripped JavaScript modules. Signature verification is an explicit synthetic fixture adapter, not qualification of cryptographic signing. Same-document history uses pushState and back rather than the Svelte router. A destroyed old-document ledger is unavailable, not empty. Expected aborts remain strict-observer failures and cannot establish upload success. The actual_native_events field is derived from observations, not merely selecting native execution.

Chromium keeps its normal sandbox, uses a denying loopback proxy, and admits renderer requests only to the exact synthetic loopback origin. The diagnostic's event ordering must not be generalized to the prior application failure without matching evidence.
