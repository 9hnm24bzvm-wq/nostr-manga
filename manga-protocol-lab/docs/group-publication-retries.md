# Retry-safe managed group publication commands

Applies to POST `/groups/{key}/publish` and its `/compat/mangadex/group/{alias}/publish` mapping. This is an optional backward-compatible command field, not a new event kind, revision link, or second publication identity.

## Nostr baseline

[NIP-01](https://github.com/nostr-protocol/nips/blob/master/01.md) includes the author, timestamp, kind, tags and content in event identity. Sending the same event again can be acknowledged as a duplicate. Choosing a different timestamp changes the ID, even when the manifest is identical. The wrapper therefore allows clients to preserve the timestamp before their first request. NIP-01 was checked8October2026.

## Client contract

1. Persist the command locally before sending: group key, kind, manifest hash, version1 and `created_at` Unix seconds. Do not persist the one-use HTTP authorization for retry.
2. Send `{ "kind":1001, "manifest":"<sha256>", "version":1, "created_at":<timestamp> }`, or kind1002 for a series. The endpoint still refuses arbitrary tags/content/kinds.
3. On response loss, repeat the same command with fresh NIP98 authorization binding its exact URL/method/body. An accepted event retains the same ID; the wrapper signs the same canonical event fields again and acknowledges the existing hosted ID.
4. A changed manifest or kind is a different publication. This mechanism does not treat two different events as equivalent.

Omitting `created_at` preserves prior server-time behavior and does not guarantee a stable ID across later retries. Clients using the retry-safe contract must retain it. Future timestamps more than60seconds ahead are rejected. Historical timestamps are permitted subject to normal event validation and local admission.

## Authority and lifecycle

An already committed publication may outlive temporary staging grants. Retrying its exact ID does not require a new upload, extend grants, charge physical bytes again, or add/alter the private first-actor audit. A currently authorized administrator may retry a former member's publication without changing its attribution.

Every attempt still checks current membership, role, group-enabled state, permission version, the local review verdict and hosted lifecycle. Revoked members are denied even if they remember a previously successful command. A locally removed/withdrawing publication is not resurrected. The original signing key must still be usable and match the managed identity. The shared publication prepare/commit path handles durable duplicates for both managed groups and native client-signed publishers; a group-specific shortcut is unnecessary. Retry acknowledgment means the exact publication was admitted and remains hosted; it is not a fresh disk-integrity or availability certificate.

There is no global exactly-once guarantee across independent instances. Pending preparations, storage failures and policy changes may still prevent acceptance. If a first attempt never committed and its grants expire, the client must restage originals under current authorization. Profile updates use their existing `expected_head` compare-and-set/read-back contract; this field does not alter profile or deletion-request semantics.

## Evidence

`server/tests/http_group_retry.rs` and `results/http-group-retry.json` exercise response loss with fresh HTTP auth, exact first event after expired grants, one publication/audit row, altered payload, clock bound, revoked publisher, administrator retry without rewritten attribution and refusal after moderated deletion. See `results/group-retry-focused.log` and `group-retry-shared-lifecycle.log`. The native publisher fixture also distinguishes acknowledgment of an already committed event from creating a new event using expired grants. Final shared-lifecycle regression passes67 result groups,Clippy and process/PostgreSQL restart. The release binary passes the HTTP retry fixture and enhanced local demo; extracted-source library/binary tests pass. See group-retry-shared-full.log, group-retry-shared-release-http.log, group-retry-shared-demo.log and group-retry-shared-archive-rebuild.log with zero exit records.
