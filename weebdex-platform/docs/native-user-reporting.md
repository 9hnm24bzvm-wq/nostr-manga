# Request-bound native profile-discussion reporting

This is the separate required reporting follow-on to the reviewed native-user
navigation/discussion slice. It does not extend the public source-event report
categories and does not publish a protocol event. Native cookie routes remain
unchanged. The original supplied frontend remains unchanged under overlays.

## Target and authority

Only a visible persisted native user (`native_public_user`, `NativeResource`)
and its actual stored profile thread are eligible. Protocol publisher profiles
have no inferred native discussion. The reporting frontend shares the exact
already-captured signed discussion account/service/signer scope. It does not
create a second actor, use a cookie account, or reinterpret an npub.

- GET `/api/v1/nostr/user/{uuid}/discussion/reports/context` captures the expected
  account, actual stored thread, target type, and the active reply ID/version when
  reporting a reply. It reads enabled actual native reasons and their versions.
- POST `/api/v1/nostr/user/{uuid}/discussion/reports` additionally captures the
  expected canonical user, actual native reason ID/version, and reviewed details.
  There is no generic `/report` category widening or new schema.
- The backend uses genuine `BoundAccountWrite`, preserves actor-first ordering
  with an adapter `FOR UPDATE NOWAIT` upgrade before acquiring targets, and uses
  NOWAIT target user/thread/reply locks. Contention is a known409 before report/audit
  insertion. The persisted enabled reason/version remains share-locked through
  the native core insert and private audit commit.
- Existing native permissions, visibility, duplicate waiting-report rule,
  twenty-per-hour rule, and transaction rollback apply unchanged.

The complete enabled reason collection is limited to1,000 rows; an overflow
rejects the entire response instead of truncating choices. SQL projects reason
text only within8,000 UTF-8 bytes and rejects invalid rows. A bounded counting
writer validates the complete serialized response against1MiB before allocating
its response body. The frontend enforces the same per-reason UTF-8 text bound.

## Review, limits, receipt and uncertainty

Native details are trimmed using native Unicode whitespace semantics and bounded
to 20,000 UTF-8 bytes. JSON escaping is allowed only through an exact report POST
131,072-byte request budget. Generic source-report and unrelated endpoint limits
are unchanged. Signed response reads retain the existing bounded JSON transport.

The mounted profile UI explicitly reviews the actual target, account, reason and
reason version, and details before submission. The reviewed payload is a frozen
capability tied to the current discussion and reason snapshot. Receipt validation
requires the exact transmitted JSON digest, POST method/path, native provenance,
and a narrow waiting-report result. Account/service postflight must also pass.

An independent per-account/profile/thread report fence is persisted before send.
Known refusal or provably unsent requests require a fresh review. Unknown outcomes,
malformed receipts and post-dispatch verification failures retain that fence.
Reading, navigation, cancellation, reopening or a known-unsent follow-up read do
not convert an uncertain write into failure or permit replay. Ordinary discussion
reads remain available when a report outcome is unknown.

## Qualification scope

[Actual backend qualification](../test-evidence/native-user-http-20261022/README.md)
records successful disposable PostgreSQL/loopback HTTP at final source `1477d6b`:
account/reason/target rebinding, permissions, private response boundaries,
duplicate/rate rules, exact receipts, rollback, reciprocal actors and concurrent
native-cookie thread mutation audit-FK contention. A reason-lock barrier proves
that reason changes cannot overtake the private core insertion. The exact
missing-bound-authority403 and stable-actor/captured-account409 cases are distinct.
All fixtures use synthetic data and were stopped and privately archived with
verified cleanup. Original failed assertions and their reviewed test-only
corrections remain recorded rather than being relabeled as passes.

The earlier finite controller/transport and mounted original-route tests retain
their own exact source scope. Full composed finite/strict/build after integration
remains required. Neither mounted DOM mocks nor stream spies constitute
native-browser terminal/cancellation proof. No hosted browser, production
credentials, real moderation reports, publication or external action is part of
this slice.

## Personal native report inspection

The separate [signed native report inspection](native-report-inspection.md) route
reads the actual bound account's submitted native thread/reply reports without
requiring a browser-login cookie. Its private scope never widens for moderators.
It does not clear the submission controller's uncertainty fence or infer the
original write's outcome from a matching report or an empty result.
