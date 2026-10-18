# Integrated reader/search/library request recipes

Source: recovery 5cbd05d91c38e22eb6a7509aa6e9f8c21b12e3b4. The [machine-readable plan](fixtures/integrated-read-fanout-v1.json) has 13 explicit finite scenarios and 35 pinned source inputs. It is not dispatch-ready and contains no rates, user distribution, capacity target or constant API-calls-per-page assumption. No app/browser/load run was performed.

## What the mounted source actually calls

The chapter route's +page.ts invokes API.Chapter.Get; +page@.svelte mounts Reader.Provider. The five reviewed pure overlay stages, applied in production order, select the gate-on UUID client and independently owned local library implementations.

For a canonical, visible embedded UUID chapter, detail normalization makes four HTTP attempts: chapter entity GET, exact parent Manga collection GET, native-forum-context POST (a read-only query), and at-home GET. An unavailable/external chapter stops before at-home. The native gate-off DTO path has one chapter GET instead. These are adapter recipes, not whole-page network measurements.

Every mounted reader then rechecks its current selector. The supplied canonical provider has lastMangaId/module aggregate caching, but the shared overlay removes it and installs a context/lifetime-fenced mount read. The initial audit's warm-cache assumption failed an actual composed-source assertion, is retained as 6/7 red, and was corrected. A new reader mount on the same title still rereads the selector. An already mounted within-chapter page turn does not re-run chapter detail/aggregate; media queue changes still need separate accounting unless preload is explicitly disabled or all eligible pages are loaded.

The UUID selector may first GET the standard aggregate, then always traverses /chapter at limit 100, hydrating exact parent Manga resources and forum contexts for each nonempty page. For the controlled standard fixture with three chapters, detail+selector is 8 HTTP attempts; with 101 chapters in 100+1 pages it is 11. Empty selector traversal still issues its collection GET but has no hydration/forum query. The general count depends on actual pages/rows/unique parents, not the human page number.

Search makes one Manga collection GET for each admitted onSearch read. Cold filter tags are a separate full 100-row pagination, capped at 100 pages/10,000 tags; a nonempty grouped cache skips it. The shared overlay repairs the supplied canonical tag-cache guard. Requests from simultaneous cold tag population are not promised single-flight. Search and tags can overlap; listed recipe steps do not claim a total execution order.

Device /library reads saved membership/status from IndexedDB, then hydrates all matching saved candidates before global sort/page. 101 distinct visible saved UUIDs require two exact 100-ID Manga queries even when only 20 cards are displayed; an empty saved set makes zero Manga queries. The public library adapter refuses more than 10,000 candidate IDs; chapter updates use parent scopes up to 1,000 parents and 10,000 result rows. Authority and visibility changes can stop a traversal; no partial-result success or global fallback is inferred.

## Other lanes must stay explicit

- Root Session initialization can read native /api/auth/check once per imported module; local-library opening independently verifies /api/protocol/capabilities, with online/manual rechecks. These are lifecycle costs, not per-page multipliers.
- A non-UUID legacy link adds its real capabilities/typed-resolution work. Canonical UUID fixtures avoid it; no numeric legacy mapping is invented.
- Reader image URLs come from the validated at-home response. Current page and preload neighbours are queued, with parallelism controlled separately. Initial request plus three retries means four attempts for one failed-image cycle; manual Retry is a new cycle. Original/data-saver switching and actual browser/image caches change demand.
- Typed chapter-view analytics is an optional 1000 ms timer, guarded by eligibility and a session marker acknowledged only after 204. It is an analytics write, not a progress save.
- Comments mount only under their actual thread/visibility controls and can add thread statistics, reply pages and signed-in vote reads. Cover/JS/CSS/font/document/service-worker requests also depend on actual viewport/cache/profile. Those require separate observed request and body-byte accounting.
- Default device progress is 350 ms-coalesced IndexedDB work, with zero progress HTTP writes. A native-cookie account-library history update, separately bound account status/Follow/rating command, or consented public relay snapshot is a different workflow. No one-to-one logical-save/network-sync ratio is supplied.

## Budget boundary

Each actual HTTP attempt, including read-only POST context queries, non2xx replies, caller retries, setup and verification, must consume the same explicitly bounded transport budget. No redirect, transparent retry or fallback may create unaccounted work. Response/upload byte caps, absolute deadline and inflight bounds are admission inputs to the transport owner; source-derived request recipes cannot supply representative byte sizes or latency objectives. The plan's lanes align with setup/catalogue/reader/media/analytics. It deliberately has no HTTP local_progress lane.

The seven finite tests execute only source hashes, the real pure overlay chain, endpoint arithmetic and resolver selection. They launch no application, browser, database, network fixture or load. Mounted-browser traces and final source association remain prerequisites before an integrated load experiment; no representative monthly-capacity claim follows.
