# Signed title and chapter editor affordances

This additive repair changes only the signed title/chapter editor overlays and
one factual public chapter DTO field. The supplied 676 frontend files and its
three existing additions are not edited. Native authorization remains the only
write authority; client controls are conservative read-time presentation hints.

## Title metadata

The controls mirror `domains/catalog/repository.rs::require_manga_edit`:

- A moderator/staff/sysop with the exact requested permission may edit even a
  locked title. General metadata still requires `manga.update`.
- An unlocked published title requires `manga.update`, plus the individual
  `manga.title.edit`, `manga.author.edit`, `manga.artist.edit`, `manga.tag.edit`,
  or `manga.link.edit` grant when its corresponding field is present.
- An unlocked draft's actual creator may use `manga.create`. The creator is the
  single server-projected `creator` relationship, sourced from native owner_id,
  never the title UUID, creator label, route admission or a cookie session.
- A reviewing moderator does not need `manga.create` to update a submitted
  title when the required update/field grants are present. State-review grants
  remain distinct from metadata grants. Submit requires `manga.create` and the
  actual owner or moderator rule, not generic metadata writability.

Title and alternate title controls share the exact title grant. Author, artist,
tag and both link families have separate disabled states. Forced disabled
field input is rejected before dispatch, not silently omitted. Untouched fields
are still omitted from partial requests. Existing opaque officialLinks and
external-link behavior is retained. The controller keeps the captured account
ID but refreshes presented role/grant facts at its existing signed account
recheck; failed or foreign-account refresh clears stale presented grants.
Captured target, service, catalogue, identity, version, exact receipts and
unconfirmed-command fences remain in force.

## Chapter ownership and membership

`PublicUploadOwner.chapterAccess(version)` is a read-only frontend method, not
a new endpoint or server capability. It verifies a fresh signed account-context
response against the original bound account, signer and service scope, reads
the exact chapter UUID at the captured version, and reads its at-most-ten actual
group UUIDs with explicit leader/member edges. No mutation is used as a probe.
It accepts officer facts only with the existing
`localMembershipRoles: "leader-officer-member-v1"` descriptor. A changed account,
service, version, wrong-target response, absent ownership proof or unavailable
membership fact cannot establish the missing permission.

The controls and pre-dispatch checks mirror the native repository:

- Update: moderator plus `chapter.update`, or `self.chapter.update` plus actual
  uploader, group leader or group officer.
- Request removal: `self.chapter.remove` plus actual uploader or group leader.
  An officer alone is insufficient. Moderator plus `chapter.remove` permits the
  separately confirmed immediate instance-hide action.
- Claim: `self.chapter.update`, actual chapter group leadership, a different
  actual uploader, and explicit `localUploaderIsSystem === false`. Being the
  uploader, merely an officer/member, or a moderator alone is insufficient.

The server still rechecks actual account and group authority under its native
transaction. Read-time observations do not eliminate races or confer authority.
The existing claim POST and removal DELETE accept expectedAccountId, but do not
accept an optimistic expectedVersion. This repair checks the captured version
before presenting/sending those actions; it does not invent an atomic server
version fence or change those mutation contracts. Update retains its existing
exact version field. No replay, removal-effect upgrade or native fallback was
added. Unknown command outcomes continue to disable controls under the existing
component recovery flow; explicit reload is a read, not an automatic replay.

## `localUploaderIsSystem` compatibility fact

Public chapter resources now optionally include the boolean attribute
`localUploaderIsSystem`. `catalogue_dto.rs` computes it only from the native
chapter DTO's string `relationships.uploader.name`, using exact equality with
`"system"`. Missing, null and non-string values leave the attribute absent.
The native public `catalog_chapter_json` projection already exposes that stored
`c.uploader_name` value (migration 0082). The public user's current username is
not the stored attribution and is deliberately ignored. Case and whitespace are
not normalized. Unrelated metadata cannot supply the fact.

This field is account-independent, safe for the existing public DTO cache, and
not a canClaim authorization result. It adds no uploader name, native ID, private
account information, route, schema migration or permission. Old servers without
the field disable claim rather than guessing. Missing/ambiguous uploader aliases
or incomplete role projections likewise fail closed for the affected operation.

## Verification scope

Pinned execution uses Node 24.19.0, Svelte 5.55.1, TypeScript 5.9.3 and JSdom 27.0.1.
The finite tests mount real components, use the real signed controller and
normalization, and explicitly substitute signer/service/HTTP responses. They
cover positive and negative uploader/leader/officer/member/foreign/moderator
roles; field-specific title grants; owner drafts; reviewing moderators; locked
states; stored-system and missing facts; renamed current usernames; revocation;
exact-target/version/service/account changes; delayed receipts and unmount;
uncertain outcomes; no duplicate dispatch; and unrelated-field omission.

Old d7ac0ef component counterfactuals fail behavioral assertions with the same
new test harness: the title permission subset fails 12/12 and the chapter subset
fails 14/18 (four unaffected positive/fence cases still pass). These are not
broken imports or claimed browser/backend outcomes. Rust DTO unit tests cover
system/non-system/missing/malformed names, unrelated metadata, included/omitted
user data, and renamed usernames. Executed results and source hashes are recorded
in the accompanying sanitized evidence. Real browser, live HTTP/PostgreSQL,
signing and publication were not run in this lane. Integrated strict/build
qualification remains a separate coordinated gate.
