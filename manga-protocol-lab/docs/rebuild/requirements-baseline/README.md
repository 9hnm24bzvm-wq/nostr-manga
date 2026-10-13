# Execution graph

The 2026-10-07 audit found accepted features missing from the historical graph. Completion of tickets 01-17 is not completion of the full backend. See [the audit](../../docs/FEATURE-AUDIT-20261007.md).

| Ticket | Prerequisites | Status |
|---|---|---|
| [01 Demonstrate durable multi-process foundations](issues/01-demonstrate-durable-multi-process-foundations.md) | None | done |
| [02 Boot an authenticated PostgreSQL node](issues/02-boot-an-authenticated-postgresql-node.md) | 1 | done |
| [03 Upload and retrieve a verified original](issues/03-upload-and-retrieve-a-verified-original.md) | 2 | done |
| [04 Project signed events through strfry](issues/04-project-signed-events-through-strfry.md) | 2 | done |
| [05 Publish a complete immutable chapter](issues/05-publish-a-complete-immutable-chapter.md) | 3, 4 | done |
| [06 Discover chapters with accepted filters](issues/06-discover-chapters-with-accepted-filters.md) | 5 | done |
| [07 Moderate protected publication removal](issues/07-moderate-protected-publication-removal.md) | 4, 5 | done |
| [08 Synchronize one approved peer chapter](issues/08-synchronize-one-approved-peer-chapter.md) | 5, 6, 7 | done |
| [09 Resume bounded large synchronization](issues/09-resume-bounded-large-synchronization.md) | 8 | done |
| [10 Publish for groups through member keys](issues/10-publish-for-groups-through-member-keys.md) | 2, 5, 7 | done |
| [11 Maintain public libraries and social actions](issues/11-maintain-public-libraries-and-social-actions.md) | 4, 8 | done |
| [12 Repair and reclaim stored originals](issues/12-repair-and-reclaim-stored-originals.md) | 3, 5, 7, 8 | done |
| [13 Import MangaDex-shaped exports](issues/13-import-mangadex-shaped-exports.md) | 5, 10, 11 | done |
| [14 Restore an interrupted instance](issues/14-restore-an-interrupted-instance.md) | 7, 9, 10, 11, 12 | done |
| [15 Operate scalable serving roles](issues/15-operate-scalable-serving-roles.md) | 2, 3, 5, 8 | done |
| [16 Verify PostgreSQL behavior end to end](issues/16-verify-postgresql-behavior-end-to-end.md) | 6, 7, 9, 10, 11, 12, 13, 14, 15 | done |
| [17 Finalize pending discovery defaults](issues/17-finalize-pending-discovery-defaults.md) | 6 | done |
| [18 Validate deployment capacity honestly](issues/18-validate-deployment-capacity-honestly.md) | 16, 17, 30 | deferred-feature-first |
| [19 Restore admitted publisher workflows](issues/19-restore-admitted-publisher-workflows.md) | 3, 5 | done |
| [20 Complete staged blob lifecycle](issues/20-complete-staged-blob-lifecycle.md) | 19, 12 | done |
| [21 Isolate unsupported synchronization items](issues/21-isolate-unsupported-synchronization-items.md) | 8, 9 | done |
| [22 Complete Blossom browser interoperability](issues/22-complete-blossom-browser-interoperability.md) | 19, 20 | done |
| [23 Unify capability advertisements](issues/23-unify-capability-advertisements.md) | 17 | done |
| [24 Implement signed service discovery](issues/24-implement-signed-service-discovery.md) | 23 | in_progress |
| [25 Deliver multi-relay library clients](issues/25-deliver-multi-relay-library-clients.md) | 11, 22, 24 | pending |
| [26 Complete series metadata discovery](issues/26-complete-series-metadata-discovery.md) | 6, 23 | pending |
| [27 Implement submission review workflows](issues/27-implement-submission-review-workflows.md) | 19, 20 | in_progress |
| [28 Build MangaDex compatibility surface](issues/28-build-mangadex-compatibility-surface.md) | 25, 26, 27 | pending |
| [29 Harden restore serving defaults](issues/29-harden-restore-serving-defaults.md) | 14, 15 | in_progress |
| [30 Verify complete backend contracts](issues/30-verify-complete-backend-contracts.md) | 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29 | pending |

Feature-first frontier: 24 (signed discovery; identity remembrance verified; announcement format approved), 26 (metadata contracts), 27 (review/group workflows), 29 (restore safety). Resolve exact wire-contract choices before implementing discovery/compatibility assumptions.

Performance ticket 18 is deferred until feature verification. Consider structural performance concerns during refactors, but do not run new load qualification ahead of accepted feature work.

Every new ticket requires actual passing evidence, including negative/failure paths. Historical diagnostics remain evidence of missing behavior, not successful acceptance.
