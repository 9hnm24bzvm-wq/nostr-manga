# Native-account manga, group and user membership follows

The official 5.13.1 manga/group Follow and Unfollow paths are implemented as
private instance-account membership operations. The additional user write path
is an explicit application extension: upstream declares user-follow reads but
no `/user/{id}/follow` mutation. Original signed graph lists and anonymous
browser-device libraries are separate domains. Default device-library saves
must remain local; selecting an instance-account follow is a deliberate action.

Primary upstream schema:
https://gitlab.com/mangadex-pub/mangadex-api-docs/-/raw/main/static/api.yaml

## Request and authority

POST or DELETE `/api/v1/{manga|group|user}/{uuid}/follow` accepts the official
empty body. Optional strict JSON adds signed application preconditions:

```json
{"version":3,"expectedAccountId":"captured canonical native User UUID"}
```

No query, unknown/duplicate/null fields, negative/noninteger versions, non-UUID
account values or body over 1024 bytes is accepted. Nonempty bodies require
JSON content type. Every request needs a fresh exact original Nostr proof and
an explicit active verified native-account binding. Its account/binding,
original admission and current `self.preferences.update` permission are held
and rechecked in the mutation transaction. Cookies, scoped native read clients,
a profile UUID or an original signed-library read confer no write authority.
The captured `expectedAccountId` conflicts with 409 after signer rebind, even when
membership versions coincide. The UI should always supply that captured ID.

The target must resolve to an actual typed native resource of the requested
kind. A protocol-only publisher User UUID returns 404; it is never converted to
its bound native account. Public profile GET exposes `data.nostr.identity` as
`native_public_user` or `protocol_publisher` so the UI can show eligible controls
truthfully. Native self-follow/unfollow returns 400. New follows require a
currently visible published Manga, visible nondeleted group or nondisabled
native User. Wrong-kind, unknown or hidden targets return 404.

An existing native alias still permits removing the caller's own membership
after source withdrawal/deletion/disable. This response reveals only the caller's
own revision/time and the typed target UUID, never hidden target metadata.

## Membership CAS and truthful status

Successful writes return `{result:"ok",nostr:{...}}`, with actual fields:

- `scope:"instance-account"`, `source:"persisted-native-account-follows"`
- canonical `accountId`, canonical `targetId`, `kind:"manga"|"group"|"user"`
- `version`, `changed`, `followed`, nullable `followedAt` and `updatedAt`
- `versionSemantics:"membership-change-revision"`

The existing private GET `/user/follows/{kind}/{uuid}` returns 200 with the same
metadata except `changed` only for a genuinely active, currently visible follow.
It returns 404 for absent or unavailable targets. Collections and followed feed
retain truthful visible totals/filters and now include canonical accountId in
the existing private provenance extension. These responses are private/no-store.

A supplied membership version is checked before no-op detection. Fresh-proof
identical commands preserve revisions and clocks. Actual membership insertion
or removal increments the revision. Removal retains a ledger tombstone, so a
remove/re-follow cannot reset CAS. Version 0 means there has never been a
persisted membership observation. A 404 is not proof of version 0; clients must
omit the optional version after an unversioned absent-status preview rather
than invent one. Once a mutation or active status supplies a version, capture it.
Exact request replay returns 401; stale membership/account preconditions return 409.

Migration 0100 backfills existing memberships with revision 1 and their actual
stored follow time. This is a migration baseline, not a fabricated historical
edit count. Insert/delete triggers on the existing native follow tables retain
revisions for native browser mutations and imports too. Changing reading status,
privacy or notifications alone does not change a membership revision. Genuine
native mutations keep their existing response contract and authority policy.

A new Manga follow uses native `plan_to_read`, private=true, notification=true
defaults. An already active Follow does not overwrite its existing reading
status, privacy, notifications or clocks. Explicit Unfollow removes the native
membership; a later Follow creates a new default membership. Existing native
Manga follow-counter deltas run atomically in the same transaction; no cached
counter is interpreted as private per-user membership. Ratings remain separate.

All writes serialize on the account row. Native User target locking fails
closed 503 while a conflicting target-account transaction holds its row; it
uses NOWAIT to avoid opposing account-follow lock cycles. These are bounded
transactional guarantees, not browser/load qualification. Settings-template
administration remains an explicit operator-only unsupported boundary.
