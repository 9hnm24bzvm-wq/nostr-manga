# Signed author and group interfaces

The existing public author/group CRUD adapter remains the authoritative command boundary. It authenticates exact NIP-98 method/URL/body/service/nonce, resolves the signer to a currently admitted native account, and calls the genuine native catalogue mutation/audit core inside the same transaction. A cookie alone cannot authorize it. UUIDs and local group leadership never create an original protocol key or signing permission.

## Reachable commands

With `PUBLIC_MANGADEX_CATALOGUE=1`, Signed catalogue tools in the existing sidebar opens `/catalogue-tools`:

- `?kind=author`: create a local author record.
- `?kind=author&id=<UUID>`: actual current author read, partial metadata edit, and authorized staff soft deletion. Referenced authors cannot be deleted. The inherited DELETE contract has no expected-version body; a successful response is only `{result:"ok"}`, not an invented version receipt.
- `?kind=group`: create a local group, with the current bound account as its instance leader under the existing public-create policy.
- `?kind=group&id=<UUID>`: actual leader/member read, partial metadata/membership update under genuine leader/staff permissions, and an ownership-evidence claim request for an unlocked unclaimed group.

Existing author/group Settings links and the group Create route redirect to this separately signer-gated editor before native cookie forms render. The retained Claim button opens a dedicated explanation and link to that same actual group read. The native report-ID submission dialog is never selected for the public route. Its unclaimed label is based on the retained typed resource and actual relationships, rather than missing normalized member data. Existing HTML/CSS bytes are unchanged; all replacements are exact script anchors, except the separately enumerated existing availability guard.

Basic name, biography/description and website controls send changed fields only. Author biography edits preserve other current languages; switching the selected language loads that language's actual current text. Additional supported fields are explicit JSON: author social-link names follow the existing public schema (`twitter`, `pixiv`, `melonBook`, `fanBox`, `booth`, `nicoVideo`, `skeb`, `fantia`, `tumblr`, `youtube`, `weibo`, `naver`); group metadata follows `website`, `ircServer`, `ircChannel`, `discord`, `contactEmail`, `description`, `twitter`, `mangaUpdates`, `inactive`, `publishDelay`. Group update additionally accepts `leader`, `members`, `focusedLanguages`, and staff-only `locked`; native user UUID resolution and current authority checks remain server-side. No unsupported officers, native IDs or original group-key operations are exposed.

## Claim contract

`POST /api/v1/group/{UUID}/claim`, signed JSON `{version,details,expectedAccountId}`, returns HTTP202:

```json
{"result":"ok","status":"pending-moderation","requestId":"<actual report UUID>","groupId":"<canonical group UUID>","targetVersion":1,"scope":"instance-local","originalSigningAuthorityChanged":false}
```

This creates the genuine private native `scanlation_group_claim` report, including its captured target version, quota, duplicate and current eligibility checks. Identical pending requests return their same actual report; changed evidence or a stale target returns409. Only an explicit subsequent native moderator review can assign local leadership, and it rechecks current version, claimant eligibility and unclaimed state. No original event, publisher admission, group signing binding or global account role is created by either operation.

The controller pins the selected key generation, proved service and catalogue namespace; checks the route at each await; re-reads actual signer/account binding immediately before mutation dispatch; includes its captured `expectedAccountId` in exact signed JSON (or the sole signed URL query for empty-body DELETE); compares that typed User UUID to the genuine actor inside the final bound-account transaction; and requires exact UUID/type/version receipts. The expected-account extension is optional for compatibility with existing public clients, but the dedicated UI always sends it. Omission retains current-binding authorization semantics and does not assert captured-account intent. Uncertain mutations install a per-tab sessionStorage fence scoped to actor/service/catalogue/kind/target and are never automatically replayed. A normal GET does not clear an unknown-write fence. Browser restart durability, automatic unknown-write reconciliation and real browser acceptance are not claimed.

## Shared author/reference locking

Title creator assignment now holds visible authors `FOR SHARE` until its reference writes commit. Native author deletion takes its author `FOR UPDATE` lock before checking live-title references; the existing public deletion visibility lock remains in place. This closes a stale visible-author snapshot followed by an FK existence check after a concurrent soft deletion. The real HTTP regression observes actual PostgreSQL waits in both orderings: a newly committed live reference refuses a waiting deletion, and an assignment waiting behind deletion refuses the now-hidden author without creating an edge.

## Qualification boundaries

Real disposable PostgreSQL/TCP tests cover public author/group CRUD, native audit rollback, current bindings and grants, colliding-eligible-account rebind refusal using already captured signed bytes, exact signed request rejection, concurrent versions, both author-reference lock orderings, review-only group claim, explicit native acceptance, stale/revoked rejection, and zero original authority mutations. Finite controller/loader seams cover exact dispatch and uncertainty. Actual gate-on/off composed Svelte checks preserve all679 tracked canonical frontend files. These are separate layers: no live browser, real external relay, or real source group ownership claim is asserted.
