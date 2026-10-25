# Prospective signed report, draft and chapter browser qualification

## Scope and status

This is authored actual-browser coverage, not a browser pass. No Chromium launch,
fixture/database run, hosted dispatch, publication or uploaded evidence occurred
for this addition. Local qualification is finite source, controller/DOM,
proof-observer, scheduler, projection and fixture-guard testing. Earlier failed
hosted runs, auth refusal checks, synthetic event doubles and JSDOM results do not
establish these new end-to-end outcomes.

The selectable `public-signed-features` batch runs one
`public-signed-feature-recovery` phase against the actual prepared gate-on
application, PostgreSQL and original server. It does not use request interception
or install a mock application. Public-upload remains the separate unchanged
`public-feature-recovery` and `public-upload-recovery` pair; none of its upload,
original-byte, revocation, resume or cleanup requirements are replaced by this
addition. The default selection remains `public-upload`.

There are now 42 distinct phases in 13 logical batches, with at most 14 physical
fixture generations and 54 scheduled executions for `all`. Every distinct phase
is required for full-suite success. Running this selection alone cannot establish
full product/browser success. Existing diagnostic-only publication gates remain
mutually exclusive and source-associated.

## Exact prospective outcomes

The six ordered cases must all pass. A failed or interrupted case stops the
journey; subsequent cases cannot be silently credited.

1. **Identity:** the owned control accepts only a fresh ephemeral public key,
   binds a nonmoderator synthetic user with exactly `report.create`,
   `manga.create`, `self.chapter.update`, and seeds separate own/foreign drafts.
   The browser enters its generated key through actual Public identity controls,
   acknowledges backup and selects session-only use. Native auth remains401;
   no CookieSession, operator binding, publisher grant or persistent real key is
   involved. Initial durable reports are zero and own draft is `draft`.
2. **Report cancellation:** actual title Report dialog loads signed manga
   reasons; its details-required reason keeps Send disabled until details are
   supplied. Cancel sends no mutation and leaves the database report count zero.
3. **Report submission:** actual Send produces one signed UUID POST with exact
   displayed target/reason/account/version/details and201. Actual UI success and
   disabled replay are required. Database report must belong to that submitter,
   target and reason with exact body and `waiting` status. Reopening and cancelling
   must not create a second report. The actual My signed reports link then opens
   the cookie-free inspection route, loads the exact owner-only signed page, and
   shows the durable report. An Accepted status filter returns no rows without
   replaying a write or changing the durable report count. The passive observer
   requires exact signed scope/filter queries and refuses native report GET
   fallback or unsigned private report reads. These added browser steps are
   authored, not executed.
4. **Chapter metadata save:** actual owner edit navigation loads the real chapter;
   Save is enabled but removal is not granted. Saving only the title produces one
   exact signed PUT200 with displayed optimistic version/account and unchanged
   other fields. Durable version increments once, actual reload shows the saved
   title and sends no replay. Source event, manifest and original page bytes are
   freshly re-read and remain unchanged, not merely compared with cached files.
5. **Own draft preview and cancellation:** actual Add Title/list controls return
   exactly the owned draft and no foreign draft. Actual private preview works and
   links back to draft editing. Cancelling Submit produces no write and no state,
   version or foreign-draft change.
6. **Draft submission and removal review:** actual Submit produces exactly one
   signed commit POST201 with displayed version/account; UI and database become
   `submitted`, version increments once. Actual Draft filter now returns no results and no write; Submitted filter/reload retains
   the card without Submit. Actual removal-review confirmation produces signed
   DELETE202, explicitly queued review, not deleted. Database has one exact owner
   removal request and the original normal report, total two, with draft still
   submitted/undeleted at the same version. Foreign draft and immutable originals
   remain unchanged; reload sends no second mutation. Total writes are exactly4.

See [fixed fixture authority and schemas](../PROTOCOL_SIGNED_FEATURE_FIXTURE.md).
Real report/draft/chapter backend tests remain separate requirements.

## Proof, bounds and evidence

A passive browser request/response/finished observer verifies actual NIP-98 event
id and Schnorr signature, exact origin/URL/method/service/owner/time, unique event
and nonce, and actual serialized payload digest. It rejects cookie authority,
native mutation fallback, incomplete responses, failed terminals and replays.
Exact expected statuses and bodies are checked privately, never copied into the
sanitized projection. Public reads may remain unsigned; account/private reads
and writes must carry verified signatures.

The phase has a300-second owned process cap and240-second inner action deadline.
The new batch reserves1235 seconds including readiness, cleanup and phase
allowances, below the unchanged30-minute browser step. The whole job remains140
minutes. No sandbox relaxation is made. Fixture controls are public-key-only,
explicitly opted in, fixed-target and action/time/byte bounded. Ephemeral key
bytes are zeroed and observer listeners/body buffers removed in `finally`; actual
context closure plus owned process/fixture teardown are still required.

The added evidence projection is a closed six-case id/status vocabulary and
booleans only. It requires exact phase/batch/step association and actual context
and observer cleanup. It exports no report body, draft title, UUID, proof, nonce,
key, request URL, raw error, snapshot, database or screenshot. Existing bounded
JSON artifact allowlist and one-day retention are unchanged. Partial or malformed
evidence cannot be green. Source hashes are retained in
`test-evidence/public-signed-features-20261022/source-pins.json` and checked by a
finite pin test; incoming edits to any pinned source require a fresh reviewed
qualification, not automatic inherited acceptance.

## Local finite checks

From `weebdex-platform`, run the new finite suites:

- `node --test tests/ci/test_signed_feature_browser.mjs`
- `python3 tests/ci/test_signed_feature_phase.py`
- `python3 tests/ci/test_signed_feature_fixture.py`
- `python3 tests/ci/test_signed_feature_source_pins.py`

Keep the existing `test_public_feature_phase.py`,
`test_public_upload_hosted_profile.py`, `test_hosted_browser.py` and
`test_independent_public_upload.py` regressions. The existing report/draft logic
suite uses `node --experimental-vm-modules --test
 tests/ci/test_public_reports_and_drafts.mjs`; its actual-component DOM suite also
requires the reviewed `WEEBDEX_FRONTEND_ROOT` and
`WEEBDEX_BROWSER_DOM_NODE_MODULES` dependencies. Those are finite stand-ins for
transport/visual leaves, not a browser run.

A non-executing schedule check is
`python3 scripts/ci/run-hosted-browser.py --batch public-signed-features --plan-only`.
An actual hosted/browser run remains unperformed and subject to the root's
reviewed publication/run authority. No local browser retry is part of this work.
