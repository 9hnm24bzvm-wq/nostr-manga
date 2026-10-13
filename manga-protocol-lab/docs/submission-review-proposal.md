# Submission review: implemented schema45 contract

Status: schema45 complete functional regression passes (58 result groups), Clippy, independent-process/PostgreSQL restart checks and110 Python tests. This is feature evidence, not a complete-system release gate.

## Authority and workflow

An instance-admitted account may declare its own signed immutable series/chapter event and canonical manifest. This does not grant ordinary publisher permission. Pending records and originals are private to the owner and local operator. NIP98 protects the HTTP operations; review introduces no new Nostr kind, auth provider, revision links, or public actor tags.

Only declared hashes with exact expected sizes may be staged. Generation-specific review grants protect pending originals against expiry cleanup and direct deletion. Approval validates every original and current account epoch, then atomically commits the decision, hosted retention and projection intent. Failure rolls all three back. A retry of an approved decision is idempotent. Rejection/withdrawal/expiry closes that submission permanently; one-off approval never confers ongoing publisher permission.

Approve/reject decisions serialize by event ID with publication checks. A rejected event cannot subsequently be republished through an ordinary publisher, group or peer path. Rejection does not itself unhost a copy that was already published through a separately authorized path: protected deletion remains a separate moderation operation. Withdrawal and expiry are not global content bans. Decisions remain instance-local.

## Bounds and cleanup

Default pending limits are4 per account and128 per instance; declared pending-byte limits are512MiB and8GiB respectively. Each submission permits at most256MiB,1024 objects and7 days pending lifetime. Retained review records have a10,000 intake cap; capacity exhaustion fails closed. Configuration may override validated bounds or disable review with submission_review:null. Physical upload/storage budgets also apply, and shared physical bytes are accounted through the existing storage service.

Expired reviews are closed by a tracked bounded worker (64 rows per pass). Rejection and withdrawal remove only their own interests. Rejection accelerates already queued cleanup. Shared pending interests, hosted references and reader/transfer protections continue to prevent reclamation. Physical removal and refund use durable receipts; there is no early accounting refund. Records/audit outcomes are not automatically purged or tamper-proof.

## HTTP interface

- POST /submissions: signed event plus base64 canonical manifest
- GET /submissions and GET /submissions/{id}: owner queue and detail
- POST/GET /submissions/{id}/blobs/{hash}: declared original upload/private read
- POST /submissions/{id}/withdraw
- GET /admin/submissions and GET /admin/submissions/{id}: operator inbox/detail
- POST /admin/submissions/{id}/decision: approve/reject and optional reason

Lists use bounded event-ID keyset pagination (maximum64), with an optional pending filter. Reasons are limited to4096 bytes. Private blob responses use private, no-store. Authority denials return403, private absent/undeclared objects404, closed decisions409, intake budgets429 and operational database/storage failures503. Invalid content returns400. These are API contracts; website presentation remains out of scope.

## Evidence

- http_submission_review: real HTTP/PostgreSQL/strfry publication; admission/privacy, missing originals, quota, revocation, expiry, idempotence, injected approval transaction failure, and decision persistence across restart.
- submission_review_fences: pending holds, rejection cleanup, shared-interest preservation, approval retention, resurrection rejection and eight concurrent approve/reject races.
- results/audit-submission-review-http-2.log and audit-submission-review-fences.log include Clippy plus independent-process/PostgreSQL restart checks.
- results/http-submission-review.json and submission-review-fences.json record checks.

The expanded HTTP negative checks and complete combined regression pass; see results/audit-submission-review-full.log and its exit-code file. Synthetic functional fixtures do not qualify production capacity, every crash point, or every network interruption.
