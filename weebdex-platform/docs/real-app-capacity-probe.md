# Source-prepared real-application capacity probe

Status: **implemented source, native execution held, not capacity qualified**.
The first adapter/test commit is `3519a22da3be19bdf59ae7de9f48389690caacc6`,
based on `8f878a6c189d2c6e0e5fe3c711724809d39e2ab6`. Independent source review
and actual functional/browser evidence are separate gates. No service, listener,
browser, heavy build, installation or hosted run was started for this increment.

This is an executable real-app adapter, not a replacement response server. It
does not convert the [unexecuted mixed-load proposal](mixed-load-qualification-plan.md),
the [read-fanout recipe](integrated-read-fanout-20261018.md), or finite machinery
tests into measured capacity. The [privacy-safe source and qualification boundary](verification/current-completion-ledger.md) is an editorial summary, not an authoritative execution ledger or a capacity result. The [withheld-evidence notice](verification/source-profile-and-withheld-evidence.md) explains which historical evidence is not included.

## What it actually exercises

`scripts/ci/qualify-real-app-capacity.py` is plan-only by default. Its explicit
execution path reuses `tests/runtime/browser_fixture.py:Fixture`, the existing
source/release inventory validator in `qualify-full-application-restore.py`, and
`tests/ci/qualification_process_owner.py:QualificationOwner`. It starts the real
platform release, PostgreSQL, embedded original publisher/strfry, generated native
and genuinely signed source seeds, and the reviewed frontend artifact. It does
not install dependencies, build artifacts, or use existing user data.

The adapter replaces only the fixture's two disposable PostgreSQL speed flags
before the original process is spawned, using `fsync=on`, `full_page_writes=on`
and `synchronous_commit=on` from initial startup. It then verifies all three with
actual `SHOW` queries. The original fixture source is unchanged. These settings
are not a claim about browser storage, kernel persistence or production DR.

The signed seed's canonical public UUIDs are obtained from the real
`POST /api/v1/legacy/resolve` response. `backend/src/public_api/at_home_core.rs`
advertises the application's configured public origin; the existing fixture
sets that to its exact numeric loopback listener. Native IDs are not substituted
into public reader paths, and media bodies are never invented by the driver.

`scripts/integrated-load/native-browser.mjs` uses the existing reviewed browser
runtime seam and the real title/reader listbox selectors. Four isolated browser
contexts each save the generated title as Reading through the real title UI,
then load the real `/chapter/{uuid}/1`, select page two, observe decoded visible
blob images and verify an actual new IndexedDB transaction completion plus the
stored page-two row. An observation intent binds the completion to that exact
chapter, manga and page-two put after the action is armed, not merely another
chapter/startup transaction. Only bounded intent numbers and counts are exported.
It subsequently cycles actual reader, search-form and
nonempty local-library navigation. Library membership is not injected directly.

The device database remains the real `weebdex-device-library-v1:<origin>` store.
A wrapper calls native `IDBObjectStore.put` unchanged and passively attaches
transaction complete/abort listeners for chapter records. Read-only checks do
not create missing databases or upgrade schemas. A former page-two row alone
cannot satisfy the new-reader-save check. This measures observed transaction
completion, not higher-level save-call resolution, page-position demand,
physical disk durability, or server writes. Default progress has **no HTTP save
lane**. Optional client-signed publication load is explicitly absent.

## Native transport and bounded accounting

`scripts/integrated-load/native-proxy.mjs` is a real streaming HTTP forward
proxy, not `context.route`, route fulfillment, an aiohttp stand-in, or a mock
backend. Chromium's native request lifecycle, API calls and image decoding stay
active. The observer does not call response `body()` or `arrayBuffer()`.

- Only the exact owned `http://127.0.0.1:<port>` origin and matching Host are
  accepted. No DNS names, userinfo, fragment, rewritten/normalized target,
  duplicate critical framing headers, CONNECT, upgrade, redirect-following or
  retry are admitted. A redirected destination must pass the same fence.
- The exact encoded path/query, method, body and end-to-end headers reach the
  app. Authentication payloads are not reserialized. Hop-by-hop headers are
  handled explicitly; Connection cannot remove an authorization or policy
  header. Existing app CSP and content encoding are retained, not weakened.
- GET/HEAD are allowed on that origin. POST is limited to actual forum context,
  protocol-resource mapping and analytics view endpoints. Upload/publication
  writes are not part of this adapter. Any unexpected HTTP 4xx/5xx fails this
  probe instead of being hidden as successful throughput.
- Chunks are charged before being forwarded, with backpressure. Declared-size
  overruns, short bodies, unknown completion, client disconnects, queue/deadline
  exhaustion or global credit exhaustion fail the probe. Received payload and
  admitted forwarded payload are separately recorded.
