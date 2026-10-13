# API runtime repair: finite source-only review

Review completed: 2026-10-13 12:41 UTC.

## Result and scope

No actionable defects found in the requested production report-status repair and three minimal test-only repairs. This is a static source review, not a compilation, runtime, browser, or acceptance result. No code edits, Cargo commands, tests, or runtime execution were performed by the reviewer.

The review is bounded to the report-status parsing change, its community endpoint regressions, the auth throttle fixture count correction, the media saturation expectation correction, and the original signed-upload URI correction. The final upload test file also contains a separately appended Ubunchu acceptance test; its acceptance outcome is outside this report.

## Reviewed semantics and safeguards

1. **Endpoint-specific report status vocabulary.** `backend/src/domains/catalog/query.rs:14-19,69-96` keeps default manga status choices exactly `ongoing`, `completed`, `hiatus`, and `cancelled`, while allowing an endpoint-supplied vocabulary. `backend/src/domains/moderation/mod.rs:29,62-89` supplies exactly `waiting`, `accepted`, `refused`, and `autoresolved`, matching the unchanged report status constraint in `backend/migrations/0050_moderation.sql:17`. Query size/value limits, pagination, browser-session requirement, submitter isolation, moderator-plus-permission gate, deleted-report exclusion, and parameter-bound SQL remain intact.
2. **Report endpoint regressions.** `backend/tests/community_http.rs:576-628` asserts the valid waiting filter, acceptance of the three terminal report states with no matching waiting report, rejection of invalid/manga-only report statuses, rejection of report-only manga status, and continued acceptance of valid manga status. Existing owner and moderator visibility assertions remain intact.
3. **Auth fixture count correction.** `backend/tests/auth_http.rs:1520` uses `count=1`, satisfying the unchanged `count >= 1` constraint in `backend/migrations/0010_native_auth.sql:48`. The actual throttle-row lock, genuine blocked upsert boundary, stale-session `401`, no-secret assertion, and zero-issued-client assertion remain intact at lines 1521-1581. The fixture correction does not weaken the credential-rotation test.
4. **Media saturation expectation correction.** `backend/tests/upload_http.rs:2506-2539` expects `429`, matching immediate `ApiError::busy()` admission rejection at `backend/src/domains/uploads/media.rs:109-120` and `backend/src/error.rs:57-62`. Both media paths, held request and database capacity, and the 200 ms timeout bound remain intact. The repair changes the expected status without relaxing admission or timing assertions.
5. **Original signed-upload URI correction.** `backend/tests/protocol_bridge_http.rs:1906-1937` stages the signed original at `PUT /upload/{hash}`. The original module routes that URI to `upload_blob`; `/blobs/{hash}` exposes only GET and DELETE (`manga-protocol-lab/server/src/node.rs:1619-1620`). Signed upload proof and subsequent blocked/disconnected publication-drain assertions remain intact. The URI correction reaches the intended handler without changing production routes or weakening the drain checks.

## Frozen source identities

All six hashes below were independently read after the owner reported the patchset frozen.

| File | SHA-256 |
| --- | --- |
| `backend/src/domains/catalog/query.rs` | `2c8f14d8728af6158223f8de1faa7b2d60d53b53bc2020a846a40f867153bd22` |
| `backend/src/domains/moderation/mod.rs` | `81da9ea523f8c8b75a06b398598f765d83864117777fa801ada2e814ad8f3346` |
| `backend/tests/community_http.rs` | `078e0bfd17806c3b1dbf33aff3b731c834795aab4504a856b519962234b06c42` |
| `backend/tests/auth_http.rs` | `2675a5a30ec560afbedcae24ac0eda54b6fb294e6c920ea2bec7ec6f97bfe00b` |
| `backend/tests/upload_http.rs` | `f1d43510e3236a5bff9c0080d696def6ce11a7698938290bca83be7af5520957` |
| `backend/tests/protocol_bridge_http.rs` | `40e3fa63864e68efe6a3a4f734a8c7bbf172b4643c368f7c98454cf9aa8f8dee` |

## Acceptance boundary

This report records source review only. Compilation, database-backed tests, real-media acceptance, browser E2E, and subsequent acceptance outcomes must be evidenced separately. No acceptance outcome is asserted here.
