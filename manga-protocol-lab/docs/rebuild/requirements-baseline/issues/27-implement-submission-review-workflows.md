# 27: Implement submission review workflows

Status: done for the admitted-account, instance-operator reference policy. Schema45 full functional regression passes.

Audit findings: F6

Prerequisites: 19, 20

## Scope

Complete the accepted feature and related safety-preserving refactor described in [the audit](../../../docs/FEATURE-AUDIT-20261007.md). Work in the authorized chat/cloud computer. PostgreSQL, deadpool-postgres/tokio-postgres and Moka; no SQLx. UI and production deployment are outside scope.

## Acceptance

- [x] Unadmitted submissions have a bounded validated review path under local policy rather than automatic publishing authority.
- [x] Approval, rejection and expiry enforce publisher authority, original possession, storage budgets and durable audit outcomes.
- [x] Managed group backup and recovered-public-key registration have a usable offline runbook and verification fixture.
- [x] Action attribution requirements are resolved; portable membership export is not introduced as a mandatory protocol feature.

## Evidence

Group backup: docs/group-key-backup-recovery.md, results/audit-offline-keys-unit.log, results/audit-offline-keys-http.log. Local audit: docs/group-action-audit.md, results/audit-group-attribution-http.log and audit-group-attribution-fences.log. Submission review: docs/submission-review-proposal.md, results/http-submission-review.json, results/submission-review-fences.json and audit-submission-review-full.log. All58 result groups, Clippy, process/restart checks and110 Python tests pass.
