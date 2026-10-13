# 26: Complete series metadata discovery

Status: pending. No implementation or passing evidence is claimed by creating this ticket.

Audit findings: F5, R6

Prerequisites: 6, 23

## Scope

Complete the accepted feature and related safety-preserving refactor described in [the audit](../../../docs/FEATURE-AUDIT-20261007.md). Work in the authorized chat/cloud computer. PostgreSQL, deadpool-postgres/tokio-postgres and Moka; no SQLx. UI and production deployment are outside scope.

## Acceptance

- [x] Typed series/title/category contracts preserve accepted free-form categories and instance-local synonyms.
- [x] Public series and chapter discovery use parameterized bounded queries with accepted timestamp, ordering and three-valued filter semantics.
- [x] Cover and remaining metadata contracts are resolved explicitly; optional unknown fields are not silently reclassified as invalid.
- [x] Import round trips and positive/negative filter/search fixtures cover supported metadata.

## Evidence

See docs/catalogue-and-covers.md, results/catalogue-full-regression.log, catalogue-upgrade-guard.log, catalogue-cover-review.log, catalogue-migration-python.log and http-catalogue.json.
