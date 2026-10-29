# Portable identity: one account settings surface, explicit external signing

## Ordinary use

Reading, private libraries, native comments, reports, ratings and contribution continue to use the ordinary native account. None of those clients imports a signer, generates keys, enrolls a public account, or falls back from cookie authentication to a signature.

Account → Advanced identity tools is the optional place for people who already use a portable identity. A browser signer is the primary connection option; local key creation/import remains an explicitly opened advanced option. Public profiles can be looked up using an npub without a signer. Public address lookup (NIP-05) requires explicit domain-contact consent, the deployment's existing exact HTTPS connect-origin allowance, no credentials/referrer/redirects, a finite timeout and response budget, a valid public key, and a matching signed profile address. Display names and addresses are not proof of a trusted person. A follow draft stores the exact public key, never a mutable address alias.

## Private account association, not an authority grant

`GET /api/v1/account/identity-link` shows the current private relationship. The relationship is **not** a login method, public profile property, protocol admission, role, permission, managed-group membership, or publisher binding. In particular, these endpoints never write `protocol_identity_bindings`, `account_permissions`, public events or user grants.

A verified, unrestricted, normal native browser session authenticated in the last ten minutes can prepare a connection. Future authentication timestamps are not accepted. Cookie-only extraction rejects bearer/signed authorization; ordinary same-origin mutation protections still apply. The selected signer proves control only for the explicit connection operation.

### Review and confirmation

1. `POST /account/identity-link/prepare` captures native account UUID, current relationship revision, action and selected public key. The server creates an unpredictable 256-bit nonce, five-minute expiry and an exact payload. The record is bound to the requesting session. A new review invalidates prior unfinished reviews under the same native user/link lock. Only the most recent 100 challenge records are retained per account, with a 30-day age bound.
2. The client displays the selected identity and private/public boundary. The user approves explicitly.
3. For linking, the selected signer signs an empty-content kind-27235 event with **exact** ordered `u`, `method` and `payload` tags. The payload hash covers the server's challenge string, including account, identity, origin, action, revision, expiry and nonce. The signature is sent inside the cookie-authenticated confirm command. This is an application-specific private proof using NIP-98 event shape, **not** a claim that the whole cookie/body protocol is a standard NIP-98 Authorization request. Nothing is published to a relay.
4. Confirm rechecks current cookie/session/restriction/freshness, account capture, challenge/session/expiry, revision, exact public key, event signature/kind/time and exact tags. The one-to-one association update and challenge consumption commit together. A unique active-key index prevents two accounts claiming one key. Conflicts reveal no other account identity. No automatic merging occurs.

A late signer result after Close, Cancel, account change, signer change or destination change is not sent. Once a native confirmation was dispatched, its outcome may be uncertain. The client does not replay it automatically. An equal-revision GET is **not** enough to clear that fence: the old POST may still finish. An observed revision advance or a new successful explicit prepare (which invalidates old challenges under the same lock) supplies the reconciliation barrier.

### Reconnect, disconnect and recovery

The native association persists, but private signing authority does not become part of the site session. Reconnect verifies that the signer chose the exact linked public key; a different key is refused rather than silently selected. Each public action still needs explicit approval.

A fresh native owner session can deliberately disconnect without the portable key, allowing lost-key recovery. This removes the private relationship and increments its revision. It does not delete already-public posts, merge data, transfer public identity ownership, or silently revoke unrelated operator-granted publication authority. Ask an operator to revoke that separately when needed. A new identity requires explicit disconnect followed by a fresh dual-proof connection. Recover a lost native login using the normal account-recovery route; an npub is not a recovery credential.

## Safe public profile and list replacement

The editor first loads verified current kinds 0, 3 and 10002. It refuses malformed, oversized, unverified or projection-pending editable profiles instead of substituting a blank object. All hydration parsing completes before replacing editor state. Changing signer or destination discards old editable/review state, and late results cannot populate a different selection.

Kind-0 edits patch the verified object, preserving unknown/nested fields and tags. Removing a field is explicit and appears in the before/after removal summary. Follow edits preserve existing relay/pet-name tag extensions, unrelated tags and content. Relay edits preserve read/write/both direction and other tags/content. Editing after review invalidates approval.

### Atomic expected-head extension for controlled recipients

The public signed event remains unchanged. Its HTTP request envelope adds `expectedHead` (prior event ID, or `null` meaning **no existing head**). The ordinary signed HTTP proof binds the entire request body, including that expectation. Missing `expectedHead` retains legacy unguarded protocol semantics; it is not the same as null.

The server checks the expected head under the existing author/admission transaction lock **before inserting any event or replacement outbox record**. A stale review returns 409 with the exact proposed event ID and `headChanged: true`. No stale proposed event is inserted. An already-current exact event remains idempotent. Concurrent updates from one reviewed head cannot both win. Admission and author checks remain in force.

The read endpoint advertises `expectedHeadSupported: true`, including its no-current-head 404 body. The new editor refuses destinations without that capability rather than silently publishing unguarded. The expectation is retained immutably in the local action outbox and reused with a fresh whole-body HTTP proof on an explicit retry. Old metadata retry records without an expectation must be cancelled and reviewed again. A safe conflict is a refusal, not unknown delivery or a claimed successful update. Copies already propagated elsewhere remain outside this recipient's transaction.

## Join-to-profile handoff

Successful public admission offers **Use this site for my public profile**. This explicit action re-verifies the allowed pinned service, remembers its destination and opens the profile editor. It does not select it for public library publication, send another admission, generate another identity, or publish metadata. Empty destination states link directly to setup. Public admission and native account association remain different, clearly named facts.

## Primary protocol references

- [NIP-01: signed events and replaceable metadata](https://github.com/nostr-protocol/nips/blob/master/01.md)
- [NIP-05: DNS public identity lookup, backlink check and redirect prohibition](https://github.com/nostr-protocol/nips/blob/master/05.md)
- [NIP-19: human-readable public key encoding](https://github.com/nostr-protocol/nips/blob/master/19.md)
- [NIP-65: relay-list direction markers](https://github.com/nostr-protocol/nips/blob/master/65.md)
- [NIP-98: request URL/method/body proof binding](https://github.com/nostr-protocol/nips/blob/master/98.md)

Remote-signer support and its tests are a separate F20 work item. This document does not certify arbitrary relay/client interoperability or any real-user signer journey.
