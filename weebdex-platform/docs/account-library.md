# Device-local and instance account library contracts

Status: source implemented; compilation and HTTP/database execution are separate gates recorded in platform-planning/progress/library.json. No UI pass is implied by these handlers or test source.

## Authority and privacy

The mounted frontend integration selects the browser-device library by default. Anonymous users can save follows, reading status, chapter completion/page progress and custom lists in durable IndexedDB. An external, source-hash-anchored adapter preserves the retained MangaFollow and ChapterHistory function/HTTPResponse contracts; no anonymous password Session is manufactured. Signing identity, device-library availability and password-account permissions are separate.

Choosing account-library mode is an explicit local setting and requires a real WeebDex login. Account mode uses authenticated platform.users/platform.sessions and writes only platform.* tables. Login does not import or replace device records. Account-to-device import requires a reviewed preview, freezes the authenticated account ID through pagination/application, enforces record bounds and applies revision-checked local merge or replacement.

Optional original-protocol publication uses a user-selected NIP-07 browser signer, verified and explicitly selected recipients, pinned service public keys and exact instance CSP/CORS policy. No cookie session signs a Nostr event, stores a private signing key or grants original public-key account authority. Public-key admission is an explicit separate request. Nothing publishes on save, login, import, reconnect or startup. See [Device library and signed identity integration](browser-local-library.md).

All feature responses are no-store. Every cookie mutation validates Origin or Referer and rejects cross-site requests. An account may mutate its private follows, history, page progress, lists and preferences only when it has self.preferences.update; banned/restricted users and narrower API-client scopes fail closed. Profile changes require self.user.update, or sysop/staff plus user.update for another account. No identity, password, verification or role field is accepted in profile updates.

Manga follows default private. The existing source notification field is respected and appears as the exact 0/1 notification tuple in /api/auth/check. The additive private field controls the account-row flag and has no relay-publication behavior. No public endpoint exposes an account's manga-follow or chapter-history rows. Public aggregate follow counts represent actual persisted account activity, including private follows, without identifying an account.

## Existing frontend routes

All paths are relative to /api. Feed pagination uses one-based page, bounded limit 0–100, stable ID tie-breaking and real total, limit, page and data fields. limit=0 supports count-only requests. Repeated list filters survive URL decoding; conflicting repeated singleton keys are rejected. Follows and history feeds reuse catalog title, year, language, rating, tag AND/OR inclusion/exclusion, creator, chapter-language, uploader and group filters.

- GET /user and GET/PUT /user/{id}: flat public profile DTO, actual persisted group memberships and profile discussion root. Profile version conflicts return409. No email, password or session secret is included. Optional points/messages statistics are omitted until an actual corresponding activity source exists.
- GET /user/follows/manga: owner follow feed with reading_status and followed_at
- POST/DELETE /user/follows/manga/{id}: status/notification updates and idempotent unfollow,204
- GET /user/follows/chapter/feed: published chapters of followed manga with notifications enabled, real map.manga
- GET /user/follows/chapter/updates: latest visible chapter per followed notified manga, real map.manga and distinct-manga total
- GET /user/follows/users and /user/follows/groups: owner-only follow feeds
- GET/POST/DELETE /user/follows/users/{id} and /user/follows/groups/{id}: persisted status/follow/unfollow;204 when followed,404 when not followed; self-follow is invalid
- GET /user/history/manga?id=a&id=b and GET /user/history/manga/{id}: exact nested manga -> chapter -> null object
- GET /user/history/manga/feed: actual read_at plus chapter DTO and map.manga
- POST /user/history/manga/{id}: deduplicated read_ids/unread_ids, actual read_updated/unread_updated counts. Opposing IDs or chapters belonging to a different manga reject the entire transaction. Retried read calls preserve the first read_at.
- PUT /settings: canonical preferences stored as IDs, public responses expand uploaders/groups to real DTOs. Preference version is independent of profile version. Unsupported locales/unknown referenced entities reject atomically.
- POST /user/follows/manga/import: up to400 status entries per transaction, actual weebdex IDs or unique catalog metadata.links.mal/myanimelist mappings. Unknown/ambiguous external IDs fail without a partial import. Existing notifications/privacy are preserved on repeated imports.
- GET /user/follows/manga/export: actual owner-only CSV download, UTF-8 and quote escaping, maximum10000 entries. Titles that could start spreadsheet formulas are prefixed with an apostrophe. The source importer ignores title and restores ID/status. Catalog generated manga IDs preserve the source's10-character CSV guard.

