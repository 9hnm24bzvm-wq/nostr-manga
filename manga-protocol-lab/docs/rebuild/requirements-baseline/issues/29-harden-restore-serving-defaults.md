# 29: Harden restore serving defaults

Status: pending. No implementation or passing evidence is claimed by creating this ticket.

Audit findings: O1, O2

Prerequisites: 14, 15

## Scope

Complete the accepted feature and related safety-preserving refactor described in [the audit](../../../docs/FEATURE-AUDIT-20261007.md). Work in the authorized chat/cloud computer. PostgreSQL, deadpool-postgres/tokio-postgres and Moka; no SQLx. UI and production deployment are outside scope.

## Acceptance

- [x] Restored configuration disables managed public serving until deliberate review/re-enable, alongside existing worker reset conventions.
- [x] An enabled-source serving backup/restore fixture covers path/port changes and preserves reader-hold safety.
- [x] Fault recovery distinctions are typed and observable; no timer alone clears holds or reopens serving.
- [x] Foreign host/namespace recovery limits remain documented unless a separately tested safe transition is implemented.

## Evidence

The current combined regression passes61 result groups plus Clippy, process/database restart and110 Python tests. See results/audit-recovery-tls-full.log, audit-recovery-diagnostics-http.log, audit-recovery-supervisor.log and audit-recovery-diagnostics-python.log.

## Verified partial progress

2026-10-07: explicit disabled-on-restore configuration policy implemented. Eight Python safety tests and the isolated PostgreSQL/node migration/restore fixture pass, including preserved source configuration and paused restored role. Evidence: `results/audit-restore-policy-unit.log`, `results/audit-restore-policy-regression.log`. Subsequent typed fault diagnostics and the explicit foreign boot/PID-namespace refusal now have passing evidence. Cross-host process control remains intentionally excluded; independent-copy restore and offline review are the supported path.
