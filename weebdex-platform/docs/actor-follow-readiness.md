# Current-actor Group/User follow readiness

This local correction is based on commit
`01a593b7d167add9a4c8e7936d4e86abd10fcf21`. It changes only the external
Group/User follow integration and its bounded verification. The immutable
676 supplied frontend files and three reconstructed route files remain unchanged.

## Bug and scope

The round8 Group/User overlays read follow status only on mount and returned
immediately while Session was still undefined. Public route loading can finish
before the actual asynchronous `Auth.Check`. Later Session completion displayed
a Follow control without reading that actor's existing status. A later actor
switch likewise retained the previous actor's boolean. The admitted mutation
could therefore choose the wrong idempotent POST/DELETE method. Existing actor
ownership fences prevented this from being a cross-account authorization escape.

## Correction

`platform-integrations/frontend-behavior/actor-follow-state.ts` supplies one typed
controller to both actual composed components. It subscribes to the actual
Session store. Every Session revision resets readiness synchronously; a target
and route pre-effect also resets and re-reads. Anonymous state never reads or
mutates. Only 204 (followed) and 404 (absent) establish known status. Rejected,
missing and other-status responses stay unknown.

Reads and writes capture the existing mutation owner and a controller generation.
Old actor, route, target and destroyed-component responses cannot change current
state. The handler checks live target identity even before the target pre-effect
runs. Mutations retain the existing mutex, actor/target assertion, API intent and
POST/DELETE contracts; state changes only after a current 204 acknowledgment.
Repeated clicks and status refresh while a mutation is pending do not race it.
A pending write is tracked independently from view generations. A matching
actor/target cannot read ahead of that write after Session refresh or an
actor/target round trip. An acknowledged stale write starts fresh reconciliation
only after settlement; a canceled/unknown result stays unknown with explicit
read-only retry. Cancellation does not prove server rollback or ordering, and no
mutation is replayed.

Existing permissions, self-follow visibility and block availability remain in
force. Six exact external markup exceptions are manifested: each existing
Follow button gains a readiness/busy disabled expression, and each existing
label offers “Retry follow status” after a status/write error; and each existing
Block visibility guard requires a known absent follow or an existing block to
unblock. Direct Block handlers also enforce current-target known absence, while
legitimate Unblock remains available during a follow read or error. Retry performs
a status read only. Existing classes, icons and DOM element structure remain
unchanged; exact rendered-markup parity is not claimed for these necessary
state/label/availability changes. Chapter-count reads remain independent.

## Verification

- `node --test tests/ci/test_actor_follow_readiness.mjs`: 52 controlled-completion
  cases execute actual composed Group/User scripts, actual Session initialization,
  actual Mutex, authority registry, mutation owner and preference apply/capture
  helpers. Svelte runes/stores and API
  completions are explicit stand-ins, not browser execution.
- `node --experimental-vm-modules tests/ci/test_frontend_semantic_contracts.mjs`:
  132 existing bounded cases passed.
- `node --experimental-vm-modules --test tests/ci/test_frontend_bootstrap.mjs
  tests/ci/test_recovery_successor_contract.mjs`: 43 existing cases passed.
- Actual production shared → reader → local-library composition was materialized
  with the existing strict checker: zero errors and the same 15 residual warnings;
  592 validated source maps include the one new controller. All 679 canonical
  frontend hashes remain unchanged. The projection validator's exact map count
  and corresponding negative-control message advance from 591 to 592 solely for
  this added production module.
- Both existing dependency caches were protected by the existing read-only guard.
  No package installation, browser, database, application service, CI run or public
  write was performed.

Machine-readable details: `test-evidence/frontend/actor-follow-readiness/summary.json`.
