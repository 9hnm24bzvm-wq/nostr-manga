# Per-item synchronization outcomes (schema41)

Status: focused and combined functional regressions pass. Source archive/release verification is included in the checkpoint process.

## Contract

Known unsupported content or local content policy does not consume five retry attempts and cancel unrelated partitions. A typed rejection settles that queued item and allows the remaining batch to continue. Categories are `unsupported`, `policy_rejected` and `invalid`, with stable reason codes rather than matching error strings.

Covered reasons include publication/library pointer versions, manifest/object versions, required features, unsupported lifecycle tags, local original format/size policy, local publication removal and invalid supported-profile manifests/originals/library objects. Public-library routing validates the signed Nostr envelope and exact requested identity without requiring the local root codec merely to identify the author. Actual content admission still performs its schema, signature, hash, policy and ownership checks.

Transport, storage, metadata-store, approval and unexpected failures remain ordinary retryable failures. They are not silently converted into successful skips. Pending projections retain their existing retry/deadline behavior. A real terminal operational failure can still fail a batch.

A batch with skips ends as `completed_with_skips`, not `completed`. Its status contains:

- `acknowledged_items`: processed queued items, including filtered/skipped items; not a hosted-publication count.
- `skipped_items` and `skip_counts`: complete aggregate counts by category/reason.
- `skip_details`: up to256 diagnostic event IDs and reasons per batch.
- `skip_details_limit` and `skip_details_truncated`: explicitly disclose the sample bound and omitted details.

Counts and samples are read from one database statement/snapshot. Skip settlement, pending-item budget updates and counters are atomic under the work lease. Duplicate acknowledgement does not double-count, and a cancelled/completed lease cannot mutate results. Scheduled history pruning recognizes the new terminal state and cascades its bounded detail rows.

The reference direct publication/library sync APIs return422 with an explicit category/reason and `hosted:false` for a typed content rejection. This is a refusal to host that item, not an implicit deletion or network-wide policy.

## Verification

- `results/audit-sync-mixed-peer-4.log` / `results/http-sync-skips.json`: two real wrapper/strfry peers, with fixture-seeded future/invalid signed records to model another implementation. An unsupported event precedes both good chapters. Two good chapters transfer and project; future pointer/manifest versions, unknown required features, receiver-disallowed valid JPEG, mismatched image dimensions and a receiver-moderated removal are skipped. Two future library pointers settle independently. Queue accounting and temporary holds drain.
- `results/audit-sync-skip-accounting.log` / `results/sync-skip-accounting.json`:300 skips,256 detailed samples, explicit truncation, duplicate accounting, cancellation/completion fencing and retryable-error distinction.
- Typed parser unit checks distinguish malformed versions/required fields from unsupported contracts and keep unknown optional fields optional.

The first mixed-peer attempts exposed a source routing codec dependency and failed. Their logs are retained; the later passing run supersedes them. This is functional compatibility/failure testing, not load qualification. The bounded sample is deliberately not presented as a complete per-event history for arbitrarily large batches.

Combined evidence: results/audit-sync-full-regression.log and audit-sync-python.log, including existing peer/group/storage/serving behavior, Clippy, PostgreSQL process/restart checks and109 Python tests.

## Synchronous temporary-source errors (October 11 reconstruction)

The operator endpoints `/admin/sync`, `/admin/sync-library` and `/admin/sync-account` return HTTP503 with exactly `{"error":"peer_temporarily_unavailable","retryable":true}` when an approved source has a recognized temporary failure. Recognized failures are HTTP408,429 or5xx, typed connection/request timeouts, an interrupted HTTP response body, and a discovered endpoint's typed DNS timeout. The learned/pinned endpoint proof preserves the HTTP status classification without relaxing destination or identity verification.

Publication transfer keeps its existing order and bound of eight independently approved sources. A successful approved fallback still succeeds. If none supplies the required verified event or blob and at least one failed temporarily, the result is operational unavailability. Verified schema/content-policy rejections retain422; malformed requests, invalid identity/proofs, unapproved/revoked sources and other established validation/quota outcomes retain their existing4xx behavior. Local sync-slot exhaustion remains429. Unknown exceptions are not blanket-converted to503. A definitive nontransient HTTP status is rejected before reading its error body, so an interrupted403/404 body cannot turn that rejection into503.

The temporary-source response does not assert `hosted:false`: the same event may already be hosted, or an attempt may have staged only part of its objects. It exposes no upstream endpoint, key, response body or database details. No caller POST, publication commit or storage write is automatically replayed. An operator may explicitly resubmit the same immutable event ID with fresh signed HTTP authorization/nonce. Existing approval epochs, staging holds and idempotent publication fences still apply. Durable jobs keep their existing bounded retry policy; the new operational type is never a permanent item skip.

The existing bounded-body helper distinguishes a transport interruption from a complete but invalid or over-budget response. Only the transport variant is retryable. Complete short bodies, declared-size/streaming-budget violations and complete corrupt content retain their verification failures. No classification matches error strings. Non-timeout DNS I/O errors are not broadened into retryable outcomes by this change.

The focused real-peer regression is `server/tests/http_peer_transient.rs`; causal observations and build identities are retained in `results/rebuild-peer-transient/`. Those focused checks are not a production-load or universal-reliability claim. Historical combined evidence above does not certify this reconstructed change; the current combined gate is recorded separately.
