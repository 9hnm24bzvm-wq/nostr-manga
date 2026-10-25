# Reader Continue/navigation audit, 2026-10-17

## Continue Reading

The recovered executed frontend already contains commit `3757eac`: if every currently visible aggregate chapter is completed, Continue Reading selects the newest aggregate row rather than restarting at the oldest. The immutable attachment still contains the old code, but its exact runtime overlay corrects it. That repair was preserved.

Fresh tests now execute the complete composed Continue click handler and real context expression with finite aggregate/navigation boundaries. Coverage includes an empty aggregate clearing stale targets, a single chapter, all-read history, stale/deleted last-read IDs, partial history, numeric/decimal chapter labels across volumes, explicit language/group precedence over saved language preferences, and late results after language/title/library/disposal changes. Removed IDs never become navigation targets. Ordering comes from the current visible aggregate; a client-side lexical reorder was not introduced.

## Reproduced defects and repairs

1. A current chapter missing from the latest aggregate produced index `-1`. Previous navigation then resolved aggregate index zero. The chapter index is now absent when no row matches, so automatic adjacent chapter navigation cannot jump to an unrelated row.
2. NaN/fraction page indices passed the old bounds comparisons, replaced the selected page with undefined, and reached invalid scrolling. Page and chapter selection now require safe integer indices before state or aggregate access.
3. An empty aggregate-entry row could dereference its nonexistent first entry. It now has no navigation target.

These are four exact script-only overlays. Original frontend source bytes, markup, styles, control labels and existing end-card behavior are unchanged.

## Evidence and limits

`test-evidence/reader-navigation-audit-20261017/summary.json` records hashes and scope. The retained before-fix receipt reproduces two failing tests; the final six-file finite suite passes 203/203 checks. Both default and public-catalogue composed strict checks pass with zero errors and 15 retained warnings, checking 679 unchanged canonical frontend files and 69/85 reachable integrations.

The source/accessibility checks also preserve RTL/LTR action labels, the end-card Last page control, one-based URL initialization, narrow Direction spacing and Stretch Width styling. They do not prove physical layout, mouse/touch behavior, or native browser accessibility. Genuine browser execution remains blocked before runtime, so no new screenshot, narrow-phone, live navigation, production build or end-to-end acceptance is claimed.
