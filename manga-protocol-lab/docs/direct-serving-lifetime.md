# Direct-original serving: lifecycle gate

Status: standalone nginx/Linux lifetime checks, the managed supervisor pipe lifecycle, and focused PostgreSQL external-reader hold tests pass. The Rust control adapter passes focused real-process tests. Public export/withdrawal coordination is not wired into the node yet. Do not point nginx at the private object store or credit physical space merely because a pathname was unlinked.

## Measured behavior

experiments/nginx_reader_lifetime.py uses installed nginx1.28.0, a synthetic8MiB file, a hard-linked public name and a deliberately slow loopback reader.

- A range request returns the exact requested original bytes.
- After both pathnames are unlinked, a new request receives404, but the old nginx worker still holds an8MiB file with link count0 and8MiB allocated blocks.
- A deliberately invalid configuration followed by HUP leaves the old reader alive. Sending a reload signal is not a drain receipt.
- With a valid configuration and a2-second worker shutdown timeout, the old worker exits after about2.11seconds in this trial. Its file descriptor is gone.
- Killing the master alone leaves the worker and its reader alive. The fixture tracks the owned session's descendants, terminates/reaps them and verifies they have exited.

Raw results and nginx logs are in results/nginx-reader-lifetime.json and .log. This is a correctness experiment, not an image-throughput or database-accounting result.

## Proposed integration requirements

1. Direct serving is explicit opt-in and restricted to a public-read instance. Private/admitted read policies retain guarded delivery.
2. Only verified, admitted originals receive public names. Staging objects, keys, backend files and library-private staging never enter the export tree.
3. A generation-fenced projection owns export creation/withdrawal. Complete removal cannot be acknowledged before all public names are durably withdrawn. Shared originals remain while another eligible publication references them.
4. Exported generations retain capacity protection through withdrawal and any in-flight reader lifetime. Reclamation/repair must not refund or overwrite under an unproven reader lifetime.
5. A supervisor must track the actual owned worker family, process birth identities and restart state. A PID file, elapsed grace period, successful signal call or dead master is insufficient proof. Uncertain recovery stays charged and fails closed.
6. New serving processes must never expose stale export trees after a private-policy restart. Backup/restore must account for an orphaned reader family as well as the storage/projection owners.
7. Failed reloads, slow readers, master/supervisor crashes, repair/quarantine, shared references and withdrawal-vs-export races need integrated tests before this gate can close.

A dedicated image-serving worker group would avoid disconnecting Nostr WebSockets whenever old image readers must drain. This is a design inference, not an implemented role or performance claim. Exact shutdown/drain policy remains an operational tradeoff: forcing a slow reader to close can interrupt its transfer.

## Primary references

NGINX documents that a failed reload retains the old configuration, while a successful reload starts new workers and asks old workers to finish: https://nginx.org/en/docs/control.html

The shutdown-timeout directive asks workers to close active connections; observing actual worker exit remains necessary: https://nginx.org/en/docs/ngx_core_module.html#worker_shutdown_timeout

The open-file cache can retain file descriptors. The experiment disables it so pathname withdrawal controls new requests: https://nginx.org/en/docs/http/ngx_http_core_module.html#open_file_cache

Sources checked2026-10-07; behavior was tested against the locally installed1.28.0 binary rather than assuming every current-documentation feature exists there.

## Current implementation boundaries (7 October 2026)

Schema32 adds generation-bound public_export_holds with no timed expiry. Reclamation retains its retry/grace records and physical charge while a hold exists. Repair refuses to overwrite an externally held object. Quarantine currently refuses an acknowledgement that would require public-name withdrawal; this is a temporary fail-closed gate, not a usable enabled direct-serving mode. No config currently enables exports.

The experimental Python supervisor persists its owned Linux process session before allowing nginx to exec. It serializes bounded commands over inherited parent-child pipes, so no public or Unix listening control socket is needed. Owner EOF triggers shutdown; exact process birth identities prevent a stale PID from authorizing signals to a reused process. Recovery also checks the executable and session before stopping orphaned workers. A3-second reply deadline prevents a parent that stops reading from indefinitely blocking cleanup.

The initial Unix-socket lifecycle attempt was blocked by the cloud sandbox (including an elevated retry); its diagnostic log is retained. The pipe design is independently tested without using that denied socket operation. Passing results: results/serving-supervisor-lifecycle.json and serving-supervisor-pipe-lifecycle.log. They cover exact Range responses, an unlinked original still held by a slow reader, observed old-worker exit after reload, owner EOF, explicit stop while stdin remains open, supervisor SIGKILL recovery, and dead-master/orphan-worker handling. These are real local processes, not a mocked drain receipt.

server/src/serving.rs is the bounded Rust parent/child adapter. A cancelled or failed request poisons the channel rather than accepting a delayed reply as a new drain proof. It does not expose a database-hold release method. An exact committed withdrawal set, a subsequent verified drain, and transactional matching-token release are still required. A successful process test does not establish that combined protocol.

Source filesystem and process state remain derived deployment details. Backup/restore export-state handling, complete-publication quarantine withdrawal, deletion acknowledgements, and re-export races remain open before direct delivery can be enabled.

