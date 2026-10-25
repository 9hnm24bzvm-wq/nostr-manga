# Finite qualification, not end-to-end acceptance

The original 36-file pinset is preserved byte-for-byte in
`source-pins-initial.json`. The immutable `workflow-test-successor.json` records
the earlier workflow-only successor at `c5ff217`, whose complete pinset can be
reconstructed exactly from the initial pinset and its one replacement row.
`integrated-source-successor.json` preserves the reviewed report/account-fence
and chapter affordance/uncertainty integration at `55a55d6`, recording the exact predecessor/current hashes, six replacement rows and four new
report-inspection rows, with reviewed source commits and reasons.
`language-source-successor.json` preserves the reviewed public-language
integration at `9948338`, recording its one exact chapter-editor replacement.
Its historical 40-row pinset is reconstructed and hash-checked rather than
treated as the current checkout.

`source-pins.json` now associates the reviewed public-language composition
integration at `9929324`. `composition-source-successor.json` records exactly
one replacement: `platform-integrations/public-api/overlays.mjs`, from
9,973 bytes / `a8e4d816b2733b11cbf0e82495a3fec369aca9c53ad748eef8867a8bb6b9c606`
to 10,784 bytes / `5d5eaeedfd462822091252b42b898ddd655a4534549e9b04a4d358be53e92bf0`.
The prior/current 40-row pinset digests are
`d885d48f6816eb458566694c62fac617482ff4e757dd10a53a6e0e0693fd81cb` and
`0a9c9b85c074bd55a0587112b5b4aaa5cd6b2659b1d31c09a0caf6fe3c43ad8a`.
The current guard checks every source row, exact association bytes and literal
overlay reversal to the historical digest; unknown, partial, reordered and
reversed pinsets or changed association metadata are refused. Runtime guards
need no Git history. Initial/workflow/integrated/language receipts remain
byte-for-byte unchanged. These are source/evidence associations only, not
current execution, binary, browser, HTTP, hosted-run or publication acceptance.
The historical counts below and `checks.json` remain historical; combined
qualification is separate.

224 Python/Node checks passed against their then-current source, including21 new
proof/observer/composition tests,6 signed-phase tests,6 fixture guards, one36-file
source-pin check,63 existing report/draft controller and actual-component DOM
checks, and retained upload/diagnostic/evidence regressions. Raw logs are not
retained. `checks.json` records the exact finite commands and counts.

Three Rust fixture-unit tests and compilation passed at `f14cc5a`, before the
additive parent integration `5cef929` was merged. The signed-feature module bytes
are unchanged by that merge; the receipt records their digest and does not claim
that this prior compile executed a later combined application. The three checks
cover strict synthetic opt-in/closed public-key request, exact minimal grants,
and the signed-event-to-manifest binding. No database was started by these tests.

Plan-only selection reports one of42 distinct phases with
`browser_or_fixture_started: false`. The current scheduler retains13 logical
batches, at most14 physical generations and54 phase executions. Syntax checks and
`git diff --check` also pass.

Independent fixture/SQL/privacy review and separate browser/DOM/projection review
found and prompted fixes for the reason slug identity mismatch, incomplete header
observation, negative draft-state filtering, historical workflow hash association,
and handling of an unsigned public read aborted during navigation. The resulting
source still requires a real authorized browser run. Normal sandbox, job140 and
browser-step30 minute caps, existing upload success requirements, fixed diagnostic
publication association and sanitized one-day evidence scope are unchanged.

No Chromium, database fixture, hosted run or publication occurred for this
qualification. No actual SQL state transition, UI interaction or end-to-end case
is credited as passed. The browser cases are prospective with exact expected
outcomes described in `docs/verification/public-signed-feature-qualification.md`.
