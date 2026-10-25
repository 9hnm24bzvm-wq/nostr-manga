# Official hosted browser run 38671869087

- Run: https://github.com/9hnm24bzvm-wq/nostr-manga/actions/runs/38671869087
- Artifact: https://github.com/9hnm24bzvm-wq/nostr-manga/actions/runs/38671869087/artifacts/11945194027
- Artifact expires: 2026-10-16T03:07:07Z. The three official sanitized JSON files are preserved here byte-for-byte.
- Public source: 5dc19a59c31e9e99f2ab1845bd70b5338d483bfc, tree fc87bea7426f2899d511cff29a28ec212e70ceb0; local tree-equivalent checkpoint 67a6601083c9a5287c4a55f4317cfb1ca4d76a4b.

## Result

The workflow failed. Of 51 planned executions across 11 batches, 37 ran: 31 passed and 6 failed. The 39-phase unique inventory has 20 passing, 6 failing and 13 pending phases. Pending phases are not passing. Both UI11 device-navigation and device-progress passed. All 11 owned fixture teardowns were verified, with ports closed, keys removed and zero remaining owned processes. Failed batch lifecycles remain marked unsuccessful.

Failed phase boundaries (causes remain unproven):
- upload: upload-commit-receipt; upload-revert-prerequisite; upload-page-edit-prerequisite.
- editorial: editorial-author-control; editorial-transition-prerequisite; editorial-publish-prerequisite.
- signed-recipients: signed-recipients-two-snapshot-accepted; signed-recipients-two-snapshot-job-retained; signed-recipients-peers-published-prerequisite.
- ui12-compact-feed: compact-feed-unique-sequential-pages in ui-reader. Native HTTP returned 200 with no database failure. Numeric duplicate/concurrency observations were not exported. The same phase passed in ui-feed.
- ui14-home-failures: home-private403-single-read.
- ui14-actor-follow: group/user initial-readiness, status-pending and status-pending-follow-visible checks. Finite observations show enabled Unfollow and absent Follow while the held native 204 status delivery remains pending; this is an observation, not a proven product cause.

The signed-c batch passed startup, then recorded RuntimeError without a signed phase report. Its namespace/source-controls phases remain pending. The retained job log and closed artifact expose no exact exception message; no cause is asserted. signed-restart is separately pending after the signed-d recipients failure.

## Native gate and verification

Frozen source/build preparation, the genuine native PostgreSQL compact-feed regression, evidence schema validation and artifact upload passed. The old native query hit the unchanged statement timeout at 5001ms; the candidate first page took 210ms on the stated synthetic fixture. Source-bound regression assertions retain canonical and 505/506/visibility correctness gates. This is not a successful complete old-baseline JSON/timing comparison or a general performance claim.

The official ZIP is 9655 bytes, SHA256 42277959df43e90ae5abd6ada41bb1b234d7777444a8c3128eef9be91848964e. Verification checked GitHub metadata, the strict three-file allowlist, closed schemas, exact summary recomputation and the frozen 1069-input source snapshot. See verification.json. No raw logs, signed download URLs, screenshots, secrets or runtime databases are included. Full-suite success and pixel parity remain false. This evidence-only checkpoint starts no new run.
