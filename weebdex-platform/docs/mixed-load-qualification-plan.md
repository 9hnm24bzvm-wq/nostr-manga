# Bounded mixed-load qualification plan

Status: prepared, not executed. Correctness and feature integration take priority.
This is a local backend experiment proposal, not a production capacity promise.

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

## Owned environment and corpus

Use one disposable loopback-only PostgreSQL database, one platform process and
one real embedded original strfry worker sharing the platform's configured pool.
Use the actual final release binary and record its source, lockfile, binary and
configuration hashes. Never target an existing user database, public relay,
production endpoint or third-party service. No runtime database, signer file,
authentication proof, cookie or personal data may enter exported evidence.

Seed 32 titles, 256 readable chapters and eight page positions per chapter using
64 generated, mixed PNG/JPEG originals. Include repeated original hashes within
one chapter and across chapters under the same owned source storage authority.
Use nontrivial images up to two megapixels, with a 32 MiB unique-original cap and
128 MiB total fixture-storage cap. Separately create eight bounded open upload
sessions and 16 explicitly admitted/bound synthetic accounts. Private read-marker,
status and list data must be real client-signed original library roots.

Record the exact corpus dimensions and byte sizes. Synthetic-image throughput
must not be described as licensed Ubunchu or real-user-image acceptance.

## Completed HTTP paths to exercise

Discover all UUIDs, source event hashes, current versions and media names from
actual successful API responses. Never substitute a native ID in a public path.
Use fresh exact NIP98 method/URL/payload/service/nonce proofs for authenticated
requests; serialize multipart bodies once before signing. Writes for each account
remain serial so artificial same-account lock contention does not dominate.

Target this per-request distribution, measured after workflow expansion:

- 50% reader media: fetch `GET /api/v1/at-home/server/{chapter}`, then use its
  advertised base URL, hash and file names for original/optimized page GETs.
  Include 15% HEAD, 15% valid byte ranges and conditional validators within this
  class. Validate expected status, length and byte digest, not just HTTP 200.
- 25% catalogue: `GET /api/v1/manga`, `/manga/{id}`, `/manga/{id}/feed`,
  `/manga/{id}/aggregate`, `/chapter/{id}` and `/statistics/manga/{id}` with
  contract-supported filters, pagination and include sets. Mix hot and cold IDs.
- 15% signed account/library reads: `/api/v1/auth/check`, `/manga/{id}/read`,
  `/manga/read`, `/manga/status`, `/user/list`, `/list/{id}` and `/list/{id}/feed`.
  Confirm actor scoping and explicit source head consistency.
- 5% signed original-library writes: use the existing client-signed bundle flow
  at `/compat/mangadex/nostr/library/batches` and `/compat/mangadex/nostr/libraries`
  on this fixture's original listener. Follow the current signed head and verify
  read-your-writes through the public adapters. Plain MangaDex POST read/status
  mutation routes that intentionally return 501 are not counted as implemented
  writes.
- 3% upload workflow calls: `/api/v1/upload/begin`, POST upload body,
  `/api/v1/nostr/uploads/{session}` stage/snapshot, signed preview/replacement,
  and `/api/v1/upload/{session}/commit`. Limit to eight completed new chapters,
  four pages each, two genuine replacements and two discarded sessions. Observe
  truthful pending receipts and eventual readable materialization.
- 2% versioned chapter annotations and local removal requests on separate owned
  fixture targets. Use the final chapter interface contract; do not mix those
  targets with reader availability assertions. Source deletion remains a review
  request; no automatic source approval belongs in the load mix.

Wrong-signature, replay, foreign-owner and stale-version probes are a separate
small correctness stream with expected 401/403/404/409 responses. They must not
inflate success throughput, error rates or overload statistics.

## Finite stages and limits

Use an absolute command deadline of 180 seconds and the existing birth-tracked
fixture cleanup. Keep setup below 30 seconds and teardown below 20 seconds.
Do not extend a running command or launch a second overlapping load job.

1. Smoke: 10 seconds, one active user, cap two requests/second.
2. Baseline: 20 seconds, four users, cap 12 requests/second.
3. Mixed: 30 seconds, 12 users, cap 30 requests/second.
4. Pressure probe: 20 seconds, 24 users, cap 60 requests/second. Run only if the
   earlier stages have no unexplained functional errors or rising backlog.
5. Drain: stop new mutations, then wait at most 20 seconds for every accepted
   source command to become readable or produce a concrete terminal diagnostic.

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
