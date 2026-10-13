# 24: Implement signed service discovery

Status: done for the accepted signed HTTP discovery/approved-peer relocation contract. Optional WebSocket projection is not claimed.

Audit findings: F1

Prerequisites: 23

## Scope

Complete the accepted feature and related safety-preserving refactor described in [the audit](../../../docs/FEATURE-AUDIT-20261007.md). Work in the authorized chat/cloud computer. PostgreSQL, deadpool-postgres/tokio-postgres and Moka; no SQLx. UI and production deployment are outside scope.

## Acceptance

- [x] A service-key-signed replaceable endpoint list uses deterministic timestamp/ID selection and peer caching/exchange.
- [x] Expected keys are verified; first possession-verified identity is remembered when no expected key is supplied; a new key never inherits peer permissions.
- [x] URL safety, expiration/staleness and replay rules are explicitly resolved before implementing automatic endpoint use.
- [x] Domain move, transferred domain, old announcement replay and cold-peer discovery have tests.

## Evidence

Focused HTTP tests pass: results/audit-service-resolution-http.log and results/http-service-announcements.json. See docs/service-identity-remembrance.md for limits. The combined regression, Clippy, process/restart fixtures, release build and extracted archive rebuild pass.
