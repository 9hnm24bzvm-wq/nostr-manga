# Account creation and public-key setup

The native registration form remains the site's email/username/password login.
Only a newly accepted HTTP200 registration response with an active identity,
followed by an authoritative matching session refresh, opens the public-identity
setup card. The supplied frontend attachment is unchanged; exact anchored
composition adds the card only to the registration route. Existing login,
browsing, flow reads, already signed-in visits, refresh and URL flags cannot
recreate it.

The card explains the two identities in plain language. Users can create a new
Nostr private/public key pair, paste an existing nsec or validated hexadecimal
private key, or explicitly connect a NIP07 browser signer. An npub is displayed
and can be copied for sharing; a public-only npub cannot sign and is rejected as
a private-key import. The optional site-email verification link uses the existing
route and does not claim or request an email automatically.

## Guided actions

1. **Choose a key.** Local creation uses secure WebCrypto randomness and the
   pinned vetted curve library. Import is parsed locally with pinned Bech32 and
   strict scalar checks. No signer is installed merely by creating/importing a
   draft. Secret entry is not inside a form and has no input name, preventing a
   browser GET-submit fallback from putting it in a URL.
2. **Back up.** Private reveal and clipboard copy are separate explicit controls.
   The UI warns that clipboard history/sync may keep a copy, the browser forgets
   the session on reload/lock, and site-password recovery cannot restore a lost
   key. A required acknowledgment neither creates nor verifies a backup.
   **Use this key for this session** explicitly selects the acknowledged key
   without network discovery, admission or publication. This local-only path
   remains available when the instance is offline or its protocol bridge is
   unavailable. It reports only session connection, never account acceptance.
3. **Review.** Read-only capabilities, allowed-origin policy and service proof
   identify the intended recipient. The public npub and verified origin are
   shown before a separate **Join this instance** action.
4. **Join.** Only that explicit action invokes the existing signed
   enrollment/admission mechanism. It installs the selected local session signer
   if it has not already been connected locally. It performs a
   fresh service proof and checks the UI lifetime, key owner and signer generation
   before signing/submission. Only a successful accepted admission presents the
   public identity as ready. Uncertain errors never claim the request failed
   remotely; retry is explicit and uses the same selected key.

The connected-session step offers **Review joining this instance** to perform
fresh recipient and enrollment-policy discovery later. Admission, public writes
and publication retain their existing verified-recipient and signed-authority
checks; offline local connection does not bypass them.

**Finish later** discards an unfinished draft and follows the original safe
return destination. Closing, navigating away, switching accounts, or changing
signer selection invalidates pending work and suppresses late focus/UI changes.
A request already received by the instance cannot be undone by leaving. An
activated session remains selected until explicit lock/disconnect, replacement,
or reload. Settings → Public identity provides manual create/import/signer
setup and a local-key lock control, without automatic onboarding on login.

## Privacy and lifecycle

Private-key bytes are owned by a draft, then transferred to the session signer;
discard/lock overwrite owned byte arrays. Secret strings required for manual
paste, reveal and clipboard cannot be guaranteed to be erased from JavaScript,
browser memory, extensions, clipboard history or compromised devices. See
[browser-local-signing-key.md](browser-local-signing-key.md) for the threat model.
No private key is sent to the backend, put in an error or URL, logged, persisted
in browser storage or included in analytics. Session-only is the default and
there is no encrypted-persistence option in this implementation.

The selected key signs through the existing publication registry. Key setup and
admission do not publish a profile, library, follows, reading history or comments,
and do not select/import an account library. Public profile editing remains a
separate reviewed Settings action. Site credentials and Nostr identity remain
separate; successful site registration does not imply verified email or accepted
Nostr admission.

The registration form is consumed after accepted creation, even if session
refresh fails. Stale/changed sessions instead show safe sign-in/browsing links.
Repeated pending/completed form submission and Continue navigation are fenced.
Safe local returns preserve query/hash/percent values, are decoded once, and
reject foreign origins, credentials, controls, backslashes and auth-loop paths.

The UI uses native Card/Button/Input components, normal document/dialog scrolling,
focused step headings, live status/errors, and minimum44px action targets. No
accordion or global styling was added. Source/compiler, strict semantic, finite
seams, production build, attachment parity and actual-browser acceptance are
separate evidence classes; only the recorded final checks establish each class.
