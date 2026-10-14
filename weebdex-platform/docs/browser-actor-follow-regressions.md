# Supplemental Group/User follow browser regressions

Source preparation based exactly on `56f9cb248334eff951be571a567697b73f710fab`.
No real browser execution is claimed. This adds one dedicated
`ui14-actor-follow` phase and fresh-fixture `actor-follow` batch alongside all
38 original phases. The full required set is now 39 unique phases. The historical
Build19 launch plan/pins remain historical; the supplemental machine-readable
plan is `test-evidence/frontend/browser-actor-follow-plan.json`.

## Actual-route scenarios

The phase registers/verifies a real reserved-address account and uses the existing
native seed Group and non-self User. Prerequisites use real native follow and
versioned settings APIs. No database seam, synthetic successful status, forged
account, production debug export, or compiled test-only component is introduced.
Both actual entity headers run the same seven scenario groups:

- Initial readiness: hold the genuine initial Auth.Check 200. The public header
  renders first with zero private follow requests/account actions. After Session
  readiness, hold genuine follow status 204 and require disabled Follow/no Block.
  Release it, click Unfollow, and prove exactly one DELETE plus durable 404.
- Read retry: abort delivery after a genuine status 404. Require visible Retry,
  absent Block, and zero writes. Repeated Retry admits one held native GET;
  release its 404 and recover Follow/Block without mutation.
- Existing Unblock, separately during loading and failed delivery: preblock via
  actual versioned settings. Follow/Retry remain hidden; Unblock persists exactly
  one settings PUT and version increment despite unknown follow status. The loading
  case proves the native settings effect before releasing the held status. The current follow read then
  restores actionable Follow.
- Pending follow, with acknowledgement, lost delivery, and disposal variants:
  hold POST after genuine native 204 and independently prove server commitment.
  Repeat pointer clicks on the disabled control; require one POST and no optimistic
  Unfollow or other mutation. Normal delivery enables Unfollow. Lost delivery
  requires explicit read-only Retry, which finds the existing native follow.
  Disposal uses SPA Search navigation, a subsequent genuine native DELETE204, and
  Back into current native404/Follow before releasing the obsolete POST204. It
  proves the same document, retains the distinct current absent state, suppresses
  obsolete feedback, and never replays the POST.

Delivery abort is reported as a browser-delivery failure, never a fabricated
HTTP status or rollback. Disposal may cancel browser delivery after the original
server commit. Native state and exact browser request methods/counts are separate
oracles. Public report checkpoints contain only the 15 fixed check identities
and finite statuses; no body, URL, cookie, credentials, account header or error is
added to public diagnostic projection. Existing redaction and owned teardown
requirements remain in force.

## Boundaries and remaining run

Actual login/logout navigation disposes these route components. It does not
reproduce a still-mounted A→B→A controller or a same-actor Session revision during
an unsettled follow write. Those ordering permutations retain their existing
controlled composed-script coverage. This phase adds actual route/Svelte/DOM/
network integration; it does not relabel controlled tests as browser evidence.

Run on a supported hosted Linux fixture after rebuilding and pinning the actual
frontend source containing the follow-state fix. Historical Build19 static output
cannot establish its acceptance. `--batch actor-follow` runs Startup followed by
this supplemental phase with existing owned lifecycle/cleanup gates. A full run
must retain all original phases as well. Local contract/seam tests verify harness
wiring and evidence rejection only; selectors, real timings, native responses,
actual browser operation and terminal fixture cleanup remain unestablished until
that run. No publication or hosted CI is authorized by this source preparation.
