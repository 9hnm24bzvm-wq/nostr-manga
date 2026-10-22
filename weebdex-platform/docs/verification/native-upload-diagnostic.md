# One dedicated native upload timing diagnostic

This diagnostic is not the combined application suite and does not change production upload behavior or strict observer success criteria. It uses synthetic loopback data, no user credentials/accounts, six isolated native Chromium cases, and one closed fixed-enum/count report capped at 64 KiB. Browser sandboxing remains enabled. An application-level consumed 404 never converts a native abort into success.

## One-run publication gate

The existing `Hosted browser E2E` workflow is reused. No additional workflow is introduced. Exactly the push commit message `diagnostic(native-upload): approved bounded timing run 20261022` selects its dedicated job and skips the combined job. Every other push and ordinary manual dispatch retains the original combined job behavior. There is no permanent paths-ignore exemption.

Before dependency installation or browser execution, the dedicated job verifies the public repository/branch/push identity, non-forced event, exact prior commit `b9389d59c978dd1bd5f5c66163a61bd24cc7849d`, single parent, clean checkout and exact ten-file timing comparison changed set. An unexpected predecessor or scope fails closed; it does not fall through to the combined suite. This is one-publication admission, not standing authority for future diagnostics. Publication must reverify the remote predecessor and include only the reviewed diagnostic diff, not unrelated feature work.

## Bounds and evidence

The dedicated job has a 15-minute cap including setup; its browser step has five minutes. Each case is capped at 20 seconds including cleanup and pending-task settlement. The harness's 260-second allowance is externally owned by a 270-second descendant-owning supervisor with at most 15 seconds of cleanup. The supervisor inherits no GitHub token, user key or unneeded runner environment. Private setup/runner logs stay local and are not artifacts. Forced cleanup or surviving owned processes blocks report upload.

Only the freshly validated canonical schema-2 `report.json` is uploaded, with one-day retention. No raw URLs, headers, IDs, bodies, error messages, screenshots, private logs or encrypted diagnostics are uploaded. Dependency setup uses the existing exact lockfile's official Playwright package and pinned official Chromium revision. Text and arrayBuffer are synthetic 18-byte controls only, never replacements for the production bounded streaming reader.

The local Chromium socket launch was previously denied and is not retried or bypassed. Local tests exercise real source modules through finite in-memory adapters and do not establish native browser outcomes. The full YAML file is parsed with required PyYAML, and a SHA-256-pinned reconstruction of the original malformed publication remains a negative parser control. Ownership cleanup, exact publication scope and workflow association are independently tested.

## Discriminating timing comparison

The prior six-consumer native run established that delayed adapter and bare stream consumers can report EOF and an application result while Chromium reports abort, with no JavaScript abort/cancel. Retaining the reader through terminal observation did not resolve it. Text finished natively but its aggregate observer was unhealthy; arrayBuffer passed. These observations do not establish the browser cause or a production repair.

This comparison changes one timing dimension:

1. Actual source-bound adapter, immediate 404.
2. Actual source-bound adapter, delayed-body 404.
3. QA-only task-yield adapter, immediate 404.
4. QA-only task-yield adapter, delayed-body 404.
5. Bare text(), delayed-body 404.
6. Bare arrayBuffer(), delayed-body 404.

The QA-only transport is generated in memory from the exact production source with SHA-256 `7093cdee6a1eff833447f0f4c488e52569a74e8df8e083e46f516368c03788de`. After each successfully bounded/copied chunk, it inserts one cancellable zero-delay task before the next read. The existing guarded operation retains identity, cancellation and deadline checkpoints. Its task timer and abort listener are removed on completion or cancellation. No request option, hard cap, reader cleanup, parsing or production file changes. Account discovery uses the untouched source in every case, and the owner captures the selected transport only after the case mode is fixed.

Actual-source local controls prove the original reader has no task yield; the QA variant does, including through the complete adapter graph. Both variants retain body caps with absent, understated, malformed and oversized Content-Length, accept valid chunked JSON at the exact limit, and reject overflow. Abort, deadline and identity changes during the task prevent another read and settle timers. These are regression controls, not claims about native Chromium scheduling.

## Strict observer projection

The unchanged strict request observer remains authoritative. Its exact existing failure enums are projected as `signed_request_invalid`, `duplicate_native_terminal`, and `signed_native_failed`. Fixed routes `account-context`, `current-upload`, and `other-upload` expose request/validated/final-state counts plus passive native response and terminal event counts. Duplicate terminals remain failures; they are counted, not normalized away. Unknown fields, arbitrary error text, unrecognized enums and excessive counts fail report validation.

Success requires both expected signed reads to finish with exactly their expected statuses, no native failures or duplicate terminals, a healthy strict observer, missing-current-upload application result, no refusal/mutation, settled work and verified context cleanup. The new projection can distinguish an earlier account failure from a current-upload finish without weakening health checks. Ordinary assertion failures remain failures; later cases run only after prior work and context cleanup are verified. Infrastructure failures stop subsequent cases.

## Causal limits

The actual upload adapter, transport, validation and multipart sources are served as stripped JavaScript modules. Signature verification is an explicit synthetic fixture adapter, not cryptographic signing qualification. These cases do not qualify Svelte routing or document navigation. `actual_native_events` is derived from observations, not merely native execution selection.

Chromium retains its normal sandbox, denying loopback proxy, and renderer admission only to the exact synthetic origin. No request interception, local browser fallback or production service is used. The native-terminal-observed ordering mark records when the harness informs the renderer after waiting for the application result and both native terminals; it does not establish native-versus-JavaScript chronology. Server finish/close counts are sampled before cleanup and do not prove successful client consumption. Built-in text/arrayBuffer need not call the JavaScript reader hook, so absent body-eof instrumentation is not evidence of an unread body.