The retained source +layout.ts changes URLSearchParams.toString() to add '?'. Normal API wrappers depend on root initialization; fresh deep-link loader ordering remains a browser acceptance gate. No speculative malformed-URI rewrite is installed. See frontend-query-and-privacy-policy.md.

## Additive contracts, absent from supplied frontend

These operations are deliberately separate from the107 observed source operations. They provide account-only functionality without claiming an existing UI integration.

- POST /user/lists accepts name, optional description, visibility and manga_ids; defaults to private, returns201 with a real owner/manga relationship projection
- GET /user/lists lists the authenticated account's lists
- GET /user/{id}/lists discovers public lists, or all own lists when authenticated as that owner
- GET /list/{id} permits public and direct unlisted access; private lists return404 to other accounts and anonymous callers
- PUT /list/{id} accepts version and optional name/description/visibility/manga_ids, owner-only optimistic replacement
- DELETE /list/{id} accepts a JSON version and requires the owner; stale versions return409
- Limits:100 lists/account and100 manga/list. Duplicate manga IDs are deduplicated preserving first appearance and stable stored positions. Public/unlisted visibility requires an email-verified owner. No list is automatically Nostr-published.
- GET /user/history/chapter/{id}/progress returns only the caller's saved zero-based page progress, or404
- PUT /user/history/chapter/{id}/progress accepts version, page and optional completed. Initial version is0; subsequent updates require the stored version. Pages are checked against actual persisted chapter page count. Marking completed adds the caller's read-history entry; completed=false does not erase independent visit history.

In explicit account mode, the retained reader-history behavior marks a chapter read on reader mount. Device mode instead saves the actual reader page (one-based in the device extension), marking completion when the last image/end view is reached. The separate account page-progress API remains zero-based and is not implicitly substituted for the device adapter.

## Verification

backend/tests/library_http.rs uses the actual full router and the owned disposable
PostgreSQL runner with synthetic fixture users/sessions/manga/chapters/groups. The
current runner uses a fresh loopback TCP port with Unix sockets disabled. The test
requires its exact fixture-only runtime receipt and reserved test database; it
refuses arbitrary hosts/databases. It covers the22 literal library/profile module
operations plus the normalized direct library export operation, custom-list
visibility/ownership and page progress. Source coverage is separate from a current
compiled or executed pass.

Run through the single compiler owner's bounded execution process, using scripts/run-auth-tests.sh to own the disposable cluster:

    scripts/run-auth-tests.sh cargo test --manifest-path backend/Cargo.toml --test library_http -- --ignored --test-threads=1

No production runtime database is read or written by that test. Test runtime data and synthetic session material must be excluded from release/checkpoint archives.

A `self.library.read` API credential cannot read private reports or moderator
notes and cannot vote, create, update, remove or hard-delete discussion replies.
Those discussion actions require an actual verified browser session; voting uses
the existing persisted `reply.create` participation grant. Public discussion reads
remain public and do not reveal the token owner's private viewer state.

Password-recovery cookies are limited to resetting credentials. They cannot
supply a personal viewer identity to catalogue reading-status filters, private
lists or discussion vote projections. Public discussion/catalogue reads remain
available with anonymous projections; a normal current browser session or an
explicit owner-library credential is required for personal catalogue filters.

List-detail visibility, owner admission and projected members are read in one
PostgreSQL statement snapshot. Concurrent public/private changes return a coherent
previously public snapshot or404, never newer private contents after an earlier
public check.
