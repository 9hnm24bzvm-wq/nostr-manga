# Bounded mixed-load qualification plan

Status: prepared, not executed. Updated after native-profile backend qualification; final feature composition/frontend qualification still takes priority.
This is a local backend experiment proposal, not a production capacity promise.
The [2026-10-18 readiness audit](mixed-load-readiness-20261018.md) distinguishes
the retained original-node generator from this integrated-app proposal and gives
reproducible finite harness checks. Neither has established the requested capacity.

## Entry gate

Do not start load until the final integrated commit is frozen and has passed:

1. The generated-image upload regressions and real PostgreSQL/strfry source
   lifecycle on that exact integrated source.
2. The new signed chapter write/removal cases, including source publisher,
   legitimate local claimant without source signing authority, moderator local
   hide, proof replay/tamper and post-decode admission/binding revocation.
3. The catalogue, account/library and frontend integration checks affected by
   the other workers, with original frontend source parity preserved.
4. The requested browser journeys, or an explicit user decision to qualify only
   backend load while the known browser execution environment remains blocked.

A compilation or old-source pass does not satisfy this gate. Keep the load job
held if any required functional result is pending or blocked.

## Mapping the requested monthly volumes

Use an explicit 30-day accounting month for this planning conversion only:

| Requested logical volume | 30-day average | What the experiment must measure |
| --- | ---: | --- |
| 8,000,000,000 image deliveries/month | 3,086.42 deliveries/s | Actual reader/cover bytes, visibility, Range/HEAD/validator behavior and hot/cold cache state |
| 350,000,000 page loads/month | 135.03 page-load events/s | Actual route/API fan-out for catalogue, title and reader journeys, including analytics |
| 300,000,000 progress saves/month | 115.74 logical saves/s | Default device IndexedDB commits/coalescing first; separately count any explicitly selected network destination |

These are arithmetic averages, not tested capacity, user concurrency, a peak SLO,
or a required artificial backend mix. Clarify whether “pages” means site page
loads or reader page positions before extrapolation. Do not divide one total by
another to invent chapters per visit, image fan-out or active accounts.

Default public-UUID reader progress uses `local-library/library.ts:saveProgress`
and the retained reader overlay's 350 ms coalescing into the captured device scope.
It makes no progress HTTP request. The 300M logical-save demand therefore cannot
be relabelled 115.74 PostgreSQL writes/s. Actual browser/IndexedDB latency, tab
contention, exit flush, offline recovery and storage failures need their own
browser experiment. Existing finite/JSDOM/IDB checks are correctness evidence,
not that experiment; a blocked native browser must remain explicitly blocked.

Optional public relay sync publishes an explicitly reviewed, client-signed
snapshot. It is neither automatic progress replication nor private cloud sync.
No monthly publication rate, snapshot size or consent fraction was supplied.
Record those as unresolved inputs and keep a separately bounded sync background
stream; do not manufacture a percentage of 300M network writes. Native bound
reading-status `POST /api/v1/manga/{id}/status` edits a six-value account status,
not per-page progress. Full native private-library mode remains fenced in public
UUID mode and is outside this local-first load experiment.

Measure actual page-load fan-out before translating 135.03 logical page events/s
to API requests. Likewise only the delivered-image distribution and measured
cache hits can translate 3,086.42 image deliveries/s to origin work and bandwidth.
Run explicit cold/warm-origin cases; do not assume a CDN hit rate or treat an
unconfigured CDN as measured infrastructure. Any future peak multipliers are
labelled planning scenarios, never user requirements or acceptance claims.

## Owned environment and corpus

Use one disposable loopback-only PostgreSQL database, one platform process and
one real embedded original strfry worker sharing the configured pool. Freeze the
final integrated source, release binary, lockfile and configuration hashes. No
existing user database, public relay, production endpoint or third-party service
is a target. No runtime database, signer file, proof, cookie or personal data may
enter exported evidence. Do not install a new load tool without its separate
required authorization.

Proposed synthetic corpus: 32 titles, 256 readable chapters, eight page positions
per chapter, and 64 generated mixed PNG/JPEG originals. Include repeated original
hashes within/across chapters under the same owned storage authority. Use
nontrivial images up to two megapixels, with 32 MiB unique originals and 128 MiB total
fixture-storage caps. Add eight bounded open upload sessions and 16 explicitly
admitted/bound synthetic accounts. Seed distinct current signed library heads,
real native account status/follow records, safe/suggestive/adult visibility cases
and both hot/cold targets. Do not use real reader libraries or licensed images.
Record exact row counts, byte sizes, image formats and public/private scopes.

## Workflows and accounting

Discover UUIDs, event hashes, versions and media names from actual responses.
Never substitute native IDs in public paths. Sign each authenticated request's
exact method/URL/service/body/nonce; serialize multipart bodies once. Serialize
writes per account in the ordinary stream; a separate small correctness stream
covers intentional races rather than hiding ordinary throughput behind them.

Maintain distinct operation counters instead of the former arbitrary percentage
mix. Use three named phases plus a separately labelled mixed sensitivity phase:

1. Reader delivery: `GET /api/v1/at-home/server/{chapter}`, then its advertised
   base URL/hash/file names for actual page GETs; also genuine cover paths.
   Count delivered bytes/digests separately from descriptor/API reads. Within
   this synthetic probe, deliberately include HEAD, valid ranges and conditional
   validators as independent test controls, not presumed production shares.
