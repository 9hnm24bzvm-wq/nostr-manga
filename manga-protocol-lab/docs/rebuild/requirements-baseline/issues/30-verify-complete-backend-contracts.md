# 30: Verify complete backend contracts

Status: reopened for complete accepted reader/group workflows. The earlier narrowed reference-profile completion did not cover the promised account/list/progress facade. Tickets31/32 and a final combined regression/checkpoint are required. Deployment capacity and unchanged external clients remain explicitly unqualified.

Audit findings: R4, R8, R9, O3

Prerequisites: 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29

## Scope

Complete the accepted feature and related safety-preserving refactor described in [the audit](../../../docs/FEATURE-AUDIT-20261007.md). Work in the authorized chat/cloud computer. PostgreSQL, deadpool-postgres/tokio-postgres and Moka; no SQLx. UI and production deployment are outside scope.

## Acceptance

- [ ] A requirement-to-wire-contract-to-implementation-to-evidence matrix covers every accepted in-scope feature and keeps unresolved choices explicit.
- [ ] Large HTTP-module extraction and supervised-worker changes follow tested domain boundaries rather than speculative rewrites.
- [ ] Functional regression, security-negative, crash/restart, TLS/reverse-proxy and cold-device recovery scenarios cover final code.
- [ ] A source checkpoint rebuilds and is backed up; incomplete features and unproven capacity remain explicitly stated.

## Evidence

See docs/RELEASE-20261008.md and the supported-client/endpoint matrix in docs/mangadex-compatibility.md. The62-group final regression,115 Python tests,Clippy,process/database restart,TLS boundary,cold bootstrap,release/demo and extracted-source rebuild pass. Source is packaged with a complete hash inventory; failed runs remain distinct.
