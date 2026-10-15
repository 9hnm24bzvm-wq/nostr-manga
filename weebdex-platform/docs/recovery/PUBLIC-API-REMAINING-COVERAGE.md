# Frontend coverage still open

This recovered branch defaults the public catalogue gate off. The normal site still uses the original short-ID catalogue; no fully UUID-based product claim is justified. Even with the staging gate on, catalogue conversion is partial.

The new textual original-source audit (`node scripts/audit-public-api-frontend.mjs`) identifies143 `API.Family.Method(...)` call sites across17families:
-39 staged public-read call sites.
-4 explicit unavailable read adapters, including time-window popularity/recommendations.
-29 guarded unavailable catalogue commands.
-2 analytics calls explicitly unavailable without a substitute native mutation.
-1 statistics call intentionally belonging to a separate identity/forum module.
-68 calls in separate modules that still require UUID and permission review.

These are source call-site counts, not backend endpoints or a semantic reachability graph. Computed access, direct imports, and calls introduced by earlier overlays need further review.

Remaining modules include ChapterHistory and MangaFollow device/publication commands; User/uploader profile and group administration; Report; Settings; UploadSession/editorial workflows; Client; and native Thread/community functions. Nostr identity/authentication, original64-hex keys/events and the explicitly namespaced actual native-forum module remain separate contracts. No MangaDex OAuth identity is introduced.

Time-window popularity and recommendations remain real product backlog. An unavailable placeholder is honest staging behavior, not completion. Passive analytics cannot be replaced with a fabricated view/rating number. Current partial statistics omit unknown ratings rather than returning zero or5.5.

The durable browser migration core was lost. A newly designed replacement and additive v2 store/write/publication fences now have21 owned fake-IDB tests plus an actual publication-module fence test; this is not the original recovered implementation. Product conversion is hard-disabled. Before activation, finish compatible write commands and reviewed recovery UI, then rerun genuine HTTP and browser acceptance including absent/disabled forum contexts and optimized page delivery. Preserve unresolved legacy records and exportable backups throughout.

## Explicit catalogue-preview library fence

When PUBLIC_MANGADEX_CATALOGUE=1, library controls report that saved-library
changes are unavailable in this preview. All public MangaFollow/ChapterHistory
module methods, current-resource local mutations, localAction writes, account
library requests and native catalogue hydration reject with501 before storage
or network side effects. The ordinary library identity cannot claim usable
follows/history. Existing records remain inspectable and JSON/CSV backups remain
available. Default0 behavior and Nostr publication/event formats are unchanged.

This is an activation blocker, not completed library support. UUID-compatible
save/status/progress/list/import/account adapters, reviewed conversion/recovery UI
and end-to-end write acceptance are required before enabling the user default.
Device UUID conversion remains independently hard-disabled.
