# Finite UI13 feed source repairs

Two independently reported source findings are addressed in this patchset. Runtime browser acceptance remains unrun.

## Terminal empty continuation

The grid now checks accumulated acknowledged entries, rather than only the most recent page's data. A valid terminal empty page retains previous acknowledged cards while adopting the server's actual current page and total. Initial empty queries still render the original empty state. An empty page before the reported end is rejected before state/history changes, with truthful error feedback and no automatic continuation.

Public navigation history commits the accumulated entries, unique acknowledged chapter data, coherent manga map and actual URL together. It never saves an empty last-page body as a substitute for acknowledged rows. Context-mismatched public cache cannot restore. Private library/history feed DTOs are not stored in navigation history.

## Initial-read ownership and recovery

Public safe-read ownership follows its actual route, query callback, navigation epoch and component lifetime. Unrelated IndexedDB scope discovery or initial account readiness does not discard that public response.

Private library/history feeds observe actual selected-library revision/context and stable account authority. A Svelte pre-effect clears prior personal rows and read flags before the next DOM update. It queues only the latest current-context read, waits for an already active safe read to settle, and makes no parallel requests. Late old responses cannot commit. A stable-context error is not automatically retried; the exactly approved **Retry feed** button in the existing non-entity error branch permits a new explicit safe read. Its loading state prevents duplicate admission. No mutation is replayed.

Cancellation stops further continuation/refresh admission and stale result attachment. A GET already sent may finish and be discarded. An interrupted initial read gets an explicit error/retry surface rather than an indefinite spinner.

## Preservation and acceptance boundary

The only new exact markup exceptions here are the accumulated-result guard and initial retry control. The previous bounded continuation control remains separately manifested. All 676 supplied files remain byte-identical; unrelated markup/classes/styles retain strict comparison. Globally identical executed HTML is not claimed.

Final source freeze is 2026-10-13T08:19:33Z, with 476 definitions / 485 exact anchors across 96 original paths. The final local overlay differs from the earlier 08:17 candidate only by changing personal-feed invalidation from post-effect to pre-effect so it precedes DOM rendering. Final hashes and composed source compilation are in `test-evidence/frontend/round13-owner-source-repairs/summary.json` and `source-manifest.json`. Compilation passed for 84 Svelte components and 52 scripts with seven recorded warnings. This is source/build evidence only.

Required real-browser cases: terminal withdrawal between pages, inconsistent empty page, Back/Forward of accumulated public results, initial public request held across scope/account discovery, private A→B/mode/revision changes while a read is held, stable-context failure with explicit Retry, and route/disposal cancellation. No browser/application journey has executed.
