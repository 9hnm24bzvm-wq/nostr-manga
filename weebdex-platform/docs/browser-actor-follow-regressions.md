# Supplemental Group/User follow browser regressions

Source preparation based exactly on `56f9cb248334eff951be571a567697b73f710fab`.
The original source preparation added one dedicated
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
oracles. The original public report checkpoints retain the 15 aggregate identities
and add four finite subchecks under each entity case (71 baseline identities)
with finite statuses; no body, URL, cookie, credentials, account header or error is
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

## Observed run and harness correction

Hosted run `38584846100` exercised the original phase on commit
`e1ea103f2d8c0d8091eb63acf11f240c1e350591` / tree
`6950c9b7a819874d630831e61ea1907a0c8c40fc`. Its sanitized result contained
15 aggregate checks: the account and four Unblock cases passed; initial
readiness, read retry, and all three pending-write cases failed for both actors
(10 failed, 5 passed). There were zero page errors. The closed aggregate artifact
does not identify the first failing inner assertion, so this is not evidence of
any later assertion succeeding or of a product regression.

The source-derived naming reproduction uses actual composed Group/User markup,
retained native Button/Spinner topology, JSDOM 27.0.1, and the hash-pinned
Playwright 1.62.1 injected accessible-name/text functions. A pending read/write
renders the accessible name `Loading Follow` (or `Loading Unfollow`), while its
exact visible label stays `Follow` (or `Unfollow`). The former exact role-name
lookup therefore cannot find the busy control. This mismatch occurs at a
required busy-button lookup in every one of the ten failed cases, and explains
why the four Unblock cases avoid it. Block/Unblock retain BanIcon without
Spinner; Retry is rendered before the next read, then becomes busy Follow.

The corrective harness uses the unique visible native entity header and exact
visible native-button text for all five labels. It still fails on missing or
duplicate controls and asserts the actual disabled state. It changes neither
product code, native statuses, response holds, polling budgets, interaction
focus, mutation counts, durable-state checks nor owned cleanup. Four fixed
subchecks distinguish prerequisites, genuine-response capture, pending controls,
and settlement, retaining all fifteen aggregate steps. Raw error messages,
request values and account data are still excluded from public diagnostics.

This is a local seam/contract correction based on exact combined source
`94c00b8b0688b121499743ed213e465eec69bc37`. JSDOM has no browser layout; the
naming seam is not a Chromium run or compiled Svelte mount. No new real-browser,
fixture, hosted CI or publication was started for this correction. Rebuild and
pin this corrected source, rerun the actual `actor-follow` batch, and then retain
the full original-phase run requirement. The existing 52 controlled Session/
controller cases remain separate evidence for in-place ordering.

## Initial readiness diagnostic probe after run 38615879103

Run `38615879103` on public commit
`e94029800c1ffde6f1d7e90d36d121ef54c12651`, tree
`83141fce1dba67dbd529cc1da53b065bba672133`, passed all Retry, Unblock and
pending-ack/lost/disposed checks. Its 71-check artifact contained 65 passed,
4 failed and 2 pending checks: both initial-readiness aggregates and their
status-pending children failed; status-settled was never reached. The initial
Session checkpoints passed, proving the public anonymous quiet view, genuine
Auth.Check delivery and target-status capture. The artifact cannot distinguish
missing Follow, actionable Follow, an exposed Block or an unexpected write.
It does not establish a product defect or a successful initial settlement.

The next local-only probe retains those exact four assertions, their messages
and order under the existing status-pending parent, and adds four fixed leaf
identities per actor (79 total). The original 15 aggregate steps, all 31 assertion
predicates/messages, three polls, five stability conditions, seven holds, all
request-count/durable-state/privacy/ownership assertions and their deadlines are
preserved. No delay, cache policy, controller or canonical frontend is changed.

Before that owned status hold is released by cleanup, a read-only observation
classifies the current expected route/header, the five exact native controls,
browser target GET/POST/DELETE/settings PUT counts, the genuine held status,
delivery disposition and whether the captured expected-account header matches
the exact prerequisite actor. Values are finite categories only; identities,
URLs, route/header/button text, request headers, bodies and exceptions are never
projected. A missing/duplicate/disabled control is observed immediately without
clicking, retrying or treating it as success. The actual hold exposes a separate
held-native-status getter; its existing delivered status and release/cancel/drain
semantics remain intact. Closed Python projection and public validation reject
extra/private/unknown/duplicate/reordered observations; a successful phase must
include both owned native204 pending-read observations with disabled Follow,
absent other actions, one GET and zero writes. Failed observations remain honest.

The exact local base is `2d9c80135328796856c789050fcd81b0093f4661` (the same
public source tree). Source inspection found no basis for changing product
readiness: private reads remain no-store and generation-owned. Pinned DOM naming,
actual hold-closure and assertion/cleanup stand-in tests establish diagnostic
contracts only. This probe is browser-unrun; it needs a fresh actual frontend
build and the actor-follow batch to identify the first failing invariant, then
any necessary evidence-based fix and the required full browser acceptance run.
The in-place Session browser limitations above remain unchanged.
