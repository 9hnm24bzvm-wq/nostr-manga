Implementation update: this historical plan is superseded by [managed-public-serving.md](managed-public-serving.md). The preserved text below records earlier sequencing and limitations.

# Remaining direct-serving integration

This is an implementation plan, not a completed feature. The tested positive/negative projection primitives and deletion barrier do not yet enable a node-level public-serving role. Positive link creation and the protected-removal acknowledgement barrier described below are now implemented and tested as explicit operations; scheduling, quarantine, startup and backup/restore remain open.

## Positive projection

A bounded durable work queue must be driven by publication visibility changes, not by repeated full scans of a100TB object catalog. A projected, healthy, complete publication can retain a shared original. The queue needs an indexed retry deadline and exact generation checks.

Before a link can exist, commit a matching physical-root-bound creating hold. Existing draining holds must not be reused as if they were active; a new creation attempt needs a committed state/token change before creating a pathname. That ensures an earlier drain cannot release a newly exposed generation.

The owned filesystem task must keep generation and publication visibility protection through verified hard-link creation and directory fsync. Mark active only after that work succeeds. A crash or failed commit leaves a charged, recoverable creating hold. No metadata acknowledgement may assume that a pending creation means the pathname does not exist.

Public filenames are derived from the original hash and validated image MIME. Manifests, library staging objects, service/group keys and backend data remain outside the public image namespace. The supervisor's hidden state directory is not served by the strict image-only nginx location.

## Negative projection and acknowledgements

Protected deletion may record its intent immediately, but backend removal must not be acknowledged as complete while the last eligible public name is still active or creating. Once the pathname has been durably withdrawn, in-flight readers may finish; the external-reader hold continues protecting physical bytes until verified drain.

Quarantine is publication-wide visibility, not just a bad-file flag. Discovering a bad page can make the other pages in that chapter ineligible. Withdraw all affected public aliases unless another healthy complete publication retains them. Commit the quarantine visibility change only after the required namespace withdrawals succeed. Never turn an uncertain unlink/fsync into a successful acknowledgement.

Positive exports and quarantine need an explicit transaction lock order. A health-epoch shared/exclusive fence can exclude positive link creation during a quarantine transition. Do not acquire an exclusive per-hash mutation guard while retaining a read observation on that same hash. Negative observations must continue fencing repair until their health decision commits. Multi-hash operations need a reviewed lock order and deterministic tests for deadlock/cancellation boundaries.

The existing negative-health path currently rejects an export-held acknowledgement. Replace that temporary gate only when the complete withdrawal path is integrated. Repair must remain blocked until all external reader holds for the generation have been released.

## Startup and recovery

Direct serving is an explicit opt-in public-read role. A guarded/private-read restart must first recover or stop any old public reader family. No stale public tree may be exposed merely because the process has restarted.

The public tree has one supervisor state directory, PUBLIC/.serving. Startup must reconcile creating/draining work before listener exposure. Physical-root and process boot/namespace mismatches remain fail-closed until a reviewed offline recovery flow exists. Do not silently clear those records.

A failed drain, killed helper, dead master, invalid reload or cancelled owner request must leave storage charges intact. Old orphaned workers are identified by the persisted session and birth identities; pidfds bind signals to those kernel processes.

## Backup and restore

Derived public aliases must not double-copy the originals, and stale process records must not be activated on the restore host. Source reader ownership must be checked in addition to storage/projection ownership. The restore profile copies independent file inodes into a new directory/database; any reset of derived export state must be explicit, transactional, and coupled to excluding the old public namespace and keeping serving disabled.

The offline helper now excludes the public namespace, rejects legacy snapshots containing it, and clears copied-target reader holds after independent-copy verification. Focused and full fixtures pass. It does not activate serving or control an independently running source nginx family; node-level startup and source-shutdown orchestration remain separate gates.

## Required integrated tests

- Crash before/after hold commit, link creation, directory fsync, active commit, unlink and drain response
- Deletion and quarantine racing positive projection; shared-original retention across healthy and unhealthy publications
- Cancelled callers and disconnected database sessions during each filesystem boundary
- Failed reload and helper/master death while a slow original reader retains its inode
- Late/uncommitted withdrawals and re-exported generations cannot inherit an old drain proof
- Wrong physical root, alternate supervisor runtime, boot/namespace mismatch and legacy unbound holds stay charged
- Guarded-policy restart with an orphaned old public reader family
- Backup/restore with active/creating/draining records and independent copied inodes
- No PostgreSQL, Moka or signature verification on a successfully exported ordinary image read
