# Restricted recovery settings success contract

## Observed gate and source-proven defect

The verified hosted run `38502322656` on public source
`0ee583e53a698975c17d19c679736146a1cea5f2` / tree
`6ab0b95f672175d3aecf7dfd255924c56f566537` passed UI10 recovery's
`recovery-reset-realm`, `recovery-reset-hold-session`, and
`recovery-reset-submit` checkpoints. It failed `recovery-reset-held-session`;
the remaining ten checkpoints stayed pending. That gate waits for an actual
normal-session read after reset. The finite artifact does not itself prove the
reset POST's acceptance or distinguish an absent read from a rejected read.

Native `backend/src/domains/auth/flows.rs::settings` consumes the submitted
settings flow, rotates the browser session, creates a new settings flow with a
fresh ID and CSRF token, and returns `flow_body(&next, Some(&updated))` with
HTTP 200 and state `success`. Its response still has password-only nodes because
the response serializer uses the restricted recovery principal. The response
cookie has the newly rotated normal session; account authority must be read
separately.

The restricted `RecoveryBoundary.svelte` instead required the successful
response ID to equal the consumed submitted ID. Thus a genuine accepted native
reset was deterministically discarded before `Session.initialize(true)` could
issue the read that the browser checkpoint was holding. The bounded regression
reproduces that exact old guard against the native response shape. This is a
source-proven runtime contract defect, without a claim that the artifact alone
established the POST outcome.

## Correction and authority fences

An acknowledged reset now requires HTTP 200, state `success`, a fresh nonempty
browser-flow ID and CSRF token, the same return target, a same-origin settings
POST action whose sole query binds its successor ID, and password-only nodes.
Invalid success acknowledgements cannot start Session refresh. Acknowledged
writes clear the consumed local CSRF even if their successor is rejected;
they are never replayed.

The successor is never mounted or adopted as the current form. The original
request generation, URL, navigation epoch, and current-flow owner must remain
alive before and after the forced Session refresh. The reset UI remains
restricted until the actual composed Session store confirms a normal identity.
A successor response alone grants no account or library authority. Error and
non-success flows remain bound to the submitted ID, including Axios 400
validation responses. Cleanup captures and clears only the submitted password
field, leaving a newer replacement form's field untouched.

## Bounded proof and remaining browser acceptance

`backend/tests/fixtures/recovery-settings-success.json` contains only synthetic
IDs and tokens. A database-free Rust unit test executes the native `flow_body`
serializer and compares its complete restricted success shape to that fixture;
the two acknowledgement fields match those added by the native settings
handler. The Node regression also checks that the handler consumes the old
flow, constructs the fresh successor, uses that serializer, and rotates the
cookie.

The 42-case Node group executes the actual boundary script, actual form and
navigation owner, and actual composed Session and Mutex. Svelte runes/stores
and HTTP are explicit stand-ins. It reproduces the old guard, checks the forced
read while authority remains restricted, rejects malformed or foreign
successors, preserves same-ID non-success handling, rejects foreign validation
flows, and fences stale completions both before and during Session refresh.
Replacement-field cleanup and no duplicate password writes are covered.

From the repository root:

```sh
node --test weebdex-platform/tests/ci/test_recovery_successor_contract.mjs
cargo test --offline --locked --manifest-path weebdex-platform/backend/Cargo.toml \
  --lib domains::auth::flows::tests::recovery_settings_successor_matches_frontend_contract_fixture -- --exact
node --test weebdex-platform/tests/ci/test_browser_account_recovery.mjs \
  weebdex-platform/tests/ci/test_browser_ui10_recovery_diagnostics.mjs \
  weebdex-platform/tests/ci/test_recovery_successor_contract.mjs
node --experimental-vm-modules weebdex-platform/tests/ci/test_frontend_semantic_contracts.mjs
node --experimental-vm-modules weebdex-platform/tests/ci/test_frontend_bootstrap.mjs
python3 weebdex-platform/scripts/verify-frontend-parity.py
```

The existing verified Rust toolchain/cache can supply `CARGO_HOME`,
`RUSTUP_HOME`, and `CARGO_TARGET_DIR`; no dependency install is needed. These
checks use no service, database, browser, network, or runtime credentials. The
changed component also compiles with the existing verified Svelte compiler
with zero warnings. Canonical frontend source remains unchanged. A coordinated
genuine browser rerun must still establish the held-session and subsequent
old/new-password, logout, and link-replay assertions; no assertions were changed.
