# Mixed-load readiness audit, 2026-10-18

Scope: source and finite controlled-HTTP/process seams only, based on repository
`be283b664f699e8c7175f6eac0200cd22e212c8a`. No application load, database, service
installation, infrastructure purchase or public-relay request was run by this audit.

## Requested demand and current generator

Use a 30-day accounting month. The requested arithmetic averages are 3,086.419753
image deliveries/s, 135.030864 page-load events/s, and 115.740741 logical progress
saves/s. The 100TB library is a storage/working-set requirement, not a request rate.
The generator records 100TB as 100,000,000,000,000 decimal bytes for planning only;
resolve whether binary units were intended before provisioning.

The retained original `manga-protocol-lab/bench/library-node/mixed.py` is a
synthetic original-node public-seam benchmark. Its unscaled rates remain stable
for historical comparison:

| Lane | Nominal operations/s | Actual unit |
| --- | ---: | --- |
| image | 3087 | One complete managed-nginx image GET |
| api | 405 | One catalogue/discovery API read; 135 assumed page equivalents × 3 |
| save | 120 | Signed durable original-node library HTTP acknowledgement |
| library_read | 10 | Signed head, root and up to three chunks; multiple HTTP calls |
| social | 2 | Signed public comment write |
| api_image | 1 | One original-image read through Rust |
| upload | 0.1 | Fresh original + chapter publication + peer import + integrity read |
| sync | 0.05 | Public profile admission/publication/import job; multiple HTTP calls |

The sum, 3,625.15 logical lane operations/s, is not the total HTTP request rate.
Upload, sync and selective restore have multiple requests; setup, projection
verification and diagnostics add separately recorded requests. Finite phase
rounding further changes actual offered proportions. Do not report these lane
weights as observed production usage or add their nominal rates as database
write throughput.

In the integrated public-UUID app, reader `saveProgress` writes local IndexedDB
and the reader coalesces saves for 350ms. Default progress produces zero progress
HTTP writes. Optional public sync requires a separately reviewed client-signed
library snapshot; neither its consent fraction nor cadence/size is supplied.
The original generator's `save` and profile `sync` lanes exercise different
semantics. `demand_accounting` now makes this boundary machine-readable in every
CLI plan and `mixed-summary.json` plan, without changing historical schedules.

Three API reads per page remain an assumption. A representative integrated driver
must first measure actual route/API/image/analytics fan-out. An image GET does not
increment a view. Page-load semantics versus reader page positions remain an open
input; no ratio between monthly totals supplies missing fan-out or active users.

The default image corpus has three retained static JPEGs and a fourth source for
fresh JPEG metadata variants. It is a tiny hot working set. Timed image hashing
already defaults off; initial corpus/integrity controls remain hash-verified.
This corpus, loopback network and same-host generator cannot demonstrate a cold
100TB library, CDN hit ratio, WAN bandwidth or independent generator headroom.

## Fixed accounting bug and reproducible finite seams

Rejected HTTP responses previously incremented operation error reasons, but were
missing from the per-lane `http_statuses` and rejected-operation status field.
The error path now retains known HTTP statuses, including response-verification
failures, without counting those operations as accepted. Transport failures with
no known status retain no invented status. Existing accepted-status counts are
unchanged. A separate JPEG metadata seam now generates a tiny owned JPEG with
installed Pillow. The authentic manifest-backed JPEG case remains, verifies exact bytes/hash
before transformation, and accepts an explicit local `MIXED_JPEG_SOURCE` path. If
the optional source image is absent, that case is explicitly skipped; the owned
synthetic case still runs.

From the adjacent `manga-protocol-lab` directory, with existing Python packages
only. If the authentic JPEG is outside the checkout, set `MIXED_JPEG_SOURCE` to
its verified absolute local path before the test command:

```sh
PYTHONPATH=bench/library-node timeout 90s python3 -m unittest \
  test_mixed test_image_shards test_process_metrics test_diagnostic_process -v
python3 bench/library-node/mixed.py --plan \
  --duration 20 --scale .001 --users 2 --extreme-users 0
```

