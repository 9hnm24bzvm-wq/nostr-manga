# 21: Isolate unsupported synchronization items

Status: done for typed rejection isolation and explicitly bounded reporting; full regression passes.

Audit findings: B4, R3

Prerequisites: 8, 9

## Scope

Complete the accepted feature and related safety-preserving refactor described in [the audit](../../../docs/FEATURE-AUDIT-20261007.md). Work in the authorized chat/cloud computer. PostgreSQL, deadpool-postgres/tokio-postgres and Moka; no SQLx. UI and production deployment are outside scope.

## Acceptance

- [x] Typed per-item outcomes distinguish unsupported contract, local-policy rejection, invalid content and retryable failures without matching error strings.
- [x] A mixed supported/unsupported inventory continues supported items and records durable bounded reasons for skipped items.
- [x] Batch status exposes skips/partial completion; unsupported items cannot exhaust retries and cancel unrelated partitions.
- [x] Lease, peer-revocation, cancellation, queue accounting and transient failure behavior retain explicit tests.

## Evidence

Pending. Keep reproduced diagnostics distinct from positive acceptance tests.

See docs/synchronization-item-outcomes.md for typed rejection scope, explicit partial completion and the256-detail reporting bound. Focused evidence: audit-sync-mixed-peer-4.log and audit-sync-skip-accounting.log.

Final combined evidence: results/audit-sync-full-regression.log and audit-sync-python.log. The256-record sample is not represented as a full detailed history.
