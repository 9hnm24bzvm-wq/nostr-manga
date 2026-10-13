# Finite UI11 frontend repairs

This is a source-only closure record. Real browser acceptance remains unrun.

## Saved-device list navigation

A list-click captures its immutable list ID, component lifetime, current URL and shared navigation-start epoch before any await. The application route guard advances that epoch before a navigation commits its URL; popstate and hash changes also invalidate it. Storage snapshot, mode selection and final snapshot continuations each verify ownership before continuing or navigating. A device-mode change already acknowledged by IndexedDB is retained; an old list click never replays navigation or undoes that change after the user navigates elsewhere.

The helper is also consulted by existing auth-form ownership, so beginning a different navigation suppresses stale history/navigation effects before URL commitment. It does not cancel an accepted auth mutation.

Acceptance still required: hold the list snapshot or mode operation, navigate to Search (including an in-flight navigation before URL commitment), release the held operation, and verify the newer route remains. Repeat with a reader-local panel disposed, and with Back/Forward navigation.

## Current-device progress after a library switch

Progress coalescing now includes the destination admission key: reader target, selection epoch, library context generation/key and stable account authority epoch. It excludes unrelated record/account-data refreshes. Returning from account mode to the same device page therefore creates a new current owned intent even when chapter/page/completion are unchanged. Previously queued intents remain fenced by their old destination; they are never replayed into another library.

The existing 350ms coalescing and exit flush retain their checks. Exit may finish only the immutable page action admitted while the reader was live, under the same persisted destination and scope constraints.

Acceptance still required: hold a queued device save, select account mode, let the old save be rejected, return to device mode at the same page, and verify a fresh acknowledged device save. Check coalescing, completion preservation and navigation exit separately.

## Preservation and evidence

All 676 supplied files remain byte-identical. These two repairs introduce no new markup/style exception. Executed scripts differ, as documented in the integration manifest. The finite receipt is `test-evidence/frontend/round11-owner-source-repairs/summary.json`, with composed source hashes in `source-manifest.json`. Svelte source compilation passed for 84 components and syntax compilation for 51 scripts, with seven recorded initial-value warnings. This is not runtime evidence.

The independently authored UI10 journey module has five wired phases and 30 prepared actual-control steps. Its syntax check passed; none of those steps has executed.
