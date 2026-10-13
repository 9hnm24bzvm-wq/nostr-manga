# 28: Build MangaDex compatibility surface

Status: done for the explicitly documented reference profile and native signed account/library workflow. Unchanged third-party client compatibility is not certified.

Audit findings: F4

Prerequisites: 25, 26, 27

## Scope

Complete the accepted feature and related safety-preserving refactor described in [the audit](../../../docs/FEATURE-AUDIT-20261007.md). Work in the authorized chat/cloud computer. PostgreSQL, deadpool-postgres/tokio-postgres and Moka; no SQLx. UI and production deployment are outside scope.

## Acceptance

- [x] An explicit supported-client/endpoint matrix covers agreed browse/search/chapter/images, account/list/progress and publishing/group launch workflows.
- [x] Authentication and IDs are translated without reusing MangaDex credentials or inferring group control from display names.
- [x] Existing supplied-export migration retains exact originals and explicit public-history consent.
- [x] End-to-end compatibility fixtures establish advertised behavior; unsupported endpoints have honest explicit responses.

## Evidence

See docs/mangadex-compatibility.md for supported endpoints/client requirements and exclusions. results/compatibility-full-final.log passes62 groups/Clippy/process restart;115 Python tests pass; actual migration and mapped group/content routes are exercised. Failed runs remain separate.
