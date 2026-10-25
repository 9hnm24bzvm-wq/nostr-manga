# One dedicated bounded pipe-ownership comparison

This is a candidate qualification, not an established production fix or the combined application suite. Production upload, adapter business authority and the strict native observer stay byte-for-byte unchanged. The candidate is generated in memory from the exact production transport and changes only body consumption/cleanup. Synthetic loopback cases use no user credentials/accounts. The only public evidence is one closed fixed-enum/count JSON report capped at 64 KiB, retained for one day.

## Exact one-publication gate

The existing `Hosted browser E2E` workflow is reused. Only the push marker `diagnostic(native-upload): approved bounded pipe run 20261022` selects the dedicated job and skips the combined job. Ordinary pushes and manual dispatch keep their original behavior; no new workflow or permanent path exemption is introduced.

Before installing tools or starting Chromium, the dedicated job checks the public repository/branch/push identity, non-forced event, exact public predecessor `1b35d7a6a1e5247931b23183f9c8368483d9d511`, single parent, clean checkout and exact twelve-file changed set. An unexpected predecessor/scope fails closed without falling through to combined tests. Publication must independently reverify the remote predecessor and include only this reviewed diff. This is one-publication admission, not standing authority for more diagnostic runs.

The full YAML is parsed with required PyYAML. The exact original malformed published workflow remains a SHA-256-pinned negative parser control. Workflow restoration independently pins the original combined workflow bytes. Action SHAs, Node version, exact lockfile, official Playwright package and pinned official Chromium revision are unchanged. Local Chromium/socket launch remains denied and is not retried or bypassed.

## What prior evidence did and did not show

Timing run 39750660220 observed native abort for the original immediate/delayed 404 adapter and the task-yield immediate 404 adapter. The task-yield delayed 404 adapter and both synthetic built-in controls passed. The signed account-context read finished in all six cases. No instrumented JavaScript abort/cancel or duplicate terminal was seen; cleanup settled. This supports testing a different EOF/reader owner, but proves neither cause nor a general repair. Adding a task yield alone was not a general fix and is not retained in this candidate.

## Bounded product candidate

`native-upload-pipe-candidate.mjs` checks complete production SHA-256 `7093cdee6a1eff833447f0f4c488e52569a74e8df8e083e46f516368c03788de` and exactly replaces three source seams. Reversal must reconstruct those exact complete production bytes. The QA ring is the only separately allowed source transformation.

The candidate pipes the actual response body to a `WritableStream`:

- `pipeTo` owns the reader/lock through EOF. Its promise is awaited through the existing guarded operation; the actual fetch AbortSignal is also passed to the pipe. No timeout-only `Promise.race` is substituted for cancellation.
- Every sink write checks captured owner/signer/view authority and the absolute deadline before copying, rejects a wrong chunk type or actual bytes above the remaining hard cap, then rechecks authority. Sink close and guarded completion also check authority.
- A single backing array is allocated lazily at the existing response cap; the final exact-size array is at most that cap. Transport-owned response copies total at most twice the existing cap, with constant per-fragment bookkeeping. No unbounded list of empty/tiny fragment objects accumulates. This bound excludes native browser buffering and the already-existing bounded decoded JSON object.
- Content-Length is only an early refusal signal, never permission to trust an unbounded body. Existing limits remain JSON 1 MiB and image 32 MiB, with smaller caller limits honored.
- Failed or incomplete pipes are cancelled through the same controller. Responses refused before a pipe starts are explicitly discarded. Fully consumed successful requests still never call abort.
- Exact signed bytes, request options, pre-send fences, headers/media/JSON validation, known-status errors and no-replay behavior remain unchanged. No `text()`/`arrayBuffer()` is used on untrusted production responses.

This stays QA-only until independent review and decisive native evidence. Production promotion would require the identical reviewed source change plus the full affected application regression controls. A passing synthetic comparison does not qualify Svelte navigation, real signatures, deployment or the whole application.

## Six discriminating cases

