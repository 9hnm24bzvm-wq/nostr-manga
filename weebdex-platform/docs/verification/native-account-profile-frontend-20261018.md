# Native account public-profile editor successor

Source `7a55916b05eab61ee5e7ec3039f29a00d1871027` is isolated from the prior
755-case source freeze. It restores the original Biography/Contact Info editing
experience through the native user settings route. A deliberate Edit profile link
in actual account following carries the captured canonical account UUID. The
editor checks exact native-public-user presentation, path, explicit account query
and signer generation, then requires a signed self-only preview before revealing
editable fields. Arbitrary profiles do not trigger a signer prompt.

The separate profile client binds accountId, exact canonical target and actual
shared native users.version. Both response provenance copies must agree. Only
changed description/discord/twitter/website fields are sent; omission preserves,
empty string clears. Rust Unicode White_Space normalization is reproduced before
UTF8 bounds. U+0085 is trimmed and U+FEFF is preserved. Actual roles are bounded
to128 nonempty UTF8 strings of at most128bytes. Native/public presentations retain
read-only markers; neither Session nor device preferences or Nostr identity data
are installed or edited.

The original settings parent was still cookie-only. An exact public-readonly
branch now lets the dedicated signed consumer mount; native-cookie else behavior
and gate-off remain intact. The outer user discussion wrapper was independently
checked: its existing unavailable context already handles absent threads, so no
fake thread/counters or outer-provider source change was needed.

## Focused qualification

`test-evidence/frontend/native-account-profile-20261018/` retains188/188 passing
outcomes across nine files:34 new actual-client cases,23 compiled Svelte/JSDOM
cases and131 affected regressions. There are zero failed/skipped final cases.

The DOM suite includes the actual composed outer user layout, real public
missing-thread Provider and UnavailableContext, User tabs, corrected settings
layout, original settings page and profile editor. Signed transport/native HTTP
and leaf UI/media/follow primitives remain explicit stand-ins. The tests cover
clear versus omission, native/auth revision changes, account rebind, invalid
provenance/receipts, pending/repeated actions, query-only navigation while preview
or signing waits, post-dispatch navigation, a newer unsent draft, explicit read
without write replay after a lost receipt, and readonly post-save presentation.

The retained historical Unicode repro has two before-fix failures; final exact
Unicode cases pass. The initial DOM harness needed its missing HTMLMediaElement
global added; that setup failure did not change production behavior. Independent
source reviews found the genuine settings-parent gate and role-bound mismatch;
both are corrected and re-reviewed.

All679 canonical frontend files remain byte-identical to f2. Registry growth is
explicit:48 original sources, nine availability pairs, three media and12 feature
exceptions. Existing preparation checks exercise both product gates and preserve
native-cookie markup outside the exact extensions. No dependencies were installed
or copied. Full strict/Vite qualification waits for the next composed freeze;
previous755/strict/build evidence remains historical. Backend native-profile
Rust/HTTP acceptance has its own API-owner source and receipts; this finite frontend
evidence does not claim an actual TS/live-profile bridge or native browser.
