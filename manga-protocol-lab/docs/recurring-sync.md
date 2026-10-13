# Explicit recurring synchronization policies

Schema31 adds operator-controlled recurring policies. Merely approving a peer does not start a schedule. POST /admin/sync-schedules requires spec, interval_seconds and an explicit enabled boolean. The spec uses the same approved peer, inventory, filter and supported fallbacks as a durable sync job. A canonical spec has a stable policy ID; resubmitting it updates its interval/enabled setting and revision.

Intervals are30–86400seconds, with at most32 enabled policies and256 total policy records. The background dispatcher runs only with sync_worker_enabled and the storage/peer worker available. It selects up to4 due policies per second. GET /admin/sync-schedules requires operator NIP98 and returns next-run time, dispatch count/error and the last batch's state/error.

POST /admin/sync-schedules/<id>/enabled accepts an explicit enabled boolean. Disabling serializes with dispatch: no new dispatch occurs after disable completes. It does not cancel already queued or running batches or undo admitted content; the existing batch cancellation endpoint remains separate. A revoked peer cannot dispatch new work, and re-enabling a policy requires current approval. Existing transfer authority checks still fence in-flight work.

A due pass atomically creates or coalesces its durable job and advances the next deadline under a revision check. Concurrent dispatchers cannot duplicate a pass. If validation or queue admission fails, the policy records its bounded error and tries at its next configured interval. The scheduler does not create credentials, approve peers, broaden filters or change moderation.

Scheduled batch history is capped at512 retained batches. On pressure, one unreferenced terminal scheduled batch and its completed partition/job records are removed transactionally before adding another. Current last-batch references and manual-job history are preserved, and pending-item credit must be zero. Manual enqueue of an active scheduled batch marks its history for retention. This is bounded operational history, not an audit archive; export records externally if long-term history is required.

Offline restore disables stored schedules, increments their revisions and records paused_after_restore, in addition to disabling background workers in the restored configuration. An operator reviews and explicitly re-enables them. Restoring an identity does not silently resume periodic replication.

## Evidence

sync-schedules-full-regression.log passed the complete schema31 suite, including:

-24 concurrent dispatch attempts produce one due job; future interval and active-batch deduplication.
- Explicit disable/revocation, interval limits and enabled-policy ceiling.
-512 synthetic completed history rows exercise linked cleanup without deleting manual history or changing pending credit.
- Real HTTP policy creation/inspection is operator-only; an actual background ticker imports a newly published profile.
- A disabled due policy does not dispatch again.
- Actual offline backup/restore preserves the policy but leaves it paused.

Due times/history are seeded in focused tests to avoid pretending a short run establishes long-term timing or capacity. Earlier diagnostic logs include a corrected needless-borrow lint and an artificial negative-infinity fixture timestamp; the schema now requires finite deadlines. No real recurring external service was configured in these tests.
