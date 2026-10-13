# 20: Complete staged blob lifecycle

Status: done for the stated reference lifecycle acceptance scope.

Audit findings: B2, B3, R2

Prerequisites: 19, 12

## Scope

Complete the accepted feature and related safety-preserving refactor described in [the audit](../../../docs/FEATURE-AUDIT-20261007.md). Work in the authorized chat/cloud computer. PostgreSQL, deadpool-postgres/tokio-postgres and Moka; no SQLx. UI and production deployment are outside scope.

## Acceptance

- [x] Bounded maintenance selects expired available never-published uploads; resumed work is idempotent and observable.
- [x] Authorized direct deletion of unreferenced uploads uses the existing durable receipt and accounting path.
- [x] Hosted publications, retained library objects, live grants, mutation generations and external-reader holds prevent premature deletion or refund.
- [x] Publisher and group grant expiry/release policies are explicit and tested; interrupted unlink/fsync/receipt and active publication races retain safety.

## Evidence

Pending. Keep reproduced diagnostics distinct from positive acceptance tests.

Focused evidence: results/audit-transfer-holds-regression.log and results/staging-reclamation.json. See docs/staging-reclamation.md. Do not mark the ticket done until authenticated deletion and shared-interest cases pass.

Combined evidence: results/audit-staging-full-regression.log and audit-staging-python.log. Ticket remains in progress because direct deletion is not implemented.

Schema40 closes authenticated deletion: dedicated domain/HTTP fixtures and combined regression pass. Earlier progress entries are historical; standard Blossom/CORS remains ticket22.
