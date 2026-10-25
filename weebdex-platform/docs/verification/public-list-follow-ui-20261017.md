# Published-list discovery and genuine account following

## Implemented boundary

This isolated frontend batch consumes the existing original-signed public-list projection and backend follow contract `e6a5a749718cef4f1c63412af73c5197c3aad99f`.

- `/list/[id]` displays the actual public list name, publisher and available projected titles in original signed membership order. The detail response must match the exact canonical requested UUID; unproven alias substitutions fail closed. Title hydration is exact-ID bounded and source head/root/owner/name/membership are checked again before installing the result. Source withdrawal, pending alias projection and changed snapshots remain errors, rather than fabricated empty lists.
- Verified protocol-publisher profiles gain a native `Lists` tab. It uses the profile's actual returned canonical UUID, including when navigation used an alias. Native private-list profiles are not converted into signed public-list publishers.
- The existing library header links to `/following/lists`. This intentional view reads genuine bound-account list follows and actual unavailable-follow counts. It never changes the anonymous device library mode or publishes local lists.
- `Follow list` opens a normal site dialog. Anonymous list viewing performs no signer/account requests. The dialog uses an actual request-bound `/user/me` observation, keeps that canonical native-account UUID separate from the signing pubkey, proves the configured service, and captures the current signer generation/catalogue/origin.
- Each POST includes the captured `expectedAccountId`, actual preview `expectedHead`, and a version only when known from a successful status read. A status 404 does not imply version zero; its unknown tombstone version is omitted. DELETE includes account/version intent and never a source head, so an explicit saved-follow removal can still be requested after withdrawal. The server remains the authority for visibility, permission, account binding and conflict checks.
- Unconfirmed responses, denials, stale account/version/head and response loss leave the control requiring a new read. There is no automatic command replay or optimistic confirmation. Closing a dialog, superseding a load, leaving the component or changing signer generation prevents late account state from being installed.

The supplied 679-file frontend is unchanged. Additive routes/components and exact reviewed overlays provide the UI. The only new retained-markup exception is the existing library-header link; the profile tab uses the existing native tab renderer. Public catalogue activation remains opt-in and the full saved-link migration/browser acceptance flag remains false.

## Checks

Evidence: `test-evidence/frontend/public-list-follow-ui-20261017/`.

- 24 actual client-module finite regressions and 11 actual component-script regressions passed. HTTP, signer, proof and lifecycle boundaries are explicitly finite stand-ins; no real account or network was used.
- The combined 278-test rerun passed, including rating, publication/signer isolation, exact chapter/volume numbering, local UUID library, discussion mapping, signed transport, upload adapter/provenance and composed bootstrap regressions.
- Real production preparation was executed against fresh copied canonical source for both gate values. It copied each new route, validated all four narrowly enumerated public feature exceptions when enabled, and left the original source inventory unchanged.
- Composed production-source strict checks passed for gate 0 and gate 1, each with zero errors and the existing 15 warnings. The full source summaries are retained with this batch.

These are source-contract, finite seam and strict-type/preparation results. Genuine rendered browser interactions, browser IndexedDB persistence, cross-tab/reload behavior, real external-signer prompts, deployed services and load qualification remain unverified. No production/parity claim is made. Public-list editing is not introduced here; device/private lists, per-key signed publication, and native-account follow observations remain separate domains.

## Independent source review

The browser/control auditor independently reproduced an initially accepted unrelated list UUID, then reran the same assertion after exact-target binding: before failed, after passed. The auditor found no remaining blocker in the frozen source/account/version/head/generation seams and independently reran all 35 dedicated client/component cases. Genuine nonpreferred aliases exist server-side, but no typed list/user alias proof is exposed; the UI deliberately fails closed for unproven alias detail responses. The independent repro and before/after receipts are retained.
