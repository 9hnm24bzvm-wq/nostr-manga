# Public-original health withdrawal (schema35)

Reapplied to the byte-verified v43 source after the execution workspace reset. The original final schema35 aggregate outcome was unavailable; the restored implementation was retested. The full PostgreSQL suite, Clippy and separate-process/restart checks passed in results/restored-schema35-full.log;107Python model tests also passed. The initial health fixture used the wrong read API and then required a Clippy iterator fix; both are corrected in the passing full run. Newly generated evidence uses restored-schema35 filenames.

## Rules

A known damaged public original is journaled for reconciliation, unlinked, and its public directory fsynced before a quarantine record is committed. The read task owns its negative-observation guard through that commit even when the HTTP caller disconnects. Available-object write failures and failed export verification use the same withdrawal rule.

A quarantined page hides every publication requiring that page. Healthy pages of an affected publication may remain temporarily in the public namespace while bounded cleanup is pending. Audit results report public_export_pending. Healthy shared publications and unrelated originals retain their aliases. This is an explicit transition, not atomic removal of all pages of a chapter.

An external-reader hold remains charged after unlink. A managed-family drain releases only the exact pending withdrawal-token set captured before the drain. Repair cannot overwrite an inode still held by readers. Repairing one of multiple quarantined pages must not reveal a publication whose other required pages remain unhealthy.

## Durable work

Migration0035 adds public_export_health_work. Quarantine insertion/removal schedules one generation-bound cursor job. Each quantum visits at most one source publication and64 page references, then feeds the existing16-item projection worker. Role pause stops processing without discarding new changes. A paused role is therefore still a durable subscription to metadata/health changes.

The pre-unlink journal changes matching held state to creating and clears old withdrawal tokens. Caller-triggered observations increment the queue version. The export worker's own verification failure does not increment its current version, so the existing exponential backoff can be acknowledged against the correct work item. Filesystem or database errors retain conservative holds and pending work.

## Recovery and testing

Offline backup excludes derived public aliases and process records. Restore clears only the independent target copy's health-work queue, export queue, role and reader holds. Source records remain intact.

The recreated71-image fixture covers two damaged pages, shared/unrelated originals, a blocked quarantine transaction with caller cancellation, bounded64-page fan-out,64+5 reader-hold drains, partial repair, and complete verified re-export. The queue regression now repairs through the verified lifecycle instead of merely copying bytes over a durable quarantine.

The supervisor's owner-pipe EOF is observed independently of a pending drain command. The cancellation regression uses a four-second slow-reader drain and checks that fresh range requests stop before that drain timeout.

Automatic node serving remains disabled until the startup/read-policy integration and fail-closed namespace-error tests in direct-serving-startup-plan.md pass. These changes make no throughput or production-capacity claim.
