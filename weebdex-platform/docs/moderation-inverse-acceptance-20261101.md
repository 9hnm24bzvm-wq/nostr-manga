# Local chapter moderation inverse acceptance

Status: prepared on 2026-11-01; **not a browser pass or a fresh HTTP pass**.
Source baseline: `1a491e48cebd716ef9eb0d3bf072a4b008ae4440` plus the focused
`staff_workspace_http.rs` assertions described below. Never inherit a pass from
an older build or a lost runtime.

## Automated seam gate

`backend/tests/staff_workspace_http.rs` uses the real router over loopback TCP,
real disposable PostgreSQL and synthetic fixture principals. It verifies native
moderator/staff role-and-grant checks, review-before-write, local hide/correction,
source-unavailable refusal, stale target refusal, retained immutable audit and
idempotent commands. Added chapter witnesses require actual restored visibility,
exact version 3, unchanged parent/uploader, one acceptance and one correction,
and identical receipt on exact retry. A new inverse preview then refuses with
409. These are synthetic seeded catalogue records, **not a newly published
protocol chapter or rendered-browser proof**.

Compile before starting the bounded fixture. From `weebdex-platform/`, with the
project's qualified Rust environment and PostgreSQL binaries available:

```sh
cargo test --locked --manifest-path backend/Cargo.toml \
  --test staff_workspace_http --no-run -j2
# Use the executable path emitted by cargo, not a guessed hashed filename.
PLATFORM_TEST_PG_BIN=/absolute/qualified/postgresql/bin \
  scripts/run-auth-tests.sh /absolute/staff_workspace_http-test-executable \
  --ignored --test-threads=1 --exact \
  native_staff_reports_profiles_and_corrections_are_scoped_fenced_and_audited
```

The runner owns the isolated database and checks loopback binding, process birth
identities and cleanup. Save its terminal exit and cleanup receipts, source hashes
and command. Do not interpret a spawned process or an empty test selection as a
pass. Do not hand-edit a live or browser account's session/grants to reuse this
seam fixture as browser acceptance.

## Browser prerequisites and minimal authority

Use a fresh bounded disposable integrated-publisher fixture, loopback only, with
current compiled frontend/backend and configured terms/privacy/guidelines. Record
the exact source/build digests, origin, runtime owner and cleanup deadline. Use
synthetic images and metadata only. Register/sign in via the normal UI and match
the exact native account ID. Do not reuse IDs or approvals from destroyed fixtures.

The chapter path itself needs:

- ordinary `report.create` to submit the removal report;
- a moderator role plus `report.update` and `chapter.remove` to accept it;
- a staff role plus `report.correct` and `chapter.restore` to undo it;
- `report.history` if verifying the decision-history UI.

The supported access UI uses reviewed bundles rather than arbitrary individual
grants. `local-moderator` additionally includes report deletion, title removal and
group editing; `decision-corrector` additionally includes title restoration and
leadership reversal. Read their current full definitions in
`backend/src/domains/staff_authority/policy.rs`. Neither bundle grants publication
or original source deletion. Bootstrap also has its own security-administration
scope. Obtain required exact-account confirmation before any new access grant;
do not bypass it with SQL or substitute broad admin access. Native immediate
publication uses its separate supported CLI and its own exact-account review.

## Rendered journey and required witnesses

1. Publish a synthetic chapter through the normal upload UI using the current
   native immediate-publication workflow. Record title/chapter public and native
   references, publication receipt/source event reference, ordered page hashes
   and original content availability. Confirm reader pages work on direct reload.
2. Use the rendered report UI on that exact chapter, select chapter removal and
   submit a synthetic explanation. Capture report reference and initial waiting
   state. If deletion directly hides an owned chapter, that is a different path:
   do not count it as the report-decision inverse journey.
3. Open Staff workspace → Moderator queue. Select the exact report and choose
   **Review report**. Verify the displayed consequence is an instance-local hide.
   First cancel and confirm the chapter remains readable. Reopen review, enter a
   reason and choose **Confirm acceptance**.
4. Verify accepted status and the chapter's disappearance/unavailability through
   a fresh reader navigation or reload, not just stale in-memory content. Verify
   its original source event/content remains retained; local hiding must not
   perform original deletion. Keep these non-UI checks separate in the evidence.
5. Filter the queue to Accepted, select the same report and choose **Review local
   correction**. Verify the exact target and local-only consequence. Cancel once
   and confirm it remains hidden. Reopen, enter a reason and choose **Confirm
   local correction**. Reauthenticate normally if freshness is required.
6. Verify report status becomes Refused, direct reader reload shows the same
   ordered pages, and original event/content hashes match step 1. Decision history
   must retain both acceptance and correction; the initial decision is not erased.
7. Reload the staff page and reader once more. Verify the restored state persists.
   No second correction should be offered as though the accepted decision still
   exists. If a request result is unknown, use **Check action receipt** instead of
   repeating the mutation blindly.
8. Stop the fixture under its owner; verify closed listeners, no remaining owned
   descendants and removed temporary secrets. Record cleanup separately from
   test success. Do not publish credentials, runtime databases or private raw logs.

## Limits

The existing HTTP harness can run without user interaction once dependencies are
available; its fixture principals are internal test setup. It does not establish
supported administrator bootstrap, real browser signup, grant consent, publication
or retained published-source behavior. Those remain distinct browser/integration
witnesses. Independent signer and organic email verification are separate gates.
