# Private scheduled upload controls

This public-mode integration requires the scheduled backend contract, including
its optional captured-account cancellation extension (backend candidate
`2648305ba2709619be39bcf3c36890edf4efadd5`). It does not change any recovered frontend
source file. Exact public overlays add controls and widen only the public-mode
snapshot state/expiry types. Native immediate behavior and ChapterEdit metadata
remain separate; no source-page replacement affordance is introduced.

## Intent and receipt boundaries

- Immediate publication remains the default. The optional control accepts exact
  UTC `YYYY-MM-DDTHH:MM:SS`, rejects null, offsets, fractional seconds and invalid
  calendar values, and sends the value only inside `chapterDraft.publishAt`.
  Private draft autosave remains unchanged. Past timestamps retain server-side
  immediate semantics rather than relying on the browser's clock.
- A future 202 is not a chapter-success resource. Before freeing the editor, the
  adapter verifies the captured-account private schedule status and the exact
  private session's parent, groups, metadata, page order and original file hashes.
  A schedule accepted dialog has no chapter reader navigation or published toast.
  The immutable intent no longer occupies the user's open upload selection.
- A lost scheduled commit is fenced before dispatch in local persistent storage,
  scoped by origin, service key, catalogue namespace, signer and bound account.
  Only session IDs, the UTC value and an intent fingerprint are retained. No
  signing key, HTTP authorization or session token is stored. A short cross-tab
  storage lock is released before signing/network I/O. Existing immediate
  dispatch fencing is not migrated into a different storage format.
- Explicit receipt reconciliation can release a matching accepted schedule or a
  terminal cancellation for that exact account/session/time. An unchanged open
  snapshot does not disprove a dispatched request. A cancelled intent is not
  reconstructed into a fabricated staging or chapter-success snapshot.

## Owner management

List, exact status, retry and cancellation all sign the full URL with captured
`expectedAccountId`. Management uses a separate, bounded owner queue rather than
changing the current open upload's selection. The UI displays instance queue,
retained-byte and horizon limits, captured retry windows, blocked reasons,
unknown/source-accepted/readable states, and immutable-intent cancellation rules.

Each cancel/retry installs its own persistent uncertainty marker before dispatch,
under a short scoped cross-tab lock. A lost response never triggers replay. An
unchanged or nonterminal GET keeps the fence; terminal complete/cancelled facts,
a confirmed original response, or affirmative original-transport unsent evidence
can resolve it. In particular, a lost retry that returns to the same blocked state
may remain unresolved in this UI. No timeout or local reset silently enables a
second attempt. Other uploads are not frozen by that per-intent marker.

The active server list omits complete/cancelled intents. Up to 64 recently known
session identities are retained for exact status lookup; unresolved markers are
never evicted to make room. A missing list entry is not publication success.
Private response cache headers are checked. UI updates are guarded against owner
replacement/disposal. Reader links appear only after an explicit action obtains
both a readable owner receipt and the existing exact public reader presentation.
A completed receipt's chapter attributes are not treated as an immutable due-time
field; the admitted UTC time is remembered separately when available.

## Qualification status

The additional cases in `test_public_upload_adapter_recovery.mjs` exercise fake
transport and browser-storage seams only. The overlay cases check exact immutable
source anchors and control wiring. They do not establish a real browser run,
PostgreSQL isolation, original-media privacy, durable restart or crash safety.
At the initial source freeze these cases are authored, not executed. Real backend
fixtures and the staged frontend/browser qualification remain separate gates.

### Terminal recovery and storage verification

The normal backend commit deletes `upload_files` and projects the committed
chapter ID into the owner snapshot. Consequently a complete receipt cannot prove
that this browser recomputed the old staged-file fingerprint. A terminal complete
receipt can resolve only the exact captured-account/session dispatch fence as a
separate completed-schedule result. Previously retained chapter and requested-UTC
identity are compared when available. The original requested UTC may be recalled
from the local intent, not inferred from completed chapter attributes. No staged
snapshot is fabricated and no reader success follows from this resolution.

Both scheduling stores require exact readback after writes; scheduled commit
fence removal also verifies absence. Silent storage drops cannot enable dispatch.
Retry 202 must preserve the full immutable resource attributes, chapter ID,
requested UTC and captured retry window, and move into
scheduled/starting rather than remain blocked. A malformed response keeps its
per-intent fence even when a later nonterminal GET succeeds. A legitimate worker
transition may move canCancel from true to false between GET and retry POST;
that one-way publication-start transition is accepted, never its reversal.


### Concurrent recovery and bounded recent status

Every scheduled dispatch record transition/removal holds its scoped Web Lock and
compares the complete captured record before changing storage. Ordinary immediate
callbacks touch only their own session-storage record, never the global scheduled
store. A late private GET failing before dispatch does not establish that an
already returned commit POST was unsent. Replacement or newer commit uncertainty
survives late confirmation/presentation callbacks from another adapter.

The active owner list performs one signed read, not up to 64 serial remembered
status reads. Missing remembered identities appear as explicitly unverified rows
with individual Check saved status actions. A 404 preserves remembered identity
and any uncertainty; absence never implies completion or cancellation. Rebind or
authority refusal removes stale rendered private rows without deleting recovery.
The existing delayed autosave checks current session identity after await, so a
confirmed scheduled upload cannot be restored into the cleared editor.

Qualification of the corrected source: 162/162 finite adapter/overlay cases pass;
8/8 actual pinned Svelte/JSDOM component controls pass with a deliberately mocked
owner manager. Seven focused new controls against prior product source 63676dd
produce five expected regression failures and two already-protected passes.
Logs retain the initial harness failures and corrected reruns. These are not
real browser layout, signing, HTTP, SQL, source acceptance or power-loss proof.
