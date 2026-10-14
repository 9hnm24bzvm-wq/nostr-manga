# Browser failure diagnostic follow-up

The full hosted attempt [38493778645](https://github.com/9hnm24bzvm-wq/nostr-manga/actions/runs/38493778645)
ran commit `cf76c09c1156da1db2a9b860eeb1eb99da3d85be`, tree
`9231fd361fa5f40521054040fc6b6355114492d7`. It failed. Fifteen actual
executions reached two uniquely passing phases, four uniquely failed phases,
and 32 pending phases. All ten fresh fixtures physically stopped with closed
ports, removed synthetic keys, zero owned survivors, and no forced cleanup.
The acceptance lifecycles and full suite did not pass.

Seven of ten startup repetitions failed only the seeded-title meaningful-content
check at the original 1200 ms checkpoint. Three startup repetitions passed all
19 checks. No JavaScript page errors were observed. The other failures were
account recovery, page-three device progress under timer and lock gates, and
the compact feed journey. The first artifact identifies these outer checks but
does not establish their exact inner assertion or root cause.

This follow-up changes diagnostic visibility, not application pass criteria:

- Account recovery records 18 fixed checks as pending, running, passed, or
  failed. Its response waiter is created only after the password input is
  filled, and concurrent click/wait failures are immediately observed.
- Device progress records 37 fixed checks and compact feed 16. Existing
  assertions, step boundaries, gate cleanup, and fixture stop-on-failure remain.
- Startup preserves its original frozen snapshot and assertion. A seeded-title
  content failure additionally receives a read-only probe bounded to seven
  seconds across all renderer calls. Four booleans describe document readiness,
  meaningful body, the known synthetic heading, and the known seeded route.
  The probe cannot turn the original failed checkpoint into a pass.
- A separate seeded-title network sample admits at most 64 document/API
  responses or failed requests, avoiding the earlier global sample saturation
  by successful script loads. Only fixed paths/categories and bounded statuses
  survive projection; query strings, values, and raw errors never do.

The public contract still allows only three canonical JSON files. It requires
the new diagnostic objects for the instrumented phases, rejects extra/private
fields and invalid identities/statuses, and preserves missing or interrupted
failed observations explicitly. Original runtime binaries, browser pins,
permissions, timeout budgets, fixture ownership, and cleanup are unchanged.

Local deterministic seams prove checkpoint wiring, unchanged exception
propagation, finite deadlines, and closed public schemas. Browser/API actions
in those seams are explicit stand-ins. They do not prove application behavior,
resolve the original failure, or establish any additional browser phase pass.
A fresh reviewed hosted run is still required.

The exact diagnostic push branch is `ci/browser-failure-diagnostics`; the job
guard admits only that branch or the existing `ci/hosted-browser-e2e` branch in
the exact public repository. Diagnostic publication leaves the existing CI head
in place. Selection remains `all`: one frozen preparation/build supplies ten
fresh startup observations and the original journeys where their startup gate
passes. There is no failed-startup bypass, retry-to-pass rule, or selective
failure erasure. A partial diagnostic result is not full-suite acceptance.

## Follow-up after run 38502322656

The next diagnostic run used public `0ee583e53a698975c17d19c679736146a1cea5f2`.
Its verified artifact retained 21 actual executions: two uniquely passing
phases, ten failed phases, and 26 pending. All ten startups passed their
nineteen original content assertions; one startup still failed its Node exit
and finalization gate. Three executions recorded an uncaught exception during
browser close. All ten physical fixture teardowns stopped cleanly. The full
suite and acceptance lifecycles remained failed.

The next reviewed candidate separately owns denied CONNECT peer-reset teardown
and corrects the recovery boundary's native settings-successor response contract.
It also refines existing local/signed library and compact preflight diagnostics.
Local library has 25 fixed checks; all eight signed phases share 23 phase-prefixed
checks. Compact has 21, with the original fixture-total parent preceding five
nested response/status/type/range checks. Its original seeded-count predicate
and every original journey assertion remain. Compact's public HTTP observation
is one of eight fixed outcomes, or explicitly `unavailable` before observation.
No status number, body, URL, error text, key, identity, or stored record is added.
A missing/invalid outcome cannot support a passing compact phase.

The central inventory is checked directly against the actual JavaScript exports.
All instrumented phases require their diagnostics object; duplicate, reversed,
missing, unknown or extra-field rows cannot support a pass. Unentered or failed
observations remain finite and explicit. The publication keeps the same guarded
branch, `all` selection, 38 unique phases, ten batches and 49 planned executions.
A fresh hosted run is required to establish the candidate's actual results.
