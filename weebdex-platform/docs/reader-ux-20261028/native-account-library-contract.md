# Native account library adapter contract

User goal: ordinary site login owns saved titles, progress, private custom lists and preferences. Guest device saves remain usable offline; signing keys/public publication are not required. These are real persisted native account rows, never an imitation sync flag.

## Authority and storage

- New module `backend/src/public_api/native_account_library.rs`, prefix `/api/v1/account/library`. The native router applies private response headers.
- Reuse `platform.library_manga_follows`, `library_chapter_history`, `library_chapter_progress`, `library_lists`, `library_list_members`; use preferred public resource UUID mappings at the adapter boundary. Do not create another identity or duplicate library data store.
- Reads use `auth::require_native_library_principal_in(state,headers,tx)` preserving existing restricted/unverified owner-read policy; writes use `auth::require_native_account_principal_in(state,headers,tx,true)` inside the same transaction as data access. Cookie only, reject any Authorization, `X-Weebdex-Expected-Account` is native Session.identity.id. Require library-read for reads; browser session + `self.preferences.update` and validate_mutation for writes. Preserve existing unverified-private-library policy; no public list visibility operations in this adapter.
- Response envelope includes canonical accountId. Client compares both captured native Session identity/authority epoch and canonical accountId. Logout/A→B/A→B→A/navigation/close invalidates old responses. No automatic fallback to device or signer.
- All data private/no-store, Vary Cookie/Authorization/expected-account including errors. Commands never publish protocol events or import signing material.
- Prefer a monotonic account-library revision table+triggers covering ALL existing follows/status/notification/history/progress/list/member mutations, including old routes. Expected revision checked under serialized user lock. New writes are atomic and stale revision returns409. Preserve follow/status existing revision triggers and counters. Any future revision mechanism must preserve these concurrency guarantees.

## Wire (JSON; no native catalogue IDs)

Common success: `{result:'ok',authentication:'native-session',scope:'instance-account',accountId:<canonical user UUID>,revision:<nonnegative safe integer>,...}`.
All command bodies: `{expectedAccountId:<canonical UUID>,revision:<captured revision>,...}`; routes use GET/POST/DELETE. No optional expected owner/version. Structured 401/403/409 errors preserve actionable existing detail.

Collection page: common + `{total,limit,page,data}`. Limit1..100; page1..1000000 with `(page-1)*limit<=10000` (bounded server offset); deterministic tie-breaker; read rows+count+revision share one transaction snapshot. Revisions prevent mixing metadata pages when needed. No metadata hydration outside selected bounded page.

- GET `/follows`: retained library filters (`page,limit,sort,order,title,reading_status,contentRating,lang,availableTranslatedLang,status,demographic,yearFrom,yearTo,tag,tagx,tmod,txmod,id,author,artist,authorOrArtist,hasChapters`; optional `list` canonical private-list UUID owned by actor). Map typed UUID filters to native IDs, preserve exact filters, reject unsupported/duplicate controls rather than silently ignore.
  Data rows `{manga:<public Resource manga with cover/author/artist/tag includes>,status,notification,followedAt}`.
- GET `/follows/state?ids[]=...`: exact selected max100 manga UUIDs. Common + `{data:[{mangaId,saved,status?,notification?,followedAt?}]}`. Must return one row per requested canonical title or reject invisible/mismatched target. For reactive card flags, not full account scanning.
- POST `/follows/{mangaUUID}`: status (six existing values), notification boolean. Preserve private default; update actual native follow/status/counters. Receipt common + `{data:<same state row>}`.
- DELETE `/follows/{mangaUUID}`: only expected owner/revision. Receipt state saved:false; do not remove unrelated history/lists.
- GET `/updates` and `/history/feed`: retained chapter/manga preference filters; page/limit; `latest=true` only on updates selects most recent chapter per followed-notification title. Data rows `{chapter:<public chapter Resource>,manga:<public manga Resource>,completed:boolean,readAt?:timestamp}`. Counts/order exact, include necessary parent in same bounded page; do not fetch whole catalog to hydrate.
- GET `/history/state?manga[]=...&page=...&limit=...`: selected max100 title UUIDs only; page of actual completed metadata `{mangaId,chapterId,readAt}` plus common collection envelope. Used for existing title completion display with revision-consistent paging; no invented completeness on partial/failure. Optional exact `ids[]` max100 chapter selector preferred for bounded reader integration.
- POST `/history/{mangaUUID}`: `readIds` and `unreadIds`, deduplicated/disjoint total1..100, exact target parent check. Receipt common + `{readUpdated,unreadUpdated}`. Keep chapter-progress completion consistent when marking unread; never create progress page zero from an unknown reading position.
- GET `/progress/{chapterUUID}`: common + `{data:{chapterId,mangaId,page:null|zeroBasedPage,completed,version}}`; not-yet-saved version0.
- POST `/progress/{chapterUUID}`: page, completed boolean, progressVersion additionally CASes actual existing chapter progress row; validate page count; receipt same data. Completed also persists actual native history.
- GET `/lists`: common collection, metadata rows `{id,name,version,memberCount,updatedAt}`; private owned lists only.
- GET `/lists/{listUUID}`: common + `{data:{id,name,version,mangaIds:[canonical IDs in order],updatedAt}}` max100 members existing native list limit.
- POST `/lists`: name, mangaIds (0..100). Native private list create, max100 lists per owner. Receipt same detailed data.
- POST `/lists/{listUUID}`: listVersion, name?, mangaIds?; at least one change. Version CAS and ownership; preserve private visibility.
- DELETE `/lists/{listUUID}`: listVersion; receipt common + `{deleted:true,id}`; version/owner check.

