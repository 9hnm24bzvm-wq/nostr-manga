# Genuine signed browser acceptance source

Status: source prepared only. `node --check` passed as permitted build preparation;
it parsed the module without executing its imports or journeys. Chromium, the
platform, PostgreSQL, the original provisioner and runtime tests were not launched.
Neither source assertions nor this document are passing runtime evidence. Fresh
parent UI round9/source/runtime clearance is required before executing the runner.

## Handoff

`scripts/browser-signed-journeys.mjs` exports `runSignedJourneys(h)` and
`signedJourneyPhases`. The harness contract is the existing acceptance runner's
`{phase,page,context,origin,fixture,step,insist,open,report,redactions}` object.
Dispatch the eight phases below from the reviewed runner. Each phase expects a
fresh browser context and independently selects its synthetic identity and
verified recipient; none depends on another phase's browser storage.

The combined supervisor contract remains in `browser-fixture-contract.md` and
`PROTOCOL_BROWSER_FIXTURE.md`. Required nonsecret fields are exact loopback
`platform_origin`, `protocol_origin`, `mail_origin` or `control_origin`, root
`relay_origin`, `service_key`, `operator_pubkey`, native `ids.manga`, and the
genuinely `fully_ready` original source seed with two imported chapters. The
runner reads actual capabilities, current aliases, verified original events and
the real root WebSocket, rather than promoting a pending seed receipt to public
success. Use the same supervisor-owned execution namespace, bounded lifetime,
browser queue, strict-origin deny proxy and terminal-process evidence as the
other acceptance phases.

## Prepared coverage

- `signed-library`: anonymous UI follow, real chapter image/progress, custom list
  and membership remain local; native-only IDs are excluded from reviewed public
  graphs. External signer selection, recipient consent, real service identity,
  wrong service key and unapproved origin restrictions are separate. Snapshot
  denial changes neither local rows nor outbox. HTTP-proof denial retains the
  exact signed11999 job. Actual unadmitted account refusal precedes explicit
  disposable admission and exact-event retry. Genuine original staging,
  acknowledgment and projection are checked separately. Public library reads do
  not unlock approved-peer federation reads. A forwarded-and-lost real acceptance
  response remains retryable across ordinary reload; retry cannot sign a new
  snapshot. Verified recovery reads real head/object bytes, restores canonical
  series/chapter/list/member rows, and preserves scoped native data. UI JSONL
  export/review/import is portable and local-only. A foreign catalogue namespace
  device backup is rejected.
- `signed-profile`: actual reviewed0/3/10002 actions use visible controls and
  separate exact-public-action consent. Real current metadata heads, signatures,
  projection and rendered content are checked. Visible authored metadata
  withdrawal creates a genuine kind5 request and removes its current head.
- `signed-social`: real mapped source read,1111 comment,7 reaction and verified
  immediate-parent reply reach the actual original social feed. A forwarded and
  deliberately lost successful response reconciles its exact counterpart without
  replay. A request aborted before the service remains outcome-unknown when no
  counterpart exists; explicit retry reuses the same signed event and recipient.
  Authored comment withdrawal uses the original `/account-deletions` kind5 request endpoint/feed; it does not delete the native site account.
- `signed-context`: a delayed genuine source read cannot attach its results after
  target change. Cancel and target-change controls discard unsigned review and
  consent. A held external signer return preserves exact signed intent at its
  original recipient/source before the changed context stops transport. Visible
  cancellation persists and removes retry eligibility. A separately reviewed
  exact-body retry reaches only its original thread despite a different current
  target. A genuinely signed altered NIP07 return is rejected by application
  exact-template validation before queueing or sending.
- `signed-source-controls`: current chapter/social and library reviews are created
  through real UI controls before genuine owner-approved source revocation. The
  browser requires public404, absent current native mapping and unchanged local
  records; neither stale review can sign/send. A separate private restoration
  command explicitly acknowledges joined listener restart. Its identical recorded
  event/native alias must regain real visibility, projection and mapping.
- `signed-namespace`: real library publication/recovery and backup precede an
  early, genuine migration-generated platform catalogue rebuild. Original service
  identity/schema/event stay intact; the new namespace differs. Stale publication
  is fenced before signing, old scoped native rows/backups and independent
  canonical data remain unchanged, and ordinary reload/current-title navigation
  cannot implicitly adopt or mutate the rebuilt catalogue.
- `signed-restart`: real signed library/recovery is followed by an early explicit
  runtime restart. Receipt checks require exit0, no forced cleanup or surviving
  old process-group members, a new process and the same immutable UI hash. Actual
  original counterpart/recovery bytes and native/canonical/outbox state must
  survive; elapsed time is never success.
