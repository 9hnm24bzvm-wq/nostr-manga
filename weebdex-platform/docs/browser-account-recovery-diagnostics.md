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
