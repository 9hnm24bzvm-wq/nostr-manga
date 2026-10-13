# Current PostgreSQL recovery entry point

Use postgres-backup-restore.md for the current offline PostgreSQL backup/restore workflow and managed-public-serving.md for reader recovery. Restore uses independent verified file copies, omits derived public serving state, pauses serving/replication/collection, clears host-specific discovery overrides and preserves identity and local moderation state. Never substitute the historical SQLite copy procedure below for the current helper.

The operator status distinguishes the serving phase from its fault category. Reader-health loss and failed namespace removal have actual-process fixtures. A stopped listener or expired timer does not prove all readers released an inode; only the verified exact reader-drain path can release holds. Foreign boot/PID-namespace state is refused for automatic recovery and needs offline operator review. No automatic restart or capacity certification is implied.

# Historical offline recovery drill follows

# Offline recovery drill

This describes a passing local synthetic recovery drill, not a production backup system. See prototype/tests/backup_restore.py and results/backup-restore.json.

## What was actually recovered

The fixture stopped both wrappers and nginx, copied the whole instance data directory and configuration, moved the original directory/config out of the serving path, and restored from the independent copy to a different directory. The restored service then passed checks for:

- The same service public key, retained signed publication and public library.
- Byte-identical original serving through the restored nginx paths.
- Persistent approved-deletion tombstones: exact replay remains rejected and the removed last-reference image remains inaccessible.
- Revoked group membership: restart/restore does not reseed it.
- An existing approved peer recognizing the same service and reconciling without a new import.
- SQLite integrity/foreign keys and preserved hardlinks between each local public image and original object.

## Necessary backup contents

The prototype needs both its configuration and its complete data directory. The configuration contains development service/group secrets, approved peer identities, policy and capacity settings. The data directory contains SQLite state (including any WAL state), private strfry/LMDB state, objects, public links, memberships, tombstones, auth replay state and queued work. Originals alone are not an identity or moderation backup. A database alone is not a self-hosted library backup.

The fixture uses an offline copy after every writer and the HTTP file server have stopped. An arbitrary live file copy is not an established consistent backup across SQLite, LMDB and filesystem state. Production online backups need coordinated snapshots or an explicitly designed export/recovery protocol and their own failure tests.

Internal hardlinks must remain internal to each independent copy. The drill used cp -a and verified that the backup and restored original have different inodes; cp -al would share live inodes and is not an independent backup. At target size, losing hardlink preservation could approximately duplicate physical bytes for public-linked originals. An external backup system must demonstrate equivalent handling on the intended filesystem.

The current demo writes plaintext development secrets. Real backup encryption, access control, key-vault integration, retention, off-site copies and restore access must be designed before deployment. No production keys or real user content were involved in the drill.

## Limits

This is not a fresh-machine compatibility test, a lost-key recovery scheme, an online backup guarantee, a power-loss test or a 100-TB restore-time measurement. A backup can restore only the decisions and content it contains; later changes require another verified source or operator reconciliation. Restoring old public links against newer moderation policy is a risk, so a production recovery procedure needs an explicit freshness and publication gate before public traffic resumes.
