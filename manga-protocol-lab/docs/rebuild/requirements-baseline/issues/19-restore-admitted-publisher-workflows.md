# 19: Restore admitted publisher workflows

Status: done for the stated acceptance scope; storage lifecycle and interoperability remain separate open tickets.

Audit findings: B1, R1

Prerequisites: 3, 5

## Scope

Complete the accepted feature and related safety-preserving refactor described in [the audit](../../../docs/FEATURE-AUDIT-20261007.md). Work in the authorized chat/cloud computer. PostgreSQL, deadpool-postgres/tokio-postgres and Moka; no SQLx. UI and production deployment are outside scope.

## Acceptance

- [x] An admitted non-admin publisher uploads a manifest and originals and publishes through signed HTTP; WS uses the same publication authority rules.
- [x] Logical publisher grants are independent of physical mutation ownership and bind the exact blob generation; another author cannot consume an ungranted staged original.
- [x] Permission epoch and grant validity are rechecked at the final commit; revocation, cancellation, duplicate uploads and shared hashes have negative/race tests.
- [x] Administrator, managed-group and approved-peer publication regressions still pass.

## Evidence

Pending. Keep reproduced diagnostics distinct from positive acceptance tests.

## Progress

Schema36 adds epoch/generation-scoped publisher grants without changing physical storage ownership. HTTP upload/read/publication and NIP42 EVENT share domain authority checks. The focused real PostgreSQL/strfry fixture passes with isolation/revocation/expiry negatives. See docs/publisher-authority.md and results/audit-publisher-regression-2.log.

Combined regression passed: results/audit-publisher-full-regression.log, including existing group/peer behavior, Clippy and PostgreSQL process/restart checks. The change adds no new load-test result.
