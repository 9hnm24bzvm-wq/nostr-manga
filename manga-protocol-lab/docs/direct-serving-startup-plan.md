# Managed serving integration plan

Status: implemented as an explicit opt-in role; final integrated regression is in progress. See managed-public-serving.md for the actual configuration and conservative failure behavior.

## Startup ownership

1. Derive the public tree from the configured storage root, never a second arbitrary data path.
2. An optional node serving configuration includes an explicit enabled flag and pinned helper/nginx paths. Enabled serving requires the public read policy and local publication/storage roles.
3. Before storage maintenance or HTTP startup, start the supervisor in prepared mode: acquire the existing public-tree owner lock, validate saved process identities, stop any previous reader family, and remain idle while holding ownership. Preparation must never bind the image listener.
4. Missing serving configuration with an existing managed runtime fails startup. Switching to authenticated/admitted reads requires enabled=false with the existing recovery configuration. An uncertain process identity or failed recovery fails startup.
5. Public mode reconciles durable export work in bounded quanta while the image listener remains closed. Only a successful idle check allows activation. Normal verified HTTP can remain available during backfill.
6. Disabled mode keeps the supervisor idle for the process lifetime, preventing another managed reader from starting. Existing holds remain conservative until safe withdrawal/drain cleanup is implemented; no quota refund follows merely from disabling the listener.

## Runtime ownership

- The supervisor gains prepare/activate/suspend operations using the existing private pipe and nonce checks. An idle-family drain may prove that no prior readers remain, using a separate explicit inactive marker rather than pretending to have worker identities.
- Failed namespace reconciliation suspends direct serving before retries. A failed withdrawal from a read/audit/write observation notifies the serving owner immediately. Known unhealthy metadata must not be acknowledged as safely withdrawn until its namespace operation completes.
- Activation after an error requires complete reconciliation, not just expiration of a timer.
- Node shutdown owns cancellation and supervisor termination. Detached worker tasks must not keep the image listener running after the HTTP node exits.
- Database failures prevent activation; errors while active suspend the direct-serving family. They cannot refund physical storage.

## Tests before enablement

- Prepared helper does not bind; activation serves; suspension closes listener and drains slow readers.
- Node with public mode serves exact originals without database access on the image request path.
- Pending startup deletion/quarantine prevents stale aliases being exposed before reconciliation.
- Public-to-guarded restart stops old workers before the guarded HTTP listener opens.
- Missing recovery configuration and foreign/invalid process state fail closed.
- Node death, owner EOF during drain, killed helper, and orphaned master/worker recovery.
- Filesystem unlink failure and database outage close direct serving and retain charges.
- Read-policy mismatch is refused before any public image listener starts.

This is component and lifecycle correctness work, not evidence for the eventual100TB or monthly request/save targets. Operator-installed reverse proxies and unmanaged readers remain outside the managed family proof.
