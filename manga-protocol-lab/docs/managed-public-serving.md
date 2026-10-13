# Managed public-original serving

Status: implemented behind explicit configuration; the final full PostgreSQL regression, Clippy and process/restart checks passed in results/serving-role-full-final-2.log. Schema remains35. The website/UI and public deployment are outside this checkpoint.

## Configuration

Keep the existing storage and backend roles. Add the following node configuration, using absolute paths for helper and nginx and an unprivileged service account:

```json
{
  "read_policy": "public",
  "public_serving": {
    "enabled": true,
    "python": "python3",
    "helper": "/srv/manga/src/serving_supervisor.py",
    "nginx": "/srv/manga/vendor/nginx-1.28.0/objs/nginx",
    "bind": "127.0.0.1:19670",
    "workers": 2,
    "shutdown_seconds": 30
  }
}
```

The node and image reader bind loopback. An independently configured TLS reverse proxy can route public hash.ext image requests to this listener. Never expose raw strfry. Never route guarded image traffic to the public listener. Do not run another nginx or unmanaged process against the derived public tree: its file descriptors would be outside the managed-reader proof.

Direct requests use /<64-character-lowercase-SHA256>.png, .jpg, or .webp. Only retained verified original image aliases are created. Manifests, staging objects, group keys and library blobs do not become public files here. Successful image reads use nginx directly, bypassing PostgreSQL/Moka/Rust on that request path. This is architectural behavior, not a new throughput benchmark or target-scale certification.

## Startup and policy changes

The helper acquires the exact storage/public/.serving lock and recovers any recorded old reader family before database connection or node HTTP startup. It stays prepared without binding the image port. Public activation happens only after bounded export/backfill/health work has converged. Verified HTTP remains available while initial public backfill proceeds.

To switch to authenticated or admitted reads, retain the same public_serving configuration and set enabled=false. The helper stops old readers and keeps the runtime lock while guarded HTTP operates. Removing the recovery configuration while managed state exists fails startup. Enabling direct serving with a guarded policy also fails startup.

Recovery validates owner, binary path, boot ID, PID namespace and process birth identities. Host/namespace moves and ambiguous identities remain conservative failures requiring offline review. It does not infer safety from a dead nginx master, elapsed lease or missing HTTP listener.

## Failure behavior

Known storage verification failure notifies the serving owner. Unproven namespace work, database errors/deadlines, or lost helper/master health stop the direct reader. The live node can launch an idle recovery helper after a dead/poisoned supervisor to stop recorded orphan workers; it never activates that replacement during error recovery. Other verified HTTP endpoints can remain available.

A failed direct-serving role remains closed until recovery and a node restart. It does not repeatedly reopen on a timer. This first implementation deliberately favors safe failure over availability after exceptional storage faults.

A reconciliation quantum has a10-second outer deadline; individual database sessions also have statement/lock limits. Reader operations have shutdown-derived deadlines. Slow existing responses can require the configured drain interval. Unlink completion does not refund storage: committed generation/root/token-bound holds remain until the exact reader drain is verified.

GET /admin/status (operator-authorized) includes public_serving.enabled and state: prepared, activating, active, stopping, stopped, failed_closed or shutdown_unconfirmed. These are operational observations, never authorization or permission to reclaim an inode. A shutdown_unconfirmed result requires investigation; it is not a successful drain.

## Evidence

- serving-prepared-regression-2.log: prepared/no-listener ownership, activate/suspend, cancellation during a slow drain, and a killed master with multiple orphan workers.
- http-public-serving-regression-2.log: real node automatic export, live helper-loss recovery, orphan recovery before guarded HTTP, missing/mismatched configuration refusal, filesystem withdrawal failure with charged hold, startup deletion reconciliation and recovery before unavailable PostgreSQL.
- serving-role-full-final-2.log: final integrated suite and lint/process gates. serving-role-lifecycle-final.log and serving-role-process-final.log: actual helper/master crash, owner EOF and reader-family recovery. serving-role-python.log:107Python model tests.

The feature uses the same bounded queues and physical owner as the small deployment. Multiple independently replicated instances remain separate owners. This is not proof that a single VPS meets100TB,8B monthly images or300M monthly progress saves.

## Typed recovery diagnostics

The operator-only public_serving status now reports fault separately from state. fault is null until failure, then one of storage_verification, namespace_reconciliation, reader_drain, activation, reader_health or task_exited. shutdown_unconfirmed remains a phase, so it does not erase the original fault. automatic_reactivation is false; a recovered configuration must still pass the existing startup/reader and storage checks. These fields never authorize deleting files or clearing reader holds.

Actual nginx/node tests distinguish helper-health loss from failed namespace withdrawal. The latter closes serving and retains its original hold until verified recovery; normal guarded HTTP remains separately governed. The combined functional regression passes; see results/audit-recovery-tls-full.log. Foreign boot/PID namespace refusal remains intentional: do not kill or assume ownership of unfamiliar PIDs, remove state files to bypass the guard, or clear holds based only on elapsed time.
