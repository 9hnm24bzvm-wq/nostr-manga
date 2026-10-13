# 14: Restore an interrupted instance

**What to build:** Restore a stopped instance from documented backups and prove that identity, retained content, moderation and authorization survive.

**Blocked by:** 07: Moderate protected publication removal, 09: Resume bounded large synchronization, 10: Publish for groups through member keys, 11: Maintain public libraries and social actions, 12: Repair and reclaim stored originals

**Status:** done

**Spec stories:** 6, 23, 26, 27, 32

## Shared context

This is the scale-first Nostr-based manga reference node. Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka, never SQLx. Originals remain owned and verified; moderation and peer approval are local. Work directly in this chat/computer environment. Website UI and production deployment are outside this ticket.

## Acceptance criteria

- [x] Service identity, originals, tombstones, group revocations and public library state are preserved.
- [x] Old job ownership cannot resume against a newer restoration epoch without validation.
- [x] Accounting is reconciled and projection work resumes without duplicate publication.
- [x] Restore tests remain isolated and do not create an unintended live duplicate service.


Schema25 checkpoint: current implementation and acceptance evidence are listed in graph.json and results/migration-backup-full-regression.log. Current behavior supersedes older incomplete checkpoint notes above. Remaining external-serving and production-capacity qualifications remain explicit.
