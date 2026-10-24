# Bounded integrated-load transport primitive

Status: finite transport seams only. Source base `5cbd05d91c38e22eb6a7509aa6e9f8c21b12e3b4`.
No application, browser, PostgreSQL or production load was started. This adds
transport controls for a future integrated runner; it does not replace the
original-node generator or establish any production capacity.

## Shared accounting

`scripts/integrated_load_budget.py` provides `SharedBudget` and the owned
`BudgetedHTTP` async context manager. Every request in setup, catalogue, reader,
media, analytics, public sync, uploads, diagnostics and verification uses the same
budget instance. Defaults/hard upper bounds are 2,500 admitted HTTP attempts,
512MiB consumed encoded response payload, 32MiB conservatively reserved serialized
upload payload, 180s from budget construction and 32 active attempts. Smaller
bounds are supported. It owns one process/event loop, not the original generator's
multiprocessing image workers. It contains no scheduler or implicit network work.

An attempt is charged at actual transport-handler admission, including connection
refusal or disconnect before response headers. A refusal spends no HTTP attempt
and sends nothing. Caller-requested retries need new admission and spend another
attempt/body reservation. The library's hidden idempotent reconnect retry is
blocked with public per-request middleware; no private aiohttp retry setting is
modified. Redirects are disabled, including same-origin redirects and 302/307
responses. No authority or body is forwarded to their destinations.

The owned session disables environment proxy discovery, cookie persistence and
automatic decompression. Only explicitly allowlisted numeric-loopback HTTP origins
are accepted; credentials, fragments, whitespace/control characters and caller
Host/length/transfer-framing overrides are rejected. URLs are passed encoded so
an already serialized/signed ASCII target is not silently requoted. A request
needs an explicit path and must round-trip unchanged through the encoded URL
representation; empty fragment/query delimiters that would disappear are refused. Signed callers
still own exact URL, nonce, payload, response-proof and authority verification.
Request URLs, bodies, headers and exception messages are never put in the budget
snapshot. Raw returned response bodies/headers remain private caller inputs.

HTTP methods are transport facts, not database-write classifications. The
read-only forum-context POST consumes an attempt and serialized-body credit but
must not be called a progress/database write. A multi-request logical journey
spends one credit per real request; no three-requests-per-page assumption exists.

## Byte and timing boundaries

The shared reader reserves credits before each streamed body read and returns
unused credits after EOF/cancellation. Concurrent lanes cannot collectively
consume more than the response limit. A delayed chunked framing EOF at the exact
limit may complete without consuming an uncharged lookahead byte. An extra suffix
fails. An omitted/truncated/false Content-Length cannot invent complete transport.
All known responses, including 503s and proof-rejected 200s, retain their status
and byte costs. HEAD counts actual zero payload, not its advertised object size.

`response_body_bytes_consumed` is encoded payload delivered to the driver, without
HTTP headers or chunk framing. `parser_received_payload_bytes` separately reports
payload already prefetched by aiohttp, which can exceed the reader limit. This is
not a hard NIC/OS-buffer or origin-bandwidth ceiling. Before a response object is
available, received body bytes are unknown and stay null per attempt; missing
observations are counted separately, never converted to measured zero.

Upload reservations use the full serialized byte body for each admitted attempt,
including failed/uncertain attempts. They are conservative limits, not claims
that those bytes reached the server. Streaming uploads are intentionally outside
this initial primitive. The HTTP fixture verifies exact received bytes only for
its own bounded synthetic positive cases.

Per-attempt timings distinguish response headers, first consumed body bytes and
complete-body observation from the transport's admission. They include client
and loopback costs and are not isolated server timings or an agreed latency SLO.
A separately supplied verifier must be cooperative caller code and return exactly
true for `semantic_verified`; this primitive cannot preempt arbitrary blocking code.
Transport completion or a 200 status alone is not semantic acceptance. Absolute
request/global deadlines are also checked after verification, so a stalled loop
cannot convert a late observation into an in-time completion.

Cancellation closes the response, joins its writer within a one-second cleanup
bound, returns read credits and ends its active permit. Exiting the owned transport
cancels and joins outstanding request tasks before closing its session. Nothing
replays an uncertain mutation. Callers must still observe fixture-owned process,
resource, source/binary, authority and final-drain gates before any application run.
The budget deadline stops HTTP work; the enclosing command must reserve its
separate teardown allowance inside its absolute 180s wall-clock deadline.

## Device progress and scenario wiring

There is no `local_progress` network lane. The snapshot records device progress as
unmeasured, its observed logical count as null, expected HTTP writes as zero, and
backend substitution as forbidden. This does not simulate or claim IndexedDB
performance. Optional public-library snapshots remain a separately selected
`public_sync` stream, with no invented consent fraction or publication cadence.

A future runner must use the mounted-client endpoint fan-out scenarios, including
conditional hydration, aggregate selectors, tag pagination, prefetch/retry policy
and separate bootstrap/media/comments/analytics requests. It must route every
network request through this primitive and preserve actual logical-action counts.
This change does not activate an application runner, choose offered rates, prove
100TB storage behavior or authorize a load job. The entry/resource limits in
[mixed-load-qualification-plan.md](mixed-load-qualification-plan.md) still apply.

## Reproduce finite seams

Using the existing Python/aiohttp environment, from the repository root:

```sh
PYTHONDONTWRITEBYTECODE=1 timeout 30s python3 -m unittest discover \
  -s weebdex-platform/tests/ci -p test_integrated_load_budget.py -v
```

For a new retained receipt including actual fixture-observed request/body counters,
use the dedicated observation runner with a new output directory:

```sh
PYTHONDONTWRITEBYTECODE=1 timeout 30s python3 \
  weebdex-platform/scripts/ci/qualify-integrated-load-budget.py \
  --output weebdex-platform/test-evidence/ci/NEW-TRANSPORT-RECEIPT
```

The tests own ephemeral numeric-loopback raw HTTP listeners and bounded synthetic
bodies. They cover shared request/read/upload/inflight/deadline limits, concurrent
last credits, actual connector/disconnect attempts, no hidden GET/PUT replay,
manual retry accounting, same/cross-origin 302/307 refusal, encoded gzip, missing
or truncated lengths, exact/delayed/overflow chunked EOF, non-2xx and semantic
rejection costs, cancellation/socket closure, late verification and unsafe-input
refusal. No service, package installation, external network, image dataset or
browser is needed. Qualification uses the installed aiohttp 3.14.4; separate
validation is still required against the original environment's pinned version.

## Finite mounted scenario consumer (2026-10-22)

The [mounted reader scenario core](integrated-mounted-scenarios.md) now consumes
this primitive through one coordinator for actual composed component/API fan-out
against an owned synthetic HTTP fixture. This is not the former recipe's
application dispatch, whole-route/browser measurement or backend qualification.
The original transport evidence above remains historical finite seam evidence.
