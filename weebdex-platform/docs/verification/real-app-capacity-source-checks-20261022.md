# Real-app capacity adapter: finite source checks

Status: source-prepared, native runtime unexecuted, capacity unqualified.

Source/test commit: `3519a22da3be19bdf59ae7de9f48389690caacc6`.
Tree: `aa1911ca4626dab8a5b984b6fa729ca39beaa147`.
Frozen base: `8f878a6c189d2c6e0e5fe3c711724809d39e2ab6`.

On 2026-10-22 UTC, these finite checks completed successfully:

```text
PYTHONDONTWRITEBYTECODE=1 python3 weebdex-platform/tests/ci/test_real_app_capacity.py
Ran 8 tests in 0.004s; OK

node --test weebdex-platform/tests/ci/test_native_capacity_proxy.mjs
9 tests; 9 passed; 0 failed; 0 skipped; 122.776295 ms

git diff --check
No whitespace errors.
```

The Python tests exercise inert planning, stated global limits, PostgreSQL flag
changes before spawn, setup streaming/encoded target preservation, byte and
attempt bounds, denied authority/redirects, failed-readiness terminal accounting,
outer owner birth admission and finite cgroup requirements. The Node tests
exercise inert imports, exact numeric origin/Host/target rules, ambiguous
framing and policy-stripping refusal, attempt/inflight/queue/deadline exhaustion,
split chunks, declared overrun/short bodies, total payload cap, disconnect,
native/proxy/cache-only counter reconciliation and null empty latency summaries.

Test-only in-memory stream and connection seams are used to test machinery.
They are not application fixtures or capacity observations. No service, socket,
browser, runtime subprocess, build or dependency installation was launched.

The sparse source-only checkout footprint was 5.1 MiB after this commit. No
backend, frontend, dependency or vendor directories were duplicated. Both base
and candidate retain all 679 original frontend files with identical tree:
`c90f53f5018f8eda8613ccff97bf4a209b0e33f0`.

Independent source review is pending at this evidence snapshot. The executable
adapter's normal native transport, final native reconciliation, actual IDB
transaction completion, durable real PG settings, resource envelope and owned
cleanup must still be observed in a separately admitted runtime. These finite
results do not grant that admission or any monthly/30K/500K capacity acceptance.

Design and remaining gates: [real-app-capacity-probe.md](../real-app-capacity-probe.md).

## Independent review corrections, still no runtime execution

Read-only independent review identified six issues in the initial source:
setup early EOF was not checked against declared Content-Length; a mutable input
path could select different worker admission; progress completion was not bound
to the intended exact record; proxy latency was mislabeled client latency;
action promises lacked global failure-path settlement; and upload pipeline
callbacks were not included in the proxy's cleanup terminal set.

The corrective source validates setup declared length/framing, snapshots and
digest-binds parent/worker inputs, matches exact chapter/manga/page-two puts to
an action observation intent, separates proxy and native request-event latency,
and awaits every owned action/upload/response terminal. Focused finite controls
cover short valid JSON with an oversized declaration, split and overrun bodies,
duplicate/ambiguous framing, original config swap and snapshot tamper, unrelated
or aborted IDB commits, delayed action cancellation and delayed/missing upload
terminals. These remain machinery tests, not mocked application acceptance.

Corrective checks on 2026-10-22 UTC: Python **10/10** in 0.004s; Node **12/12** in
129.60675ms; `git diff --check` clean. Independent re-review is pending. No
service, socket, browser, heavy build, installation or hosted run was started.

### Follow-up review of the corrective source

The original setup short-body and mutable-config findings were verified fixed
by independent read-only review. Two adjacent issues remained: setup
Transfer-Encoding needed duplicate/value validation, and a successful response
could release an active proxy slot before a delayed upload failed during close.

The successor reads every Transfer-Encoding field, accepts only absent or one
`chunked` value without Content-Length, and refuses all ambiguous variants.
Each proxy exchange now records upload and response outcomes independently and
retains active credit until both are terminal. Only two successful terminals
can make a complete exchange; failed/cancelled outcomes survive shutdown.
Rejected responses own their actual stream close, and a missing response fails
on upstream close instead of leaving a fictional successful response.

Finite controls now cover early response plus delayed successful upload, no
premature slot release, failed/cancelled upload after an early response, and
duplicate/empty/unsupported/combined Transfer-Encoding. Results on 2026-10-22
UTC: Python **11/11** in 0.006s; Node **14/14** in 146.600598ms; whitespace check
clean. No runtime/service/socket/browser/build/install was launched. Successor
source remains pending independent re-review and all capacity claims stay false.

Final narrow parser-alignment correction: the setup guard no longer trims
Transfer-Encoding whitespace, matching installed `HTTPResponse`'s exact
case-insensitive `chunked` decision. A real `HTTPResponse` parsing in-memory wire
bytes accepts normal/uppercase chunked and proves trailing space/tab instead
select close-delimited parsing and are refused by the guard before body reads.
No network socket is created by this control. The dual-terminal proxy correction
passed the subsequent source review. Latest finite results: Python **12/12** in
0.005s; Node **14/14** in 136.516333ms; whitespace check clean. No runtime run or
capacity acceptance is implied.
