# Round-nineteen independent relation UI source review

Reviewed 2026-10-13, 10:18–10:24 UTC. This fresh reviewer did not implement the
relation-permission repair. This is a finite source review of the single UI18
helper patch, its actual imported capability context, mounted create/editor
controls, and the native versioned relation contract.

## Verdict

**The assigned live non-staff relation-control blocker is source-closed. No
additional actionable defect was established within this scoped pass.** The
actual composed helper enables an editor-admitted ordinary actor on an unlocked
live published title with `manga.relation.edit`, and an unlocked actual draft
owner with `manga.create`. It also enables prospective relation drafting in the
existing create form. It preserves the staff/moderator exact-permission lock
exception and denies unavailable, deleted and immutable-pending DTOs before
that exception.

The broader finite UI18 source verdict in `docs/round18-ui-source-audit.md`
remains applicable to its unchanged source. This review adds a fresh scoped
closure for the one newly changed permission module; it does not claim a new
exhaustive audit of all controls or production acceptance.

No application/backend/original frontend source was edited. No Cargo, compiler,
Vite, SQL, HTTP request, application/runtime test or browser journey was run.
Only independent source-composition/hashing inspection copies and this report
were written. Source compilation in the owner's receipt is separate evidence;
Build19 and real ordinary-account/browser acceptance remain separate gates.

## Independently identified candidate

The reviewer independently invoked the actual shared → reader → local-library
source transformation handlers in the order supplied by
`scripts/vite.platform.config.mjs:37–38`, without loading Vite or compiling or
executing the application. Composition completed at
`2026-10-13T10:20:12.765Z`.

- 534 patch definitions, 543 exact occurrences, 106 original paths
- Each transform's original SHA and sequential exact-anchor checks succeeded
- All 106 independently composed transformed hashes match
  `test-evidence/frontend/round18-relation-permission/source-manifest.json`
- 541 composed/copied `.svelte`, `.ts` and `.js` files; path-sorted source
  inventory SHA-256:
  `62778b49a2fd43862246e51a5f2557e12383cad3ab0c0d21bde01be4564e6d6c`
- All 63 actual integration files, sizes and hashes match the frozen manifest;
  independently recomputed inventory SHA-256:
  `08b4788aa51bdec2919f05596a76dc366b0785a7c66f38c0f8c4b3ad43af2e2c`
- Shared overlay SHA-256:
  `1d69c7b365e9730e5d1ee6308ffa5d4b3cd273db38a4fdf414f9835922bbd3bc`
- Reader overlay SHA-256:
  `70ec02db52c0867872f1545ff20667a9b7e0f7e747261de8b5a57f62285c9505`
- Local overlay SHA-256:
  `d394e81fdd7f6e02745ea905ec69f52d278bdac942ef9b2794110cc4ca6fc473`

All 676 original files were freshly hashed against both recorded original/copy
hashes in `test-evidence/frontend/source-parity.json`; all match. Relative to
the source533 candidate used by the broader UI18 review, the only changed
composed path is `src/lib/permission/manga.ts`. The complete module difference
is exactly the one before/after replacement in the repair receipt. Imports and
all other permission helpers remain byte-identical.

Original permission module SHA-256:
`fdf62a1220d8e13f446f4fc070788c78aded93baeef360ae88cee2b67827e799`.
Composed permission module SHA-256:
`9f9a64cf97f3a8fac5d75a25c6e99c1c14d62ed9077e40061ef2f683ac0ec6c3`.

Logical `src/` line references below refer to this independently composed source,
not the retained untransformed attachment body.

## Actual identity, permission and owner context

`src/lib/index.ts:10` and `src/lib/permission/index.ts:6` resolve the mounted
callers to the repaired Manga permission module. Its unchanged imports use the
real `State.Published === "published"`, the exact `sysop`/`staff`/`moderator`
role constants, and the real Session store. The retained stores barrel's
`./session` import is resolved to `stores/Session.ts` by the exact Vite case
resolver (`scripts/vite.platform.config.mjs:25–27`). No second capability store
or fabricated identity was substituted.