1. Actual original adapter, immediate 404.
2. Actual original adapter, delayed-body 404.
3. Exact-source-bound pipe candidate adapter, immediate 404.
4. Exact-source-bound pipe candidate adapter, delayed-body 404.
5. Bare pipe to an explicitly capped 18-byte sink, delayed-body 404.
6. Bare arrayBuffer, synthetic fixed 18-byte response only, delayed-body 404.

Account discovery remains the untouched original transport for every case. The selected transport is captured only after case mode selection and before creating the owner. Each case gets a fresh browser context; the sole current-upload dispatch uses identical signed URL, method, headers and fetch options. Baseline failures stay failures, never normalized into comparison success. A useful repair signal requires both candidate adapter cases to satisfy all strict success requirements; bare-pipe success alone is insufficient. If both candidate cases pass while original cases fail, that supports this candidate under these exact timings but still does not prove the Chromium root cause or broad product reliability.

Passive hooks preserve receiver identity and original promise identity. `pipe-started`, `pipe-resolved` and `pipe-rejected` are fixed event counters, not native success claims. Native pipe internals need not invoke JavaScript `getReader`/`read` hooks, so missing `body-eof` is not evidence of missing consumption. Test-owned bare response/controller references remain live through terminal observation. `native-terminal-observed` is marked after both the application result and native terminal wait; it cannot establish native-versus-JavaScript chronology.

## Finite controls and hard execution bounds

Local tests run the exact candidate through actual in-memory WHATWG streams and the full source-bound adapter graph. They cover exact caps, absent/understated/malformed/oversized Content-Length, overflow before copying, zero budget/204, wrong chunk type, 2,177 empty/tiny fragments, pending-read abort/deadline, owner/signer invalidation before write and at EOF, stream errors, malformed JSON/UTF-8, preserved known status, and actual pipe lock/cancellation settlement. Deliberately broken cap and missing-AbortSignal counterfactuals fail those acceptance conditions; even the intentionally stalled negative control is explicitly released and settled. The untouched source is the allocation/ownership comparison, not an invented approximation.

The dedicated job is capped at 15 minutes including setup, browser step at five minutes, and each case at 20 seconds including verified cleanup. A 270-second descendant-owning supervisor bounds the 260-second harness with at most 15 seconds cleanup. It inherits no GitHub token, signer key or unneeded runner environment. Forced cleanup or surviving owned processes blocks artifact upload. Private setup/runtime logs stay local.

Normal Chromium sandboxing, exact-origin denying loopback proxy and restrictive CSP remain unchanged. No request interception, production network, user account/key or local-browser fallback is allowed. Guarded abort can settle the application request before native pipe cancellation settles. Therefore cleanup separately waits for pipe-started to equal resolved plus rejected, within the original case deadline and before observer detachment. The application work and both owned polling scopes must also settle. A delayed-cancel finite control proves eventual cancellation, pipe rejection, lock release and absence of unhandled rejection. Pending, truncated or unavailable pipe accounting cannot claim settled work. Unsafe pipe cleanup retains the observer through owned context/browser shutdown and stops later cases as an infrastructure failure. Ordinary assertion failures remain failures; the next case starts only after settled work and verified context closure.

## Strict observer and evidence contract

The strict request observer is unchanged and authoritative. Fixed account-context/current-upload/other-upload route counts show validation, response status and terminal outcomes. Known strict failure enums and duplicate terminal counts remain failures. No raw URLs, IDs, headers, bodies, error text, screenshots or logs enter evidence. Unknown fields/enums or excessive counts fail validation.

An observed case requires both expected signed reads to finish natively exactly once with expected statuses, no native failure/duplicate, healthy strict observer, missing-current-upload application result, zero refusal/mutation, settled work and verified context cleanup. A consumed 404 with `net::ERR_ABORTED` cannot pass. Server finish/close counts do not prove successful client consumption. The report may be complete only if every case passes; a mixed comparison remains an incomplete, truthful report even if it identifies a promising candidate.