Signals use pidfds and recheck the saved process birth identity, avoiding PID-reuse races between a /proc check and kill. Saved state is bound to the current boot ID and PID namespace; a different host/namespace requires offline review rather than signaling a coincidentally matching PID. The Rust cancellation regression uses a real16MiB slow-reader fixture, cancels an in-flight drain, and observes owner-pipe EOF shutting down the entire reader family.

## Next integrated acceptance slice

The next implementation must connect these separately verified pieces rather than enable exports early:

- Persist a hold before any hard link can become visible. A crash between link creation and commit retains the charge and leaves recoverable work.
- Acquire generation/visibility protection while creating the public name; recheck the eligible complete publication and projection state. Deletion or quarantine must not race a late link into visibility.
- Withdraw names durably before acknowledging removal. When one bad original hides a whole chapter, all now-ineligible originals need withdrawal; another healthy publication may still retain a shared original.
- Mark each committed withdrawal with a random token. Snapshot the exact committed token set before requesting a drain. Release only that same still-draining set after the verified response. Sequence allocation or a maximum ID is not commit order.
- Killing the node, helper or master at every boundary must preserve charges. No timeout or restart alone refunds bytes. A failed drain must prevent repair/reclaim.
- Startup under a guarded read policy must recover/stop any previous public reader family before accepting requests. Restore must discard derived names/process identities safely and keep serving disabled pending review.

The database-hold fixture currently removes a synthetic hold directly in test-only SQL to verify eventual reclamation. That is not a production release implementation or evidence for the combined acceptance slice above.

The helper subprocess receives a cleared environment with only PATH and LANG retained; database credentials and other node environment secrets are not passed to nginx. Linux pidfd support and an unprivileged service user are required. The helper listens for image traffic only on the configured loopback address; public TLS ingress and deployment policy are still an explicit future integration concern.

## Schema33 withdrawal/drain slice

The positive public link is still test-seeded, and automatic serving remains disabled. The real PostgreSQL/strfry/nginx fixture now exercises production withdrawal and hold-release methods:

- Every supported public alias for a hash is unlinked and its directory fsynced under a cancellation-safe owned storage task.
- A random withdrawal token commits while the hash mutation guard remains held. The physical hold remains.
- A bounded64-row committed token set is captured before requesting the managed reader-family drain. Only matching still-draining rows can be released afterward. A later token forced to commit while DELETE is blocked causes the older proof to release zero rows.
- An owner/public-tree check binds the Rust store to the supervisor. The helper's only permitted runtime directory is PUBLIC/.serving, so two helpers cannot use separate state directories to manage the same public tree unnoticed.
- Holds and filesystem withdrawal receipts carry a physical-root identity derived from the current boot, canonical path and owner-lock device/inode. Another physical root with the same logical owner/volume cannot withdraw/release the original hold. An unknown legacy binding remains charged. Moving storage or changing boot requires reviewed offline recovery; automatic migration across those boundaries is not implemented.
- A partial queue index supports the bounded drain cursor. Per-store drain work is serialized, and owned tasks retain cleanup responsibility across caller cancellation.

Focused evidence: results/public-export-volume-regression.log and results/serving-bound-tree-*.log. Full regression/build are tracked separately. There is still no automatic positive export projection, whole-chapter quarantine withdrawal, backend-deletion acknowledgement integration, or direct-serving-aware backup/restore. These remain release gates; the fixture's manually seeded link is not evidence that they work.

The earlier paragraph describing a test-only SQL deletion of the hold applies to the schema32 fixture. Schema33 replaces that stand-in with actual managed reader draining. It still uses an explicit test-only token mutation to force the late-commit race.

## Positive projection and protected-removal barrier

The new explicit positive-projection primitive verifies the original under its hash mutation guard, commits a physical-root-bound creating hold, then rechecks the blob generation, complete healthy projected publication and hold under database locks. A shared health-epoch lock and publication-row lock remain held through hard-link creation, directory fsync and active-state commit. The owned task continues across caller cancellation. An existing alias must refer to the exact verified original inode; it is not blindly overwritten.

A repeated or returning export commits creating state and clears any old withdrawal token before linking. An earlier drain therefore cannot release a re-exposed generation's protection. Failure after the creating hold retains the charge and recoverable state.

The backend now refuses to complete removal of the last eligible publication while its original has an active/creating public hold. A healthy, complete, projected sharing publication may retain the alias. Once the public name is durably withdrawn, backend removal may complete while the external-reader hold remains until drain.

Focused tests pass in public-export-positive-regression.log and public-export-removal-regression.log. The positive fixture checks exact original inode/bytes, idempotent repeats, wrong MIME/manifest/generation rejection, draining-to-active re-export, and an actual moderator transaction racing a blocked positive export. The combined fixture now uses the real positive primitive rather than seeding a hard link. It observes real strfry removal while PostgreSQL stays withdrawing until the public namespace is withdrawn.

These are explicit component operations. Node-level automatic scheduling, whole-publication quarantine withdrawal, startup/read-policy gating and direct-serving-aware backup/restore remain incomplete. No server config enables this role yet, and this is not an image-throughput result.

## Offline backup namespace handling

The offline helper now excludes the derived public tree and process records, rejects legacy backups containing that namespace, and clears only copied-target reader holds after verified independent copies and database restore. Source reader holds remain unchanged. Focused safety and actual-node restore tests pass; see postgres-backup-restore.md. The helper does not activate serving or automatically control a still-running source nginx family. Node-level restart/read-policy and automatic export scheduling remain open.
