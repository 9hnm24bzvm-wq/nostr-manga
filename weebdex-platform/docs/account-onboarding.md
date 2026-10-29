# One site account; optional advanced signing identity

The normal reader journey uses the site's email/username/password account.
Ratings, reports and user/group/title follows use separate **cookie-only**
`/api/v1/account/*` adapters over the existing canonical platform account and
persisted rating/report/follow data. They never select a key, enroll a protocol
identity, borrow a signed account, or silently fall back between authentication
modes. A selected advanced signer does not change the account behind these
ordinary controls.

Signup opens an account-ready card only after a newly accepted registration and
matching authoritative session refresh. It prioritizes email verification and
continuing reading; it no longer promotes key creation for ordinary account
features. Signed-in visits, refresh and URL flags cannot recreate onboarding.

`AccountActionGate` makes a missing or unverified session actionable inside the
original rating/report/follow task. Sign in, Create account and Verify email are
clearly labeled new-tab links, preserving the existing modal. **Continue here**
explicitly refreshes the site session and resumes only the account read. It never
submits a rating, follow or report. Closure and replacement suppress late reads.
Permission refusals are distinct from missing login and email verification; they
are not mislabeled as missing keys or joining steps.

Each cookie-only request captures the native session account in
`X-Weebdex-Expected-Account`. Account context resolves its canonical UUID, which
is captured again in write payloads alongside the observed rating/follow revision
or report-reason version. The backend owns current session, verification,
permission, origin and transactional actor checks. Private reports retain their
cross-dialog uncertain-outcome fence; rating/follow uncertainty requires an
explicit read before another write. Nothing retries a write automatically.

## Advanced identity setup

Settings → Advanced → Advanced identity tools is an optional self-custody workflow for externally
signed actions, separate from normal login. Existing signed APIs keep their
original admission, binding and permission checks. They do not gain cookie
fallback. Precise server prerequisites distinguish joining, a missing explicit
binding, inactive verification, and a genuine permission refusal; a generic 403
never proves joining will grant access.

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
   **Connect without joining (advanced) → Use this key for this session** explicitly selects the acknowledged key
   without network discovery, admission or publication. This local-only path
   remains available when the instance is offline or its protocol bridge is
   unavailable. It reports only session connection, never account acceptance.
3. **Review.** Read-only capabilities, allowed-origin policy and service proof
   identify the intended recipient. The public npub and verified origin are
   shown before a separate **Join this site** action.
4. **Join.** Only that explicit action invokes the existing signed
   enrollment/admission mechanism. It installs the selected local session signer
   if it has not already been connected locally. It performs a
   fresh service proof and checks the UI lifetime, key owner and signer generation
   before signing/submission. Only a successful accepted admission presents joining as accepted; it does
   not claim that every permission or platform-account binding was granted. Uncertain errors never claim the request failed
   remotely; retry is explicit and uses the same selected key.

The main backup path continues to site-access review. Local-only connection is
an advanced/offline option and its return button explicitly says **Return without
joining**. Reopening setup from Settings or an advanced task reuses the currently
selected key without generating, importing or signing another one.

The connected-session step offers **Review joining this site** to perform
fresh recipient and enrollment-policy discovery later. Admission, public writes
and publication retain their existing verified-recipient and signed-authority
checks; offline local connection does not bypass them.

**Finish later** discards an unfinished draft and follows the original safe
return destination. Closing, navigating away, switching accounts, or changing
signer selection invalidates pending work and suppresses late focus/UI changes.
A request already received by the instance cannot be undone by leaving. An
activated session remains selected until explicit lock/disconnect, replacement,
or reload. The closed Advanced identity tools section provides manual create/import/signer
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
focused step headings, live status/errors, and minimum44px action targets. Advanced key controls are deliberately placed in a closed disclosure; no global
styling was added. Source/compiler, strict semantic, finite
seams, production build, attachment parity and actual-browser acceptance are
separate evidence classes; only the recorded final checks establish each class.

### Normal profile, preferences and directory

The browser account also owns profile editing, account preference saves, hidden
group/uploader lists, and reader directory searches. `/api/v1/account/user/{id}/profile`
only edits the current canonical account and requires the observed shared user
revision. `/api/v1/account/settings` merges only reviewed known changes with the
observed version/template/server timestamp, preserving unmapped exclusions and
unknown native fields. Blocking never silently unfollows. Unknown block outcomes
remain fenced across remounts. Ordinary reads never acknowledge them. Explicit
Check status retires the exact uncertain command only after an authoritative
greater settings revision makes its old CAS impossible. At the same revision,
Keep current setting requires a separate confirmation and a native revision-only
barrier; it never replays Block/Unblock or changes any filter value. Its exact
account/target/command receipt is required before clearing that command’s fence.

The normal settings profile editor does not need an `account` query parameter.
Only an exact explicit `authority=external` route selects the preserved advanced
signing editor; malformed authority parameters are refused, not reinterpreted.
Directory rows are scoped to the current browser Session, not stored in browser
history, and discarded on account/route changes even before reactive effects run.