- The ceilings concern **encoded payload forwarded by the application proxy**,
  not NIC traffic, HTTP headers/framing, Node parser buffers, browser decode
  allocation or OS socket buffers. Those have separate process/resource fences;
  the report explicitly sets `wire_and_os_buffers_bounded` false. This is not an
  OS network namespace or a claim that arbitrary hostile software cannot egress.

Passive browser request/requestfinished/requestfailed events and passive CDP
events are reconciled by private exact method/URL multiplicities. Every browser
HTTP request must match a proxy attempt or an independently terminal cache-only
CDP observation with zero encoded wire length. Service workers are blocked.
Ambiguous cache behavior, hidden retry, missing terminals or bypass cannot pass.
Method/URL identities are used privately for matching and never exported.

Setup/readiness/UUID discovery also use an exact-owned-origin transport with
streaming credit admission, no ambient proxy, no redirect and no retry. Unread
setup payload is drained within its cap, not omitted. Connection-refused
readiness attempts are failed, terminal attempts without an invented HTTP
status. Internal provisioner, database and original-worker work is explicitly
server setup, not counted as browser requests. A setup failure is not a load
result, and no bucket's unused capacity is borrowed by the other.

Setup Content-Length must exactly match the consumed body. Transfer-Encoding is
accepted only when absent or exactly one `chunked` value without Content-Length;
duplicate, empty, unsupported or combined framing is refused. The setup guard
matches the HTTP parser's case-insensitive value exactly; trailing space/tab is
refused rather than normalized into a framing mode the parser did not select.
Proxy exchanges
retain active credit until both upload and response pipeline outcomes settle.
An early successful response with a pending or failed upload is not complete,
and teardown cannot turn a cancelled upload into an unforced success.

## Proposed admitted envelope, not an observed runtime result

The new proposal reserves **600 seconds for the owned lifecycle**: setup at most 150, native browser
(including warmup, nominal phases, tails and drain) at most 180, inner fixture
cleanup allocation 245, and outer owner cleanup reserve 25. The existing outer
owner supplies the hard absolute bound. This does not reuse the old proposal's
unrealistic 20-second allowance for the real fixture's 220-second native drain.
No running deadline is extended. Exhaustion or forced cleanup is failure.
Outer source/release preflight and post-run inventory hashing occur outside that
owned-lifecycle interval; 600 seconds is not an end-to-end CLI wall-time claim.

| Resource | Ceiling or minimum |
| --- | --- |
| Setup / native HTTP attempts | 300 / 2,200; total 2,500 |
| Setup / native response payload | 64 / 448 MiB; total 512 MiB |
| Setup / native upload payload | 1 / 1 MiB; request body at most 64 KiB |
| Native active / queued requests | 16 / 32; at most 64 accepted sockets |
| Browser devices | Four isolated contexts, one active action per context |
| Owned process RSS | At most 1 GiB |
| Measured finite cgroup headroom | At least 512 MiB; unknown/unlimited fails |
| Host available-memory floor | At least 768 MiB |
| Launch / live free disk | At least 4 GiB +256 MiB / 4 GiB +128 MiB |
| Evidence growth | At most 256 MiB, including owned runtime roots |

The fixed nominal phases are smoke 10s/one device/2 HTTP requests per second,
baseline 20s/two devices/12, mixed 30s/four devices/30, and pressure 20s/four
devices/60. These are HTTP ceilings, not achieved rates. The nominal 80 seconds
exclude warmup and measured phase tails. At each phase boundary all admitted
actions and streams must settle. A busy device drops a newly offered action,
counts that drop explicitly, and does not accumulate an unbounded action queue.

The reader/search/library cycle is an explicit sensitivity mix, not inferred
from monthly production demand. The existing small generated fixture is reused,
not the old proposed 32-title/256-chapter corpus or 24-user pressure phase. This
does not establish sustained capacity, a representative cold/warm distribution,
HEAD/range/validator coverage, upload demand or peak SLOs.

## Measurement and evidence boundaries

The fixed-schema summary includes source commit and inventory/build/functional
association hashes; browser/runtime identity; phase offered/started/completed
actions and busy drops; elapsed time and phase tails; actual HTTP status/route
class counters; payload bytes received/admitted for forwarding; and nearest-rank
p50/p95/p99 proxy queue, upstream-header, first-payload and full-proxy-payload
latencies. These proxy timings are not browser completion measurements. Separate
native request-to-finished/request-to-failed event timings and observed IDB
transaction timings are reported. Admitted proxy bytes are not proof of browser
receipt. Empty observations are null, not zero-latency successes. HTTP/image
requests, decoded-image checks, logical journeys and IndexedDB commits remain
different units.

