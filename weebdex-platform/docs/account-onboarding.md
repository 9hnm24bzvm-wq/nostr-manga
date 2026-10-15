# Account-creation welcome

The native registration form is preserved. After a **successful HTTP 200
registration response with an active session and identity ID**, the current form
refreshes its session and shows a short native Card. This is an intentional
rendered extension mounted only in `auth/register`; the supplied attachment files
are never edited.

The welcome explains that saved browser titles remain in the browser, library
storage can be chosen later in Settings → Library, and separate public sharing is
optional. An email-verification link appears only when the registration response
explicitly identifies an unverified email address. Opening the guide does not
request verification mail or claim that mail has been sent. Continue uses the
captured safe intended return destination.

## Lifecycle and privacy boundaries

- The guide exists only in the fresh registration component's memory. Login,
  ordinary browsing, registration flow reads, already signed-in visitors, refresh,
  and query flags cannot create it.
- Existing auth-form ownership fences protect all async work and Continue. A
  late abandoned response cannot mount the guide or navigate an unrelated page.
- The refreshed account ID must match the just-created identity before the guide
  presents the ready state. A failed refresh, changed account, or lost session
  shows a sign-in continuation instead. The completed form is not revived.
- Pending and completed registration cannot be submitted again through queued or
  repeated controls. The original registration API and validation UI remain.
- Safe return URLs are restricted to the current origin, preserve local query
  strings and fragments, and reject authentication-loop destinations. Query
  values are decoded once by URLSearchParams. The login and verification
  consumers receive a narrow script-only safe-return repair too, so a literal
  percent or an already-encoded query value survives those existing routes. Retaining the safe redirect beside
  the existing flow ID lets refresh bypass the welcome without replaying signup.
- No library mode is selected, copied or imported. No signing extension is
  contacted, key is created or stored, public identity is admitted, or public
  action is published. Site-account recovery and public-signing-key recovery
  remain separate.
- The guide uses existing Card/Button components and utility tokens, the native
  max-width auth wrapper, normal document scrolling, a focused heading, live
  status, and at least 44px action targets. It adds no modal, accordion, dependency
  or global style.

## Integration and verification

The exact SHA-256/count-anchored registration additions live in
`platform-integrations/frontend-overlays.mjs`. The preparation wrapper validates
one narrowly scoped `fresh_registration_welcome` markup exception and includes
both the new component and helper in the integration inventory. Unrelated
retained markup and styles are still compared exactly after existing exceptions
are reversed.

Use the normal composed-source compiler, semantic checker, frontend build,
focused onboarding seam tests, and original-source parity check. These are
separate evidence classes: source and finite stand-in tests do not establish
actual rendered-browser acceptance.

At the implementation freeze, 114 selected finite tests passed (46 new onboarding
tests), the composed compiler passed 106 Svelte components and 65 scripts, and
strict semantic checks reported zero errors with 15 retained warnings. The owned
production build exited zero with no surviving processes and all 762 frozen
inputs unchanged. All 676 supplied attachment files and all 111 supplied static
files remain byte-identical. Real-browser acceptance is recorded separately and
was still pending when this source/build checkpoint was made.
