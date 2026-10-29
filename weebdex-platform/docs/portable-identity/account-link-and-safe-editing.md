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
3. For linking, the selected signer signs an empty-content kind-27235 event with **exact** ordered `u`, `method` and `payload` tags. The payload hash covers the server's challenge string, including account, identity, origin, action, revision, expiry and nonce. The signature is sent inside the cookie-authenticated confirm command. This is an application-specific private proof using NIP-98 event shape, **not** a claim that the whole cookie/body protocol is a standard NIP-98 Authorization request. The association is not published as a public event. A selected remote signer receives its signing request through the separately reviewed encrypted relay connection; that signer sees the site URL and challenge hash, not the cleartext account identifier or private library.
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

Remote-signer qualification is tracked separately below. This document does not certify arbitrary relay/client interoperability or any real-user signer journey.

## Remote signer (bounded direct bunker connection)

The optional Account connection accepts a `bunker://` token from a signer the user already trusts. Its service key is distinct in role from the returned user key, but they may have the same value. The token must name exactly one reviewed relay; there is no automatic failover. Production uses public-domain WSS origins explicitly admitted by the deployment, while loopback WS is only accepted from a loopback development site. This is a browser-origin boundary, not backend DNS pinning or a claim to prevent DNS rebinding of an operator-trusted domain.

No socket or transport key is created before explicit approval. The temporary client key is only a transport credential, not a new user identity. It and its conversation key stay in memory, are cleared on disconnect/account switch/page exit, and are never backed up, stored in browser storage, or sent as a private key. JavaScript memory clearing is best effort, not a secure-hardware guarantee. The relay sees the browser site Origin, transport identities, IP addresses, timing and ciphertext; the trusted signer decrypts requests. The browser WebSocket handshake may attach cookies already scoped to that relay host; this API cannot request credential omission. The adapter adds no Authorization header or relay AUTH response. Browser WebSockets reject redirects and send no referrer, as specified by the [WHATWG handshake](https://websockets.spec.whatwg.org/#opening-handshake). The site does not gain the user's private signing key.

Only NIP-44 v2 encrypted kind-24133 transport is supported. NIP-04-only signers are refused, not silently downgraded. The adapter uses the pinned official `nostr-tools/nip44` module, not a new cipher implementation. It asks only for the event kinds used by the site's reviewed portable features. It does not expose private-message encryption/decryption, secret export, account creation, or broad signing permissions. The connection obtains the real user public key, checks any native association, and consults `switch_relays`; a changed relay requires a new explicit connection review rather than contacting it. An authenticated, correlated, bounded string RPC error for this optional method alone may retain the exact reviewed relay, with a notice that relay preferences were unavailable. This is not detection of a standardized unsupported-method error code. Timeouts, malformed errors, network failures and unverifiable replies still fail closed; no other endpoint or broader permission is used.

Replies must have a valid signature from the pinned bunker, the exact recipient, bounded timestamp/payload, valid NIP-44 authentication and matching random request ID. Returned events must match every reviewed field and carry the expected user's valid signature. Native account and association revision are rechecked across a sign operation. Closing an unfinished connection, cancellation, timeout, account switch and disconnect invalidate late results. The client sends each RPC once and never reconnects or resends a signing request automatically. Disconnection does not undo a public event already sent elsewhere.

An authenticated signer may request external web approval. A global status panel shows the HTTPS domain and an explicit no-referrer/no-opener link; it never opens a URL automatically. Cancel disconnects the signer. The finite request timeout still applies. No relay `AUTH` login fallback is provided.

Dependency restoration is `scripts/restore-remote-signer-deps.py` from the exact integrity lock in `platform-integrations/local-library/remote-signing-dependencies`. Only the NIP-44 module is imported; other package features are not activated. New code and synthetic tests require qualification separately from the earlier F01/F05 checkpoint. These statements are implementation boundaries, not a claim of real-user bunker interoperability.

Additional primary references: [NIP-46 remote signing](https://github.com/nostr-protocol/nips/blob/master/46.md), [NIP-44 encrypted payloads](https://github.com/nostr-protocol/nips/blob/master/44.md), [official nostr-tools NIP-44 implementation](https://github.com/nbd-wtf/nostr-tools/blob/v2.25.2/nip44.ts).