`--plan` does not start traffic or require a database. The 20s plan's rounded rare
lane offers are synthetic smoke controls; it is not a monthly-demand sample.
Controlled HTTP tests bind only ephemeral loopback ports, use sub-second offered
windows, and own/reap spawned image workers and diagnostic children. They cover
strict versus bounded acknowledgement deadlines, rejection/status accounting,
reader poisoning, complete body length/MIME, missing/crashed workers, shared
pending credits, altered worker evidence and finite cleanup. These are generator
correctness checks, not server/application capacity measurements.

Evidence is under
`manga-protocol-lab/results/rebuild-mixed-prep/readiness-20261018/`:
`accounting-red.log` reproduces the missing-503-status and absent demand-accounting
regressions against base `be283b6`; `seams-green.log` records 37 final finite
tests passing in 32.575 seconds, with the separately recovered authentic JPEG
explicitly selected and no skipped cases;
`original-node-tiny-plan.json` records the no-traffic plan. The source export's
signing/CBOR packages are not needed for this finite suite. The authentic JPEG
case needs the separately recovered bytes; the owned synthetic case uses installed
Pillow and remains runnable without those bytes.

## Held application qualification

No heavy/application load is authorized by a passing seam. The integrated plan
remains [mixed-load-qualification-plan.md](mixed-load-qualification-plan.md),
with frozen final source/binary/config hashes and functional/browser decisions
before execution. Its eventual loopback-only stages are:

1. 10s smoke, one user, aggregate cap 2 HTTP requests/s
2. 20s baseline, four users, cap 12 HTTP requests/s; establish actual logical fan-out
3. 30s mixed sensitivity, 12 users, cap 30 HTTP requests/s
4. Conditional 20s pressure probe, 24 users, cap 60 HTTP requests/s
5. Stop new mutations and observe the accepted work for at most 20s of drain

Use one coordinator's sole heavy/PostgreSQL lease and the admitted memory budget,
4GiB free-space floor plus 128MiB launch reserve, 180s absolute deadline, 2,500
HTTP requests including retries, 512MiB response-byte cap, 32MiB new upload cap,
eight chapter commits and one load process. Do not convert the old driver's
logical lane rates to this cap using `--scale`: its multi-request operations,
setup and diagnostics require an actual transport-level budget. A dedicated
integrated driver with global request/byte/deadline accounting is still required.
A [bounded shared transport primitive](integrated-load-transport-budget.md) now
provides finite request/upload/read-credit/deadline controls and controlled HTTP
seams. Full integrated scenario scheduling, measured browser progress and
application-load qualification remain pending. No retry of an uncertain mutation is permitted. Preserve unexpected errors and
stop on integrity/authority failures or the established resource limits.

Keep TTFB/complete-body and client signing/serialization time distinct. Current
lane latency is client dispatch-to-ack or scheduled-to-ack, not isolated server
latency; original image requests do not yet have separate TTFB histograms. Sample
pool waits, PostgreSQL waits, outbox age, node/worker/generator resources and
cache/byte distribution before proposing an optimization. Do not infer an SLO
from the existing 30s request/60s closing safety bounds.

The initial audit environment lacked the original signing/CBOR Python packages
and retained source JPEG. Parallel restoration recovered isolated cbor2 5.6.5,
coincurve 21.0.0 and the authentic manifest-backed JPEG; this audit did not install
or download them. PostgreSQL/native application executables and a newly qualified
final integrated release remain separate runtime gates. Any later run must
recheck actual prerequisites and exact source/binary identity. Real browser/IndexedDB
performance and the user’s production-scale storage/throughput requirements remain unqualified.

## 2026-10-22 finite mounted core, not application admission

A [source-bound mounted reader scenario core](integrated-mounted-scenarios.md)
now exercises actual composed reader/API fan-out, local coalescing and fake-IDB
commits against exact owned loopback fixtures, using the shared transport budget.
Whole-app/shell/static/comment/cover fan-out, real browser/device performance,
actual backend/native runtime, consented signed sync and production capacity are
still unmeasured. No application-target execution gate is relaxed by this addition.