The actual composed `stores/Session.ts:14–30` accepts only a real auth-check 200
with identity ID and username. Recovery-only 200 results have no identity and
clear the normal account store. `:40–49` reads only the returned current
permissions/roles. Native `auth/preferences.rs:21–30,42–48` returns a separate
identity-less reset capability for recovery cookies, rejects API credentials
as browser sessions, and filters every returned permission through the current
principal. `auth/principal.rs:26–35,49–55` therefore denies write permissions to
recovery/API, banned/restricted and unverified actors and uses the same three
moderator roles. An identity ID alone cannot supply either required permission.

The owner comparison is real: the final registered Manga projection at
`backend/migrations/0082_catalog_projection_visibility.sql:18–20` maps
`relationships.submitter.id` directly to `catalog_manga.owner_id`, the same
column checked by `require_manga_edit`. There is no display-name/related-title
identity comparison.

## Helper/backend branch comparison

Composed `src/lib/permission/manga.ts:92–103` was compared directly with
`backend/src/domains/catalog/repository.rs:507–538` and the current principal,
read and create implementations. These are source-evaluated scenarios, not
executed tests:

| Current scenario | Composed relation helper | Native authority |
| --- | --- | --- |
| No normal current identity; recovery/API capability | Denied | No effective relation/create write permission |
| Deleted, unavailable, or `can_mutate: false` DTO, including staff | Denied before role exception | Deleted/source-hidden rows fail admission; prepared/source-accepted commands fail mutability |
| Ordinary live unlocked published title; relation permission present | Enabled | Exact `manga.relation.edit` required |
| Ordinary live unlocked published title; only create permission | Denied | Published branch does not use owned-draft create authority |
| Ordinary locked title, including its draft owner | Denied | Lock checked before ordinary published/owner branch |
| Staff/sysop/moderator; exact relation permission; live locked title | Enabled | Same moderator-and-permission exception before lock check |
| Staff role alone; no relation permission; locked title | Denied | No moderator exception without exact permission |
| Live unlocked non-published title; actual submitter; create permission | Enabled | Actual owner plus `manga.create`, independent of relation permission |
| Live unlocked foreign draft; ordinary relation permission | Denied | Only actual owner or exact moderator exception |
| Actual draft owner without create permission | Denied | Ownership does not replace required create permission |
| No saved Manga argument; current create permission | Enabled | Create requires `manga.create`, and stores the actor as the new draft owner |
| No saved Manga argument; only relation permission | Denied | Relation permission cannot authorize title creation |

The same non-published branch covers draft/submitted/rejected states, matching
the backend's `state != published` behavior; it does not incorrectly restrict
the owner to a single draft enum. A moderator without relation permission may
still use ordinary unlocked owner-create authority, also matching the backend.

The helper's unavailable/immutable defenses are not replacements for current
server authorization. Public reads already require a live published
locally/canonically visible title (`repository.rs:55–57`); private draft reads
require the owner/create or current staff read capability. Native
`catalog/read.rs:211,216–223` marks minimal pending publication receipts and
prepared commands with `can_mutate: false`. The mounted boundary at
`frontend-behavior/MangaEditBoundary.svelte:26–28,63–73` does not mount the full
editor for those receipts. A subsequent permission/visibility/lock change is
still rechecked by each actual relation mutation.

## Mounted control and submission trace

### Existing editor

The actual title edit loader and refreshed editor page retain their independent
full-editor `canUpdate` admission (`src/routes/(app)/title/[id]/edit/+page.ts:15–18`
and `+page.svelte:41–45`). The prior scoped finding's published-account
reproduction explicitly includes these ordinary editor permissions. The helper
repair does not add a new route-level permission grant or purport to change
editor access for an account possessing only an isolated relation permission.
For an owned unlocked draft, the unchanged `canUpdate` itself uses owner plus
`manga.create`, so relation permission is not secretly required to enter.

The page owns a fresh title/cover read under `privateFeedIdentity`, clears prior
rendered data on authority change and mounts the actual boundary only with the
current result (`+page.svelte:17–20,29–50,89–91`). The boundary mounts the actual
retained composed editor. That editor has a visible Relations tab at
`MangaEdit.svelte:675`; `:691` passes the repaired helper to `RelationsSection`.
The section forwards the value to `RelationGroup` and shows the denial text
only when false (`RelationsSection.svelte:26–29`). An enabled group renders the
real Add trigger and enables its actual type/remove inputs
(`RelationGroup.svelte:29–43`; `RelationGroupInput.svelte:22–35`). No second
negated-deletion or relation-permission-only owner gate remains in this path.