## Frontend integration

The account library client/store/components use native account scope. Private-library owner operations opt into `requireVerified:false`; the default social-action verification requirement is unchanged.

- Use new scope; no signed account access import. Unknown write outcome persisted as bounded per-account action fence; no automatic replay. Reload/check affected actual state before explicit retry. Offline authenticated save shows not-saved/uncertain, never says saved locally unless the user explicitly selected guest device library.
- Select account library by default after an actual ordinary login, and device library for guest. Explicit device choice is account-scoped, not an accidental cross-account global preference. Preserve guest rows untouched. Account visible flags obtained in bounded exact-ID batches, not by catalogue scans or broad account metadata hydration.
- Reuse retained pagers/renderers through normalization (`normalizeManga`, `normalizeChapter`); real account title/update/history feeds remain server-paged.
- Guest adoption is explicit review/copy, not automatic on login: show counts, destination account, existing conflicts, unavailable items; user confirms bounded batches. Keep source records; fail/uncertain/cancel stops subsequent batches, report acknowledged progress truthfully. Do not overwrite account conflicts silently; preserve account by default. If not safely complete in this increment, show unavailable rather than claim cross-device adoption.

## Required bounded tests

Cookies missing/duplicate/Authorization ambiguity; wrong expected native/canonical account; permission/disabled/recovery/revoked/expired session; transaction lock and A/B ownership; UUID alias canonicalization and cross-kind/missing/hidden mapping; parameter limits; cross-parent history denial; revision stale/ABA; counters/status revision semantics; ordered list CAS/private ownership; visibility filters; exact counts including empty filtered page; no private caches; frontend unknown outcome/cancel/logout/account switch/offline/late results; guest adoption no writes before confirmation and partial/uncertain batch stop.

## Implemented completion lookup

`GET /continue/{mangaUUID}` accepts the existing **public bounded-reader** filter vocabulary (for example `translatedLanguage[]=en`, not retained-library `tlang`). It reuses public reader filter defaults and public-retained-v1 ordering. Common envelope plus `data:{mangaId,orderingProfile,catalogIdentity,effectiveFilters,state,reason,winner,hasMultipleEditions,continueProof,crossRequestSnapshot}`. `winner` is the existing minimal public reader Row or null; reason is oldest-unread/all-completed-newest/empty; proof is native-account-snapshot-v1; crossRequestSnapshot is false. Complete eligible nullable-label groups are selected inside one account-fenced transaction. Any completed eligible edition completes its group. Only one winner leaves SQL; there is no history/catalogue scan or invented progress page on the client. Statement, lock and overall Continue deadlines remain bounded.

## Revision implementation notes

Migration106 is metadata-only. Existing rows are baseline revision1; accounts without any library source row read0. BEFORE triggers lock source owners; AFTER triggers advance only actual source-row changes, so repeated read checks (`ON CONFLICT DO NOTHING`) are idempotent. All existing native source writers are observed. A command may change several rows; receipts are authoritative and must not be interpreted as `old+1`. Both owners are fenced for legacy maintenance ownership moves, though no account-library API supports owner transfer. Normal source routes already lock account before source rows; catalogue operations and maintenance can still contend, so callers must treat nonacknowledged results as uncertain and re-read, never automatically replay.

- Exact `history/state?ids[]=...` responses additionally echo root `selection:[canonical admitted chapter UUIDs]` and `catalogIdentity` from the same authority-fenced snapshot, even when none are completed. Callers must validate the full admitted selection before interpreting absent completed rows as unread. The normal bounded collection page contract is unchanged.
- `GET /lists/{listUUID}/titles` returns **all visible owned list members**, including unsaved titles, without implicitly following them. Common collection envelope plus root `listId,listVersion`; rows `{manga,position,saved,status:null|string,notification:null|boolean,followedAt:null|timestamp}`. Retained manga filters and explicit title sorts apply; default ordering is membership position ascending. A separate `list` query control is rejected because the path owns the target. `/follows?list=` remains saved titles restricted to that list.
- Unread updates omit `readAt`. Accepted progress writes increment the actual progress row version, including an explicitly repeated same-position save under the new captured revision; repeated chapter-history read checks remain no-ops.

## Saved-page reader integration (2026-10-28)

`GET /progress/{chapterUUID}` additionally returns `data.pageCount` and root `catalogIdentity:{catalogNamespace,serviceKey}` from the same visible-chapter/native-authority RepeatableRead transaction. The bounded reader validates only its exact already-selected chapter, complete progress version and page bounds twice, then rechecks the unchanged Continue selection before navigation. It does not scan history or claim cross-request snapshot isolation. Missing/unknown metadata never silently resumes or changes the selected chapter. Completed/new chapters start at page1; valid incomplete page0 resumes at page0+1.
