# Round 15 frontend repairs

Frozen source: `test-evidence/frontend/round15-owner-source-repairs/summary.json` (2026-10-13 08:57:53 UTC). Exact composed inventory, original hashes, anchors and exception lists are in the adjacent source manifest. The previous Build15/source496 is historical; the current composition has503 definitions/512 anchors across100 original paths.

Five finite source findings were repaired:

1. Title chapter filters compare editable values to the actual applied current URL/title context, instead of the component's mount-time query. Unchanged URL preserves unsaved selection. Actual navigation and Back/Forward refresh the baseline and draft. The applied object uses raw state, avoiding structuredClone of a Svelte proxy. Existing language/group/order/reset controls and markup are unchanged.
2. Automatic account reader-history writes require real200 plus a non-null body. Shared current-owned feedback handles401/403/409 and uncertain outcomes. The existing automatic token is not reset on failure, preventing automatic mutation replay. The automatic-write inventory distinguishes this HTTP path from device page-progress writes acknowledged by atomic IndexedDB transactions; the two manual read toggles were already repaired in Round14.
3. The existing double-page selector converts its displayed spread ordinal into that spread's actual first page index after numeric/range validation. Single-page indices are unchanged. Existing currentSpread logic already finds the spread containing currentPage, preserving selected display, offset and landscape-singleton semantics. Exactly one approved existing-handler expression changes; DOM/classes/styles are retained.
4. Tap navigation uses the actual page/spread reading surface bounding rectangle and relative clientX, rather than a hard-coded450px sidebar. It respects RTL and the existing long-strip Tap Navigation preference, and ignores default-prevented/nonprimary/interactive-control clicks. No sidebar or reading layout changes.
5. A real acknowledged empty recommendation feed renders No recommendations found, not a permanent spinner. Current-owned errors expose an explicit loading-disabled Retry. Fresh reads recheck current public availability; unscoped historical DTOs do not substitute for the current response. Target/navigation changes coalesce latest safe reads and discard stale responses. No recommendation data is invented.

All676 supplied files remain byte-identical. Source compilation passed88 Svelte components/54 scripts with8 recorded initial-reference warnings. Approved selector and recommendations exceptions are exact path/SHA/anchor entries; all unrelated original styles/markup stay checked. Globally identical executed HTML is not claimed.

Runtime acceptance remains unrun: en→es Apply→en and Back/Forward; expired-account automatic history401/403/409/unknown; choosing spread3–4 including offset/landscape/single mode; left/right taps with open/collapsed/mobile panel and interactive Retry; real empty/error recommendation feeds, Retry and target changes. Bundling and source compilation are not runtime evidence.

Build16 completed successfully at2026-10-13T09:00:03Z, exit0, unchanged1630 protected inputs, no surviving owned descendants. Full artifact/index/CSP byte evidence is in final-build16. All111 original static assets match. This does not change any unrun runtime status.