Save reaches `MangaEdit.svelte:142–157,314–382`. Relation-only changes do not
cause a preliminary metadata PUT: `updateManga` excludes covers/relations from
the title payload and returns when only its version remains (`:193–195,220–222`).
Existing pairs use PUT, new pairs POST, and removals DELETE with the actual
owner-title version. The caller requires 200/201/204 respectively, adopts
`manga_version` or the DELETE version header, retains each acknowledged tuple,
and reconciles uncertain outcomes through owned current reads without automatic
mutation replay. The rendered editor changes to its existing progress branch
during an actual relation operation (`:528,331,341`); an isolated Add-button
`disabled` omission is therefore not evidence of an accessible in-flight editor.

### Create

The mounted create route retains normal verified-account and `manga.create`
admission (`src/routes/(app)/create/title/+layout.svelte:10–13`). Its actual
`MangaCreate.svelte:282` calls `canEditRelations()` without a saved title, which
now selects the prospective-owner create branch instead of the former always-
false ordinary-account branch. The same real relation selection/type/remove
controls are reached.

The create caller waits for a real 201 title identity/version before relation
requests (`MangaCreate.svelte:173–181,224–240`). Native
`catalog/mutations.rs:443–457` independently checks `manga.create` and stores
the actual actor as the owner. The subsequent POST201 relation writes carry
that title ID and acknowledged owner version; their owner-create authorization
is the same branch examined above. No prospective title or relation DTO is
invented, and relation drafting alone does not perform a mutation. Existing
actor-owned recovery and acknowledged-step skipping remain unchanged.

## Native versioned relation contract retained

The current backend identities still match the scoped independent backend
review:

- `catalog/repository.rs` SHA-256:
  `0e6f11fc4b3133da602d566368fe7d0c91d25b369ffd193ee3f54d9eba624785`
- `catalog/mutations.rs` SHA-256:
  `53e764a999f7fe00b85c122145f1a727588b44be92a90ff09630eecf04ca53d9`

`catalog/mod.rs:37–42` mounts POST/PUT/DELETE on the real relation route.
`mutations.rs:760,796,924` uses the same current owner/staff/permission/lock and
immutable-publication gate for every write. The owner row remains locked while
its version is checked. POST duplicate409, PUT required positive owner version
and unchanged-type no-op, transaction-contained changed tuple/version/audit,
published visible related-target admission, real projected response plus owner
`manga_version`, and DELETE204 owner-version header remain unchanged
(`:746–953`). The composed transport actually exposes that header at
`src/lib/HTTP.ts:104–109`.

The visible helper does not grant server authority, loosen target visibility,
change signed source events, replace the owner version with `Relation.version`,
or bypass the current browser-CSRF/mutation principal at `mutations.rs:969–972`.
`frontend-behavior/authenticated-mutation.ts:21–49` retains the original actor,
current Session generation, target and component lifetime and sends the expected
account constraint. A late write cannot rebound to a newly signed-in account.

## Remaining acceptance, all unrun here

Use the final current bundle and real isolated API/database/browser fixture:

1. Eligible ordinary published editor: add, change type and remove a relation;
   verify reachable controls, POST201/PUT200/DELETE204 and actual owner versions.
2. Actual ordinary owned draft with create but no relation permission: the same
   operations; another ordinary account cannot edit it. Include submitted and
   rejected unlocked owner states where the current backend permits editing.
3. Ordinary create form: select a real published related title before creation;
   verify one acknowledged owned draft and its subsequent POST201 relation.
4. Missing permissions, foreign owner, ordinary lock, staff exact-permission
   locked exception, deleted/source-withdrawn and immutable pending receipts.
5. Current account/permission/target change while controls or a request are
   active; no stale private DTO or automatic mutation replay.
6. Real stale-version, duplicate, unchanged-type, ineligible related target and
   lost acknowledgement flows; preserve actual saved tuples/versions and expose
   truthful recovery state.

Stopping condition: the bounded fresh helper/capability/mounted caller/native
contract comparison is complete, the assigned blocker is source-closed, and no
additional actionable scoped source defect was established. No runtime or
production-readiness conclusion is implied.
