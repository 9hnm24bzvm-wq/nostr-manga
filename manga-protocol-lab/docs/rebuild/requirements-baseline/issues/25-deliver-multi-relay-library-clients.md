# 25: Deliver multi-relay library clients

Status: done for the signer-agnostic reference Rust SDK and same-origin root-relay profile; website/browser/mobile adapters remain outside current scope.

Audit findings: F3, R7

Prerequisites: 11, 22, 24

## Scope

Complete the accepted feature and related safety-preserving refactor described in [the audit](../../../docs/FEATURE-AUDIT-20261007.md). Work in the authorized chat/cloud computer. PostgreSQL, deadpool-postgres/tokio-postgres and Moka; no SQLx. UI and production deployment are outside scope.

## Acceptance

- [x] A signer-agnostic client discovers selected write relays from NIP65 and sends identical signed chunked-library heads plus required objects.
- [x] First acceptance completes the initial save while other recipients retain separate bounded retries and visible outcomes.
- [x] A fresh device recovers the newest valid library from surviving write relays with deterministic replacement validation.
- [x] Offline recipients, conflicting heads, interrupted object delivery, revoked access and corrupted responses are tested without private-library scope.

## Evidence

See docs/library-client-delivery-contract.md, results/audit-library-client-relays-http.log, audit-library-client-negative-2.log and audit-library-client-outbox-process-2.log. Final combined regression, Clippy, process/restart, release and archive rebuild pass; see audit-library-client-full-regression.log.

Cold bootstrap now has an actual surviving-node workflow: recover the signed NIP65 list from one seed, automatically verify root-route bindings, and recover the exact signed library graph while reporting the offline route and unsupported path. See results/audit-cold-bootstrap-pipeline-2.log. The initial attempt correctly rejected a stale fixture relay-list timestamp; the corrected fixture publishes a newer list. No private key is persisted by the client outbox.
