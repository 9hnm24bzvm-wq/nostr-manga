# Account-recovery failure accounting and finite diagnostics

The full hosted run `38493778645` on source commit
`cf76c09c1156da1db2a9b860eeb1eb99da3d85be` passed synthetic registration in
`account-recovery`, failed its restricted-recovery step, and left its password
reset step unresolved. The uploaded sanitized projection retained zero page
errors. It intentionally omitted the private inner error. These facts do not
identify whether the original failure was title/device setup, mail recovery,
the mounted restricted form, or a security assertion. No recovery application
change is justified by that projection alone.

The runner did contain a separate reproducible failure-accounting defect. It
created a settings-response promise before filling the password field and
attached its rejection handler only after a successful click. A failed fill or
click could leave the response timeout unhandled and terminate the runner before
the step's completed failure receipt. The finite regression test reproduces the
old process failure under strict unhandled-rejection handling and checks the
repaired path. This reproduction uses stand-ins and does not establish that the
same defect caused the original run's unresolved step.

The repaired runner fills the password field before creating its response
waiter, immediately joins the response and click promises with `Promise.all`,
and does not attempt a reset after the prior actual restricted-recovery
prerequisite failed. A click failure still leaves a bounded Playwright response
wait, with its rejection already handled. No password write is retried. Every
original successful-path assertion, the three top-level step identities, and
their pass/fail requirements remain in place. A failed prerequisite keeps the
phase failed.

Eighteen fixed ordered check IDs now distinguish device setup, logout, actual
mail delivery, local link validation, reset UI, restricted capability and private
read denial, device-record retention, password rotation, and old/new password and
link-replay results. `scripts/browser-account-recovery.mjs` is their canonical
identity list. Each receipt row contains only `check_id` and a fixed status:
`pending`, `running`, `passed`, or `failed`. An action's return value and error
body never enter that diagnostic array. The owner-supplied save callback writes
the current check before awaiting it. The shared hosted projector and closed
public schema must independently allowlist these exact identities and statuses
before any new diagnostics are published.

## Bounded local checks

From the repository root:

```sh
node --test weebdex-platform/tests/ci/test_browser_account_recovery.mjs
node --experimental-vm-modules weebdex-platform/tests/ci/test_frontend_semantic_contracts.mjs
node --experimental-vm-modules weebdex-platform/tests/ci/test_frontend_bootstrap.mjs
python3 weebdex-platform/scripts/verify-frontend-parity.py
```

The new six-case group covers receipt privacy/order, failed-check preservation,
absence of a response waiter for a missing input, the old strict-mode process
failure and repaired accounting, and the actual journey module's prerequisite
failure and complete assertion sequence against explicit stand-ins. The existing
132-case semantic group and five-entry bootstrap graph group also passed. All
676 original frontend files and the three separately pinned announcement
additions remain unchanged. These checks launch no services, database, browser,
network operation, or dependency install. A targeted genuine browser rerun with
the reviewed shared diagnostic integration is still required.

## Genuine device-setup localization and selector correction

The subsequent verified hosted diagnostic run `38498307038` on public source
`d62c0e8b3db57d3ee024676e8bdec67f425abadd` / tree
`f016ada9e111b15e3ec919e48bf5a242695e35c2` passed signup and failed the first
`device-library-seeded` checkpoint. All 17 following recovery checks stayed
pending, so the mail/password recovery controls were never reached. The final
prerequisite step now failed cleanly instead of being unresolved.

Inspection of the preserved title component and actual exact-lock Bits UI
2.16.5 package establishes a concrete harness selector defect: the title's
`SelectPrimitive.Trigger` renders a native `button` with `aria-haspopup=listbox`;
it does not have `role=combobox`. Bits UI's separate input does have that role.
The ordinary follow helper and post-reload Reading assertion incorrectly
searched for a combobox and therefore could not locate the real title trigger.
The helper now intersects the native button role and exact visible status name
with the retained title-control wrapper, trigger data slot, and listbox-popup
attribute. It deliberately does not choose the first of ambiguous matches.
The actual Reading option click and subsequent durable-record assertions are
unchanged. No application or authentication behavior was rewritten.

A separate source-proven ordinary helper defect affected repeated panel use:
the native `details` boolean `open` attribute is commonly present as the empty
string. A truthiness test incorrectly clicked an already-open panel closed.
The helper now clicks only when the attribute is `null`, with repeated-call
regressions for closed, empty-string-open, and explicitly-open panels.

The five added control seams execute the hash-pinned installed Trigger props
expression using explicit rune/DOM stand-ins, verify its retained title markup,
reproduce the impossible old role lookup, exercise the corrected exact scoped
selection against decoy controls, require the exact Reading readback, and prove
idempotent details expansion. The existing six recovery seams still pass. Use
an existing verified frozen-lock dependency staging copy, without installing:

```sh
WEEBDEX_FRONTEND_ROOT=/path/to/verified/frontend-stage \
  node --test weebdex-platform/tests/ci/test_browser_select_controls.mjs \
  weebdex-platform/tests/ci/test_browser_account_recovery.mjs
```

The test fails closed if either inspected Bits UI source file differs from its
recorded hash or the retained lock no longer pins 2.16.5. These eleven finite
checks do not constitute real browser acceptance; the corrected journey still
needs a coordinated genuine rerun.

## UI10 reset-lifetime localization

The same genuine diagnostic run passed the first five
`ui10-recovery-lifetimes` steps and failed its final abandoned-reset/session
refresh step. Those earlier passes establish that this separate failure occurs
after actual restricted recovery was reached; the title Trigger correction
does not explain it. The uploaded evidence has no inner failure identity, so
the authentication application remains unchanged.

The final step now records fourteen fixed IDs through the existing finite
journey recorder. They distinguish the current realm, installed session-response
hold, actual password submission, captured genuine normal-session response,
newer navigation and late response release, absence of stale completion and
duplicate writes, logout, rejected/settled old-password login, unauthorized old
session, successful new-password login, and rejected one-time-link replay. All
original observations and assertions remain required. The old-password login
response waiter and click are immediately joined with `Promise.all`, preserving
the required real 400 while handling a later waiter rejection after click
failure. No write is replayed and no authentication result is fabricated.

`test_browser_ui10_recovery_diagnostics.mjs` executes the actual outer checkpoint
wiring with explicit inner-action stand-ins, injecting a failure at each of the
fourteen points and checking exact failure propagation, listener/hold cleanup,
fixed-only receipts, and skipped later checks. It also executes the actual
old-password checkpoint body against explicit response stand-ins to require 400,
reject 200, and verify strict-mode failure accounting. Its eighteen finite cases
launch no browser, network, services or storage. Shared public schema/inventory
integration and a coordinated genuine rerun remain necessary.
