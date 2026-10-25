# Finite source-bound mounted reader scenarios

This is an executable **synthetic scenario core**, not an application-load pass.
It mounts the actual composed Svelte Reader.Provider, Queue, History and Analytics,
uses the composed Chapter API and the actual public client, normalization, selector
pagination and DeviceStore/saveProgress implementations. Requests are not generated
from a nominal reads-per-page recipe. `integrated_load_scenarios.py` owns a single
loopback fixture, one Node child, and the existing `SharedBudget`/`BudgetedHTTP`.
It has no external target URL, real account, PostgreSQL, native publisher or public
relay mode. No dependency install or application-service launch occurs.

## Scheduling and admission

Default invocation prints a plan without traffic. Explicit `--synthetic` schedules
one to eight **serial device actions**, each with an explicit chapter, prefetch
setting, finite zero-based page positions, and dwell time. Concurrent API/queue
requests share one rate gate, attempt/byte/inflight budget and absolute deadline,
including setup, actual automatic image retries and analytics. No action weights,
active-user estimate, consent fraction, public-sync cadence, or monthly projection
is inferred. This is not a multi-user concurrency scheduler.

Hard synthetic bounds are 101 chapters, eight positions/chapter, eight actions,
60 HTTP attempts/s, 250 total attempts, 8 MiB consumed response payload, 64 KiB
serialized request bodies, eight active attempts, and 30 seconds including child
startup. Defaults are smaller (see `--plan` output; the no-flag invocation is the
plan). A one-second child reap allowance and fixture shutdown follow the shared
work deadline; this is not permission to extend a production command's deadline.
Explicit per-action prefetch is not claimed as the application's default setting.

Every request traverses the coordinator, with encoded payload accounting and
latency boundaries inherited from the transport primitive. The bridge disables
external origins, redirects, ambient proxy state, native account transports and
public signing. It exposes no cookie/account authority. The ordinary anonymous
analytics callback is mounted, but a real visitor cookie jar/server dedup policy
is not modeled. A fixture 204 is not proof of backend dedup or rate limiting.

A global admission/transport error, integrity/origin escape or two unexpected 5xx
responses cancels all admitted work and reaps the child. All unexpected statuses
remain failures even if a UI adapter provides graceful fallback. Nothing retries
an uncertain write. The child's ordinary media retries are actual Queue policy,
not an extra driver retry. Per-fetch cancellation is forwarded to its owner.

## Observations and boundaries

- `logical_actions_*` count offered, started and semantically settled reader
  mounts. `page_positions_selected` is a controlled mutation of actual reader
  state, not a browser click or a second page load. Per-action HTTP call counts
  and actual transport attempts are separate, with scope setup outside actions.
- Python records actual admitted HTTP attempts, statuses, body consumption,
  prefetched parser bytes, method, lane and header/first-body/complete-body timing.
  Percentiles are nearest-rank client/loopback observations (including failed
  requests when that boundary was reached), not server latency or SLOs. The
  transport `semantic_verified` flag stays false: there is no per-response
  callback at that layer. Actual adapter/image/final-IDB checks are reported
  separately by the mounted child and required for aggregate success.
- The independently owned fixture counts received requests and request-body bytes.
  Its response sizes/statuses are explicitly **prepared**, not claimed delivered.
  Positive tests reconcile those sizes with client-consumed payload. Cancelled
  responses can differ. Neither observation is NIC traffic or bandwidth capacity.
- History's real 350 ms coalescing calls actual `saveProgress`, which commits to
  actual DeviceStore code on fake-indexeddb 6.2.4. The report counts admitted calls,
  completed commits, failed commits, final stored page and adapter elapsed times.
  It never labels a page position as a commit or a commit as a backend write.
- Svelte 5.55.1 and JSDOM 27.0.1 run real component lifecycle/reactivity. Geometry,
  keyboard/scroll components, whole route shell, static assets, covers and comments
  are explicitly excluded. A digest-checked 1×1 owned PNG uses a declared Image
  decode stand-in; no native-browser image decoder, cache or rendering is measured.
- Local cryptographic hashing uses Node SHA-256; signature methods fail closed.
  No optional public-sync stream executes. Its consent, cadence and sizes remain
  unresolved, not a guessed fraction of progress commits.
- Source files actually loaded are hashed and rechecked after execution. The
  coordinator records its Git base and driver hashes. Committed source identity
  must be checked separately from dirty worktree hashes.

The known requested goals (8B image deliveries, 350M page loads and 300M logical
progress saves per month, plus a 100TB library) remain entirely unqualified.
Whole-app/native-browser/private runtime/storage/backend execution is still held
by `mixed-load-qualification-plan.md`. This finite hot fixture is not a baseline
for extrapolation, provisioned dataset, stress run or production approval.

## Reproduce with existing exact dependencies only

```sh
export WEEBDEX_LOAD_NODE_MODULES=/absolute/existing/node_modules
python3 weebdex-platform/scripts/integrated_load_scenarios.py
python3 weebdex-platform/scripts/integrated_load_scenarios.py --synthetic
PYTHONDONTWRITEBYTECODE=1 timeout 70s python3 -m unittest discover \
  -s weebdex-platform/tests/ci -p test_integrated_load_scenarios.py -v
```

The dependency root must contain Svelte 5.55.1, JSDOM 27.0.1,
fake-indexeddb 6.2.4 and TypeScript 5.9.3. Python/aiohttp requirements are those of
the existing transport primitive. Missing/changed prerequisites fail before
starting the fixture, not as a skipped pass. Node is invoked with experimental
VM modules and browser export conditions; this is still Node, not Chromium.

`--config /path/to/synthetic.json` accepts only the documented finite fields in
the plan. Unknown fields, target URLs, sync settings and out-of-range values fail
closed. The implementation and tests do not authorize an application target mode.

The mounted probe exposed and now guards an actual queue replacement ownership
bug; see `reader-queue-lifetime.md`. Six controlled-tick regressions fail on the
exact old source, independently of the synthetic HTTP fixture.