2. Page journey reads: `GET /api/v1/manga`, `/manga/{id}`, `/manga/{id}/feed`,
   `/manga/{id}/aggregate`, `/chapter/{id}`, statistics, and genuine `/manga/top`
   or `/chapter/top` for each supported window. Derive fan-out from the retained
   actual client/route. Record view calls only through
   `POST /api/v1/analytics/{manga|chapter}/{id}/views` with the actual visitor jar.
   Verify empty 204, resource/visitor/hour dedup and rate-limited 429 distinctly.
   An image GET is not a view-counter increment.
3. Progress and optional sync: keep the default device-progress/browser result
   separate. For a synthetic consented public-sync stream, use the actual
   client-signed `/compat/mangadex/nostr/library/batches` and
   `/compat/mangadex/nostr/libraries` workflow on the owned original listener;
   verify public read-your-writes against its exact new signed head via
   `/api/v1/manga/{id}/read`, `/manga/read`, `/manga/status`, `/user/list`,
   `/list/{id}` and `/list/{id}/feed`. Original signed-library GET status is not
   native account status. Plain public read-marker POST remains intentional 501.
4. Mixed sensitivity: replay the measured synthetic reader/page fan-out alongside
   explicitly bounded public-sync batches. Report offered and achieved rates per
   class and every chosen weight, rather than presenting a locally chosen mix as
   the 8B/350M/300M production demand. New native account profile/status/follow
   writes may form a small separately identified current-version control stream;
   they are not substitutes for page-progress saves.

Upload/replace/discard/commit and versioned chapter edits/removal remain finite
correctness controls, not silently assigned a share of the monthly read demand.
If included in the sensitivity phase, cap them at eight completed chapters with
four pages each, two genuine replacements and two discarded sessions; keep
mutation targets separate from continuous availability assertions. Source
withdrawal remains an explicit review request, never automatic approval.

Wrong signatures, replay, foreign-owner and stale-version requests are a separate
small expected-refusal stream. Measure 401/403/404/409 separately; do not inflate
successful throughput or hide overload failures inside expected denials.

## Finite stages and limits

Start only under the coordinator’s sole heavy/PG lease, with the existing 4 GiB
free-space floor plus 128 MiB launch reserve and no automatic cleanup. Proposed
aggregate owned-process RSS cap: 1 GiB, with at least 512 MiB of additional
headroom under the measured finite cgroup memory limit. The coordinator must
record/admit this memory budget before starting; sample CPU rather than treating
normal saturation as proof of failure. If headroom is insufficient, report the exact blocked stage;
do not delete additional data or weaken the floor.

Use an absolute command deadline of 180 seconds and the existing birth-tracked
fixture cleanup. Keep setup below 30 seconds and teardown below 20 seconds.
Do not extend a running command or launch a second overlapping load job.

1. Smoke: 10 seconds, one active user, cap two requests/second.
2. Baseline: 20 seconds, four users, cap 12 requests/second; record actual logical journey fan-out.
3. Mixed: 30 seconds, 12 users, cap 30 requests/second.
4. Pressure probe: 20 seconds, 24 users, cap 60 requests/second. Run only if the
   earlier stages have no unexplained functional errors or rising backlog.
5. Drain: stop new mutations, then wait at most 20 seconds for every accepted
   source command to become readable or produce a concrete terminal diagnostic.

Even the 60 total HTTP requests/s pressure stage is far below the requested
3,086.42 average image deliveries/s alone. It can expose regressions and measure
this small fixture, but cannot qualify the requested monthly capacity. That
requires a separately sized deployment/corpus/load-generator exercise after
actual page definitions, bytes/cache distribution, peaks and sync demand exist.

Global caps: 2,500 HTTP requests including retries, 512 MiB response bodies,
32 MiB newly uploaded bytes, eight new chapter commits and one load process.
No automatic retries for uncertain mutations. Reads may retry once within the
same absolute budget; categorize the original response as well.

Stop immediately on an integrity mismatch, private/foreign data exposure,
unexpected authority success, source autoapproval, persistent process growth,
fixture escape, two unexplained 5xx responses or exhausted resource caps.
Expected admission 429 and version 409 responses are measured separately. A
pending outbox at the drain deadline is an inconclusive/failed delivery result,
not successful sustained capacity.

## Measurements and optimization order

For each stage and operation class record offered and achieved request rates,
status counts, p50/p95/p99 TTFB and complete-body latency, response bytes and any
failed semantic assertions. Record signing/bundle serialization separately from
server latency. Sample process RSS/CPU, owned descendant count, shared-pool
occupancy/waits, PostgreSQL waits and incomplete source-outbox age/count.

Record the original/platform worker readiness, cache state and exact database
connection count. Start with the documented deployment pool size; an optional
four-connection sensitivity run is a separate experiment only after correctness
qualifies, not an unnoticed configuration change.

Use the baseline to diagnose before optimizing. First separate request-body/image
CPU, database/pool waiting, signature work, repeated DTO expansion, media admission
and source projection. Inspect the actual slow statements with bounded local
EXPLAIN analysis only if needed. Propose the smallest measured fix, rerun affected
correctness cases, then repeat the same corpus and load stages against a new
frozen commit. Keep the first result as the comparison baseline. Do not infer a
production SLO or maximum user count from a bounded local pressure probe.

Completion requires an honest per-stage result, source/binary/config identity,
semantic invariants and verified shutdown of all owned processes/listeners. If a
gate or runtime blocks execution, report that exact blocker and keep this plan
unexecuted rather than manufacturing a performance result.
