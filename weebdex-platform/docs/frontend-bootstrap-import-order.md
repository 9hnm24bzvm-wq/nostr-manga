# Frontend bootstrap import-order repair

## Source defect and narrow repair

The integrated API resolver substitutes the local-first `ChapterHistory` and
`MangaFollow` adapters. Those adapters import `library.ts`, which imports and
subscribes to the native `Session` store during module initialization. The
Session overlay also hydrates account preferences, importing `prefs.ts`.

Before this repair, preferences imported `Constants` from the application `$lib`
barrel. That barrel imports the API barrel before initializing its reader
constants. The shared-store entry therefore evaluated reader preference defaults
before `Constants.Reader.Scale` was initialized. The composed-source regression
reproduces `TypeError` reading `LimitHeight` at `prefs.ts`.

Changing only that preferences import was insufficient: Session-first evaluation
still entered the API/local-library cycle and read the Session binding before
initialization. The regression also reproduces this second failure.

Four exact, hash-anchored executed-source replacements remove these back-edges:

1. `prefs.ts` imports the constants namespace directly from `$lib/constants`.
2. `Session.ts` imports the authentication namespace directly from `$lib/API/Auth`.
3. Its existing session check calls `Auth.Check()`.
4. `Auth.ts` imports HTTP directly from `$lib/HTTP`.

The API operation and authentication path, normal-session authority binding,
preference hydration and local-first library behavior are unchanged. No delay,
exception suppression, network/CSP relaxation or backend change is introduced.
All 676 supplied files and all three pinned frontend additions remain byte-identical.

## Focused verification

Run from `weebdex-platform` with Node 24:

```
node --experimental-vm-modules tests/ci/test_frontend_bootstrap.mjs
node scripts/prepare-frontend-integration.mjs
python3 scripts/verify-frontend-parity.py
```

The regression uses the actual frontend source, all three composed overlay
transforms, and the actual local-library API resolver. It reverses only the four
new transformations in memory to reproduce the original failure, then reverses
only the Session/Auth repair group to demonstrate why preferences-only repair
is insufficient. Five entry orders are covered: shared stores, API, Session,
application barrel, and local library. Each initializes 87 composed modules.

For each order, the test checks real reader enum defaults, initial anonymous
session initialization, forced account preference hydration, A-to-B switching,
logout, native-authority and library-context fences, and explicit device/account
mode separation with retained local records. Its fake transport observes only
three credentialed GET checks to the exact `/api/auth/check` path.

This is a finite Node module-order and store-lifetime seam. External package and
browser APIs are explicit stand-ins; no browser, network, server, IndexedDB,
crypto operation or real persistent storage is exercised. Node's TypeScript
transformation does not replace a Vite/Svelte production build. The recorded
result is `test-evidence/frontend/bootstrap-import-order/result.json`.

The earlier hosted startup aggregates establish common undefined-property
errors but do not include their exact property or source location. This local
proof identifies a concrete source-composition defect; the unchanged real
startup acceptance phase must still confirm that the patch fixes the observed
browser failure. No all-features or pixel-parity claim is made here.
