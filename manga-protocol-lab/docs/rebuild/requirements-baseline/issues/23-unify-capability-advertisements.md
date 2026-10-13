# 23: Unify capability advertisements

Status: done. Runtime registry and enforcement-linked limits verified.

Audit findings: F2, R5

Prerequisites: 17

## Scope

Complete the accepted feature and related safety-preserving refactor described in [the audit](../../../docs/FEATURE-AUDIT-20261007.md). Work in the authorized chat/cloud computer. PostgreSQL, deadpool-postgres/tokio-postgres and Moka; no SQLx. UI and production deployment are outside scope.

## Acceptance

- [x] One typed registry drives namespaced NIP11 and HTTP application capabilities.
- [x] Endpoint, API/schema version, configured original format and query/body/admission limits match actual runtime behavior.
- [x] Unsupported versions and budget overruns produce explicit stable responses; advertised support has positive and negative contract tests.

## Evidence

Real HTTP: results/audit-capabilities-http-2.log and results/http-capabilities.json. Combined regression: results/audit-capabilities-full-regression.log. Contract and explicit compatibility limitations: docs/capability-registry.md. This registry does not claim all internal worker/database budgets or target capacity are exposed or qualified.
