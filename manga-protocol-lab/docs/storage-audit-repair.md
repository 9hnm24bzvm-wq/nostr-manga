# Bounded storage audit and verified repair

`POST /admin/storage/audit` with an optional limit1..64 checks a rotating page of local available generations. The cursor persists in PostgreSQL and wraps after the collection. A page reads at most64MiB; known quarantine entries advance the cursor without being silently cleared. Only one audit runs per storage owner, using existing I/O admission. Storage mutation ownership remains exclusive.

`storage_audit_interval_seconds` enables periodic32-object pages when nonzero (1..86400); default0 disables the background sweep. Manual audit remains available. Failed database work is not counted as a successful health decision. Quarantine protects wrapper visibility, discovery and cached inventories; exposing raw strfry outside the wrapper is unsupported.

`POST /admin/storage/repair` accepts an explicitly approved peer service key and an existing local blob hash. For library roots/chunks, set `library: true`; otherwise the original-publication endpoint is used. The receiver checks source service identity, response bounds, exact expected size and SHA256 before mutating storage. A healthy retry is a no-op.

Repair retains the existing generation and its charge. Under its exclusive filesystem guard, the node either confirms already durable correct bytes or removes the corrupt generation path and writes/syncs the verified replacement. Quarantine is cleared only by a sealed durable receipt plus a final owner/generation/size/peer-epoch check. A successful clear advances the health epoch, invalidating stale negative inventory snapshots. No refund or new content charge is introduced.

Shared read-observation pins last through the negative-health transaction, not merely file I/O. An old failed read therefore cannot race a repair and subsequently re-quarantine the same repaired generation. Cancellation keeps the blocking operation's pin until I/O actually ends.

## Evidence

- `results/storage-audit-repair-regression.log`: bounded audit finds a synthetic corrupt original, affected publication becomes unavailable, wrong source identity is rejected, approved source restores exact bytes and charges stay unchanged.
- `results/storage-audit-repair-crash-regression.log`: an actual receiver process is killed after repaired bytes are durable but before quarantine-clear commits. Restart still hides the content; verified retry completes recovery without duplicate charging. This is not power-loss certification.
- Library-chunk repair and the latest account-peer additions passed the HTTP fixture in `results/account-peer-http-regression.log`; the combined regression and discovery-choice updates are tracked in `results/discovery-peer-repair-full-regression-2.log`.

## Scope

These operations repair existing locally receipted bytes. They do not authorize arbitrary URL fetching, treat a peer URL as identity, restore a moderator-deleted publication, erase tombstones, or infer health from file presence alone. A missing/corrupt object's logical charge stays reserved/used until actual verified cleanup. Automatic selection of a repair peer is not enabled; operators select an approved source. Protected-original garbage collection and external nginx-reader lifetime integration remain separate work.
