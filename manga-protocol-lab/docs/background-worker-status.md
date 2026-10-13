# Background worker lifecycle visibility

The operator-authenticated GET `/admin/status` now reports named long-lived background workers with `running`, `returned`, `cancelled` or `panicked` state, start/finish timestamps and an aggregate `unexpected_exit` flag. Entries are limited to 16 statically named roles; disabled/unconfigured roles are absent. Failure messages contain a role name and state, not panic payloads.

Tracked roles are staged-upload recovery, blob reclamation, library-history pruning, storage audit, backend projection, authorization expiry, sync scheduling, sync jobs and relay notifications, when configured. Managed public serving retains its separate existing status. Per-request owned commit/cleanup tasks are intentionally not converted into cancellable background roles.

A `running` state is only task-lifecycle information. It does not establish forward progress, database health, completed replication, durability or target throughput. Operational errors already retried inside a loop remain governed by that loop. This change does not automatically restart failed tasks, resume failed-closed public serving or replace durable recovery procedures.

The relay notification role now has drop guards: unexpected cancellation/panic clears its readiness flag and notifies subscribers to resynchronize. Its database notification driver is aborted when the owning connection future leaves, including early-error/cancellation paths. This closes a cleanup gap where an abandoned driver could otherwise outlive the parent future or readiness could remain stale after a role exit.

Evidence: lifecycle unit tests deliberately exercise normal return, panic, cancellation and a still-running task; feed guard tests check readiness reset/subscriber notification and driver cancellation. The actual metadata-node HTTP fixture checks operator-authenticated status, enabled role names and absence of storage roles. Full relay/restart/regression evidence is recorded after the aggregate run.
