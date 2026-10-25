# Explicit native account biography and contact editing

This local extension edits the four already-public native account profile fields. It is separate from original Nostr kind0 metadata, password/email/username/authority editing, and account deletion. A public UUID, native cookie, or public User.Get response grants no write authority.

## Wire contract

- Signed `GET /api/v1/nostr/user/{UUID}/profile?expectedAccountId={canonical-bound-user-UUID}` previews the explicitly bound account's own native User. No other query/body is accepted.
- Signed `PUT /api/v1/user/{UUID}` requires JSON `expectedAccountId`, `expectedUserId` (the captured canonical User UUID), `version`, and at least one of `description`, `discord`, `twitter`, `website`. All three capture fields are mandatory; duplicate/unknown/null fields are rejected. No query is accepted.
- `version` is the actual positive PostgreSQL integer `platform.users.version`, maximum2147483647. Its semantics are `shared-native-user-revision`: native profile and authentication changes can invalidate it. There is no invented profile-only counter. Overflow is409.
- Description permits20000 UTF-8 bytes; contacts2048 bytes each. Native validation trims values, permits only safe HTTP(S) URLs without credentials, constrains Twitter/X and Discord hosts, and permits native Discord invite tokens. Empty string clears; omission preserves. No fields can edit username/email/password/roles/permissions/preferences or protocol identity.
- Both responses are standard User entities with actual username/roles/version, actual four profile fields when present, and empty relationships. Top `nostr` and `data.nostr` contain `scope:'instance-account'`, `source:'persisted-native-account-profile'`, exact canonical `accountId` and `userId`, and `versionSemantics:'shared-native-user-revision'`. Entity provenance additionally has `identity:'native_public_user'`. Writes add `changed:true` to both copies. Native accepted same-value patches still advance version/audit; callers should submit changed fields only. An empty patch is400.
- Profile preview/writes are self-only. A protocol publisher presentation, another native account (including staff targets), missing explicit binding, inactive/disabled/banned account or missing `self.user.update` grant cannot authorize them. Native session staff-other update behavior remains available only through its existing native API policy.
- Every request needs genuine request-bound original Nostr HTTP admission. The current binding and native actor are locked and rechecked in the final transaction. Exact expected account/target and current native user version precede mutation. Profile/updatedAt/version/audit and response projection commit atomically; permission withdrawal, stale capture, or projection failure cannot leave a successful hidden write.
- Global classification preserves `private, no-store` and Authorization variation on preview/write successes and failures. Native-session authority is never borrowed by the public adapter.

## Public presentation and privacy

Anonymous `GET /api/v1/user/{UUID}` adds only those four public native fields. It keeps protocol-publisher kind0 presentation unchanged. SQL projects the four-key allowlist rather than the full `profile` JSON; unknown/private extension keys, email, settings and permissions are excluded. Existing32768-byte/128-entry roles bounds are retained; public selected profile metadata is bounded and native contact validation fails closed on malformed legacy values. Public profile changes are publicly visible, as in the existing native profile UI/API.

## Qualification

Exact source `c0e82cb1032a972bc1234166518dcd979fe4868d` (6fdedc4 feature plus genuine publisher fixture registration) passed a warm locked/offline jobs1 no-incremental compile, with all920 inputs unchanged.141 unit tests passed (2 deliberately ignored). Four fresh serial PostgreSQL/HTTP fixture cases passed: native profile, retained native library, signed account reads/writes plus113 routing probe, and genuine original Nostr profiles. The native-profile case exercised binding/target/version/privacy, empty clear versus omission, native session shared-version parity, same-value revision/audit semantics, private extensions preserved but excluded, colliding-version account rebind, concurrent CAS, observed final-lock grant withdrawal, rollback, replay and private-cache refusals.

All four owned runtime receipts prove PostgreSQL stopped, ports closed, no forced stop, and no survivors, unknown births or cleanup errors. Raw logs, source/binary hashes, compiled input map and physical cleanup receipts are retained in [the exact-source report](../test-evidence/native-account-profile-20261018/report.json). Prior nineteen-case full-matrix results retain their original baseline association. The new frontend consumer/full composed checks and native-browser acceptance remain separate stages.

The pinned113 MangaDex operations and remaining nine REST gaps are unchanged: this is a documented local profile extension.
