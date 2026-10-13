# 13: Import MangaDex-shaped exports

**What to build:** Preview and import supplied exports and originals while preserving explicit identifiers, labels, credits and public progress mappings.

**Blocked by:** 05: Publish a complete immutable chapter, 10: Publish for groups through member keys, 11: Maintain public libraries and social actions

**Status:** done

**Spec stories:** 31

## Shared context

This is the scale-first Nostr-based manga reference node. Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka, never SQLx. Originals remain owned and verified; moderation and peer approval are local. Work directly in this chat/computer environment. Website UI and production deployment are outside this ticket.

## Acceptance criteria

- [x] Dry-run output identifies unsupported or ambiguous mappings before applying them.
- [x] Exact originals and chapter order are preserved without guessed associations.
- [x] Imported publications can be read and synchronized through the new node.
- [x] No real account access or catalogue scraping is required for the synthetic acceptance fixture.


Schema25 checkpoint: current implementation and acceptance evidence are listed in graph.json and results/migration-backup-full-regression.log. Current behavior supersedes older incomplete checkpoint notes above. Remaining external-serving and production-capacity qualifications remain explicit.
