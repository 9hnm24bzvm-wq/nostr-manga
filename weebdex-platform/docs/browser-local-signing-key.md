# Browser-local Nostr key contract

The browser can generate a Nostr private/public pair or import an existing nsec
or 64-character hexadecimal private key. An npub identifies a public key and
cannot grant signing authority. This does not link a password account to a
Nostr principal or change backend admission rules.

## Ownership and explicit actions

`local-signing-key.ts` creates an owned pending draft. Generating, importing,
displaying the public npub, or exporting the recovery nsec never selects an
identity, signs an event, contacts a server or persists a key. The UI presents
recovery backup guidance and public-key review before its explicit activation
and instance-admission action. The draft captures a UI-lifetime assertion;
stale drafts cannot export or activate. Closing a draft clears only that draft.

Activation transfers the key into the existing publication signer registry.
Only the public owner, generation and local/external source enter the Svelte
identity store. Local and NIP-07 signers use the same `selectedSigner`,
`signExact`, publication and protocol-identity paths. Existing action-review,
UI-lifetime and owner-generation fences still apply. No background task or
automatic retry acquires new signing authority. Lock, identity replacement,
or switching to an extension clears the active mutable key bytes and invalidates
previously selected signer handles. Reload requires importing the recovery key.

## Cryptographic boundaries

- WebCrypto `getRandomValues` supplies fresh 32-byte candidates. Rejection
  sampling admits only secp256k1 scalars validated by the pinned curve library.
  There is no `Math.random`, modulo shortcut or home-written curve algorithm.
- Existing exact `@noble/curves@2.0.1` creates BIP-340 Schnorr signatures and
  validates curve points. Existing `@noble/hashes@2.0.1` hashes the NIP-01
  canonical array. Every local signature passes the original `verifyEvent`.
- Exact `@scure/base@2.0.0` implements NIP-19 Bech32 checksum, alphabet and
  padding handling. The adapter requires the nsec prefix, 32 decoded bytes and
  a valid scalar. Bech32m, mixed-case strings, wrong prefixes, invalid checksums,
  wrong lengths, zero and out-of-range scalars are rejected without echoing input.
- A signed event must preserve the reviewed owner/template, contain only the
  supported template fields, and fit the original 16 KiB event budget.

The new encoding package has zero runtime dependencies and no install hooks.
Its independent package/lock files live outside the immutable supplied frontend.
`restore-local-signing-base.py` fetches the exact official npm archive, verifies
its pinned SHA-512 integrity, rejects unsafe archive paths and writes a complete
file-hash provenance receipt. The normal Noble restore invokes it for clean
builds. The original two-package Noble receipt remains unchanged. Vite uses an
explicit alias to this admitted stage; release packaging retains its MIT license.
Official references: https://github.com/paulmillr/scure-base/tree/2.0.0 and
https://github.com/paulmillr/noble-curves/tree/2.0.1. This adapter has not received
an independent security audit.

## Threat model and limitations

Private material stays in JavaScript session memory. It is never placed in
localStorage, sessionStorage, IndexedDB, a backend request, URL, analytics,
logging, a worker message or the identity store. The key module has no clipboard
or transport API. Only an explicit UI copy action may put the nsec on the user's
clipboard, which then follows the operating system's clipboard retention and
sync rules. A failed clipboard write must leave a user-selectable local fallback.

Mutable byte and encoding-word arrays are cleared when their owner releases
them. JavaScript strings, DOM values, engine copies and cryptographic internals
cannot be guaranteed to be zeroized. A compromised browser, malicious extension,
XSS, screen capture, clipboard reader or someone with device access can steal
an unlocked key. Users who need isolated key custody should use an external
signer. The recovery key is the authority: losing it cannot be repaired by a
password reset; sharing it lets the recipient act as that identity.

Tests use synthetic keys only. Evidence must never include real recovery keys,
key-entry DOM values or screenshots revealing a private key. Cryptographic seam
tests are not browser clipboard/accessibility acceptance or backend HTTP tests.