- `signed-recipients`: requires the optional actual second original service,
  independently verified identity and explicit synthetic admission. One identical
  reviewed11999 event must obtain two actual original receipts/projections. A
  real recipient-selection change invalidates old unsigned review. A failed
  primary-recipient action, retried while the secondary is selected, preserves its
  exact signed body/key and sends only to the original primary. Actual recorded
  mutual peer revocation/restoration leaves browser federation requests denied
  while separate public library heads remain genuinely readable. Federation may
  propagate public copies; browser transport destination is the immutable-intent
  assertion, rather than claiming all other copies remain absent.

## Private controls and finite phase budgets

The new phases are `signed-source-controls`, `signed-namespace`,
`signed-recipients`, and `signed-restart`. All must be admitted by the supervisor's
signed phase allowlist with a600second browser queue budget. They use the existing
`runSignedJourneys(h)` mount; no new browser harness parameter is needed. The exact
supervisor queue label is recovered from its owned child whose PID equals the
runner PID, so the control's `browser_phase` acknowledgment cannot silently use a
different environment phase label.

`fixture_control` and `run_directory` must identify a direct, private, nonsymlink
`test-runtime/browser-final-*` root with the reserved ownership marker. The runner
atomically writes a bounded mode0600 command to the verified private command path,
polls its exact request-ID terminal receipt, rejects false/incomplete results, and
reloads matching current supervisor readiness. No production HTTP moderation or
rebuild endpoint is introduced. Restore, rebuild and restart commands explicitly
set `allow_listener_restart:true`. Source IDs can only be the recorded genuine
fixture publications; peer decisions can only target the two recorded services.

Restarting controls require at least430seconds left in both the fixture and active
600second phase. These phases issue their single restart early after common UI
setup and, where needed, one small real publication/recovery. The source phase
first performs one bounded revocation and then its single restoration; it must
still meet the430second restart admission threshold. Drain220 + provisioner90 +
readiness90 is a400second worst-case control envelope; the runner waits at most
415seconds for its terminal result and5seconds for matching current readiness.
No second restart is attempted in one phase. Native300second phases cannot host
these lifecycle drills. Insufficient actual remaining time fails explicitly,
rather than queuing an impossible or unbounded control.

Both the browser routing guard and synthetic HTTP-proof signer must validate the
returned maximum-two exact loopback HTTP/rootWS/service-key recipients. A blocked
or absent second recipient fails `signed-recipients`; it cannot produce a passing
two-service claim. All other browser destinations remain denied.

## Signer and evidence boundaries

The test explicitly emulates an external NIP07 signer through a Playwright
binding. Its synthetic secret is generated in Node memory using the genuine
Schnorr implementation, never serialized to Chromium, fixture admission, cookies,
local storage, files, logs, screenshots or evidence, and is zeroed at teardown.
The browser receives only a public key and requested signed public events. The
fixture admission seam accepts only that public key and an explicit boolean.
Source-frame origin, supported event kinds, event budget and exact HTTP-proof
destination are constrained. This is a test seam, not application identity
generation or password-account signing.

Only the previously verified noble2.0.1 stage may be imported. The eventual runtime
boundary checks pinned provenance/official archive digests and every admitted
installed file hash before importing it. A changed dependency/provenance requires
fresh review; no upstream node_modules or network fallback is permitted. No
package hooks or installations belong to this journey.

Application writes use real rendered buttons, forms, confirmations and consent.
IndexedDB is read-only observation, not a state mutation shortcut. Network fault
seams either abort a request or forward an actual original response and then lose
it. They never fulfill a synthetic success, event, identity proof or receipt.
Evidence retains public synthetic IDs, kinds, local origins/namespace, digests,
receipt/projection states and outcomes. Request bodies, HTTP authentication
headers/proofs, signer secrets, session credentials and databases are excluded.
There are no screenshots or traces in these journeys. If licensed Ubunchu input
is explicitly selected, its pages remain local-cache inputs and must stay out of
checkpoint/evidence uploads, along with artwork-bearing screenshots.

## Remaining runtime acceptance

The previously missing genuine control calls are now authored. None has executed.
Foreign-backup rejection and ordinary authored kind5 withdrawal still do not count
as live namespace rebuild or moderated source-revocation evidence; only their
dedicated real control phases can establish those outcomes. Exact service-peer
positive authentication remains original backend-suite coverage; the browser
phase checks real policy changes and continued denial to ordinary browser users.

The supervisor owner must finish independent lifecycle/cleanup review, admit the
new600second phase names and provide final immutable UI/backend/provisioner hashes.
Only terminal runtime runs after fresh clearance establish browser acceptance.
Final browser-first cleanup, backend descendants, port closure and key removal
remain supervisor-owned checks, separate from intermediate joined restarts.
