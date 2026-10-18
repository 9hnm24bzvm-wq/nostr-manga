# Saved-device list rename

Source commit: `fa86b2b2da93265b20425401b8af18a4243f6382`, isolated from the previously qualified combined frontend.

The existing device-list rows now offer Rename, an inline name field, Save name and Cancel. The operation needs no account, signing identity or network. It uses the original signed-list codec for new-name validation, without rewriting retained legacy records.

A frozen WeakMap-bound review captures the exact device facade, list ID/original name, durable owner, scope and revision. The existing DeviceStore CAS transaction updates only that list's name and monotonic clock. A changed revision, owner, scope, missing/deleted list or stale UI context fails closed. Same-name saves confirm the original row and do not grow the revision. Existing members, progress, migration metadata and deletion fences remain intact. A rename invalidates an older public-snapshot review before signing; it does not publish or change per-key canonical records.

## Checks

- 96 / 96 affected regression cases passed, including 20 new cases: 13 actual IndexedDB-adapter cases and 7 compiled Svelte/JSDOM cases.
- Actual production DeviceStore and IDB request/transaction modules run against pinned fake-indexeddb 6.2.4. Cases cover reopen durability, two-connection CAS, deletion, owner/service/namespace drift, malformed names, no-op confirmation and rollback after a queued put.
- Actual compiled LibraryManagementTools runs under jsdom 27.0.1. Native UI primitives and library/publication orchestration are explicit stand-ins, while preview/rename execute the real storage helper. Cases cover anonymous offline rename, Cancel, cross-tab stale Save, Close and delayed authority changes.
- The real publication fixture rejects a previously captured public-snapshot review after rename before any signature or transport; its per-key protocol records/jobs remain unchanged.
- Direct strict TypeScript checking of the new helper's actual import closure passed with zero diagnostics.
- Canonical frontend source is unchanged and `git diff --check` is clean.

The independent read-only review found no owner/scope/CAS defect. The final no-op row-identity check additionally has a dedicated actual-IDB regression.

## Boundaries

No full composed strict generation, Vite rebuild or real-browser run was started for this successor because shared free space was approximately 4.38 GB. The earlier combined build/627-case/strict receipts remain associated with their original frozen source, not automatically with rename. The new patch requires a later combined association and genuine-browser acceptance before stronger claims. No production/parity claim is made.

Commands, raw regression output, direct type-check script/output and source hashes are retained in `test-evidence/frontend/device-list-rename-20261018/`. Reproduction uses the existing verified dependency stage and the existing noble/base symlinks; no downloads, installs or cache copies are needed.
