# 22: Complete Blossom browser interoperability

Status: done for the explicitly restricted server-scoped/private-staging profile. No claim of full or unrestricted Blossom conformance.

Audit findings: B3

Prerequisites: 19, 20

## Scope

Complete the accepted feature and related safety-preserving refactor described in [the audit](../../../docs/FEATURE-AUDIT-20261007.md). Work in the authorized chat/cloud computer. PostgreSQL, deadpool-postgres/tokio-postgres and Moka; no SQLx. UI and production deployment are outside scope.

## Acceptance

- [x] A documented supported BUD subset defines standard upload, lookup and authorized deletion routes and signed-token scope.
- [x] Browser preflight, allowed origins and response-proof header exposure are configurable and tested.
- [x] Cross-origin handling does not grant publication, operator or deletion authority; negative origin and authentication tests pass.
- [x] A real client contract fixture exercises original upload/read/delete plus authenticated library requests.

## Evidence

docs/blossom-profile.md; results/http-blossom.json; results/audit-blossom-http-4.log; results/audit-blossom-full-regression.log. HTTP clients exercise actual node behavior, not a production browser/reverse-proxy deployment. Optional BUD extensions remain outside this subset.