Actual PostgreSQL samples record connections, active/waiting backends, original
outbox count/oldest age and pending platform publication-command count. Final
backlog must be zero. These are not application pool-acquisition timings or
per-handler server latency; both are explicitly reported unavailable. Owned
resource samples aggregate peak RSS/process count, disk/memory minima, evidence
growth and birth-matched sampled CPU tick deltas. CPU deltas miss work before
the first/after the last observation and are not full process-lifetime CPU.

The reused owner supplies PID birth identity, pidfds, subreaper adoption,
descendant tracking, resource observation and terminal stdout/cleanup evidence.
The hidden worker verifies its outer owner's parent PID and birth identity.
Both the real fixture's clean/unforced teardown and the outer owner's verified
unforced shutdown are required. Proxy sockets, upstream requests and pipeline
terminals, including request-upload callbacks, must also settle. All action
promises are globally retained, contexts cancel pending actions during teardown,
and every action must settle within the bounded wait before cleanup can pass.
Uncertain ownership or cleanup is never success.

Only a reviewed fixed-schema `summary.json` is an export candidate. Raw runtime
logs, supervisor records, runtime databases, generated signing keys, mail links,
cookies, request/response bodies, exact URLs, UUIDs, ports and filesystem paths
are not capacity report content. The existing private fixture retains its
normal diagnostic/runtime exclusion boundaries. Do not archive these private
runtime roots as source evidence.

## Future invocation and gates

Plan-only, safe to inspect without importing runtime dependencies:

```sh
python3 scripts/ci/qualify-real-app-capacity.py
```

Do **not** use the execution flag while the heavy lease is held, local browser
launch remains denied, or source review/functional evidence is incomplete. A
denied local launch is not authorization to retry, install a different browser,
lower a floor, switch to a hosted run, or mark runtime acceptance complete.

Future explicitly admitted invocation from the full source/release checkout:

```sh
python3 scripts/ci/qualify-real-app-capacity.py \
  --execute-synthetic --config /absolute/reviewed-capacity-inputs.json \
  --output /absolute/weebdex-platform/test-evidence/new-private-capacity-run
```

The config requires exactly these fields: `binary`, `protocol_provisioner`,
`ui_dir`, `release_manifest`, `frontend_artifact_manifest`, `pins`,
`pins_sha256`, `source_commit`, `build_association`,
`build_association_sha256`, `functional_gate`, `functional_gate_sha256`.
Paths are absolute. The output must be a new private directory under the
checkout's `test-evidence`. The parent serializes its admitted inputs into a new
private canonical config snapshot and passes its digest to the worker. The worker
hashes the exact parsed bytes and its report must match the parent's source,
pins, build, functional gate and config identity. Swapping the original config
between parent admission and worker startup cannot select a different run.
The sparse preparation worktree deliberately lacks
the required full runtime source/artifacts and cannot satisfy runtime admission.

The pins use the existing full-restore inventory format, including actual
source, tools, locks, platform release and frontend manifest/artifact checks.
`source_commit` must be the exact clean tracked HEAD. Both inventory and release
are rechecked after work, with a final check after outer cleanup.

The separately reviewed build association has format
`reviewed-capacity-build-association-v1` and exactly `source_commit`,
`source_inventory_sha256`, `release_manifest_sha256`, `platform_sha256`, and
`provisioner_sha256`. The source inventory digest is SHA-256 over compact,
sorted-key JSON of `pins.source`. This is an explicit reviewed build association,
not proof inferred from an arbitrary binary beside an arbitrary checkout.

The separately reviewed functional gate has format
`reviewed-capacity-functional-gate-v1`, exact `source_commit`, `requirements`,
and `evidence`. Requirements are upload originals/replacement/recovery, signed
chapter authority, catalogue/account/library, and real native reader browser.
All must be true, backed by one to 32 exact existing evidence files and their
SHA-256 hashes under this checkout's `test-evidence`. The gate is a reviewer
attestation to actual evidence, not a schema someone should populate merely to
make a blocked command run. This driver does not self-grant its functional gate
and does not automatically reuse old-source passes.

Early worker admission or outer-owner exceptions can leave only the existing
private owner failure receipt rather than a completed worker/final summary.
Missing reports remain failure, not acceptance; failure-summary completeness
is a reporting follow-up, not a reason to infer a successful run.

## Acceptance that remains separate

Every possible report from this driver keeps `capacity_qualified: false`.
At most, fully admitted future runtime evidence can establish a
`passed-bounded-real-application-probe-only` result. Missing browser execution is
an acceptance blocker, not a backend-only success.

The 30K/500K library browser publication/recovery exercises, optional signed
publication load, sustained representative read/save workloads, and the target
8B image deliveries /350M pages /300M successful progress saves monthly with
about 100TB storage all remain unqualified. A few devices and 60 total HTTP
requests per second cannot prove even the monthly average image-delivery demand.
No tiny-run extrapolation, production deployment, purchase, new credential or
real-user-data use is authorized or performed by this increment.
