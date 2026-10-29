# Bounded synthetic reader verification precondition

This is **test setup, not email verification**. The application and Rust backend
have no corresponding bypass. Organic registration and verification instructions/
resend are observed before using this helper; actual token verification is covered
separately by the real auth integration tests. Browser results after its use must
say they used a simulated verified-account precondition.

`tests/runtime/browser_fixture.py --reader-verification-control` enables a single
POST on the disposable loopback **mail fixture**, not on the site/API server:

- Path: `/fixture/reader-verification`
- Exact JSON: `{"email":"synthetic-reader-final@example.invalid"}`
- Off by default; incompatible with licensed/non-synthetic content mode.
- Requires an owned `weebdex_browser_test` database, ready non-draining fixture,
  at least 30 seconds remaining, and the exact fixed email (no normalization).
- The user must already exist from the fresh browser registration: a runtime-new
  UUID ID, unverified, enabled, exact ordinary `["user"]` role, and an active
  unrevoked/unexpired native session. Session existence is checked without reading
  any session ID/token/password fields. No seeded uploader/staff account qualifies.
- A transaction locks the eligible user and updates **only** `verified=true` and
  `version=version+1`. No role, permission, group, password, token, session or cookie
  is created/changed. Statement/lock bounds are 3 seconds/1 second; owned psql is
  capped at 15 seconds.
- The one attempt is reserved under lock before SQL, including refusal, timeout
  or uncertain outcome. Never retry a lost response: inspect the nonsecret
  `reader-verification.json` receipt. It marks simulated setup and explicitly says
  organic email verification was not completed. Exactly one updated row is required.
- Cleanup includes the same nonsecret receipt. No mailbox link is retrieved or
  persisted by this helper. No application authentication/authorization code changes.

Tests: `python -m unittest discover -s tests/ci -p test_reader_verification_fixture.py -v`
(from `weebdex-platform`). These are actual-helper guard/SQL-scope tests with a
stubbed DB boundary, not a claim of real-email or browser verification success.
