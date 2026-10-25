# Explicit signed account blocklists

Source: `cd9d944` → `b81af4d` → `0756e7d` (base `0395fbc`).

The preferences form now offers **Manage account blocklists** whenever a public signer is selected, independently of native cookie login or local form changes. It previews the actual explicitly bound account and its current settings, hydrates existing UUID references, searches typed group/native-user directories, and requires a deliberate save. Only changed group/uploader lists go through the existing account/version/timestamp/template merge. Device defaults, cookie Session, native settings storage semantics and public-profile Block fencing are unchanged. Unavailable references remain server-preserved.

## Actual evidence

- 46/46 focused tests pass: 12 actual-controller cases, 8 actual compiled Svelte/JSDOM lifecycle cases, and 26 existing settings tests.
- The retained layout query-prefix regression failed on the first candidate, then passed after exact-one-delimiter normalization. Both logs are retained.
- Actual enabled composed strict reached one structural typing error and 12 existing warnings on `b81af4d`. `0756e7d` replaces the dynamic assertion with explicit typed fields. Final source strict remains pending the coordinated combined run. The earlier missing-cache-alias setup failure is also retained.
- No heavy build, database fixture, genuine browser, remote publication or deletion was performed for this batch. JSDOM does not establish native-browser portal/layout or live signed HTTP acceptance.

Reproduce finite checks with the already pinned frontend/JSDOM roots supplied as `WEEBDEX_FRONTEND_ROOT` and `WEEBDEX_BROWSER_DOM_NODE_MODULES`:

`node --experimental-vm-modules --test tests/ci/test_account_blocklists_client.mjs tests/ci/test_account_blocklists_dom.mjs tests/ci/test_account_settings_client.mjs tests/ci/test_account_settings_ui.mjs`

All fixture account names, UUIDs and identity stand-ins are synthetic.
