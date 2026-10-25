# Caught browser failure diagnostics

## Retained second hosted attempt

Run `39144824203`, source `3e019d2d` (local reviewed `c963d383`), passed
the actual native compact-feed PostgreSQL gate and reached a ready public-upload
fixture. Its sanitized artifact archive has SHA256
`128c3366fc3def1a69606572239f77c6f7a6dd2d673ff7652b18d8e2a585bfe6`.
The report finalized with exit 1 and zero steps. Fixture teardown closed all
recorded ports and processes. The earlier lifecycle was unsuccessful.

The passive Node fatal monitor reported no uncaught failure, while a top-level
error was caught. The old `finally` transitions replaced its stage with
`complete`. Successful deny-proxy closure implies its listen succeeded. A
finalized report also excludes failures in imports, runtime loading and early
fixture parsing, which occur before report initialization. The remaining source
region includes sandboxed Chromium launch, context setup and public-upload
preflight. The archive does not identify which failed. Its default public-upload
zero counts are not independent evidence of zero traffic. No sandbox failure or
application defect has been established from that archive.

## Fixed private-to-public observations

- Preserve the first caught stage before cleanup. Keep it separate from Node's
  passive uncaught-exception/unhandled-rejection observations.
- Export only a closed stage/category pair. Unknown or inconsistent pairs become
  explicitly unavailable and cannot pass the gate.
- A sandbox-refusal category requires the launch stage plus an exact recognized
  Chromium refusal phrase. Other launch failures remain generic. This identifies
  a private diagnostic banner, not an independently inspected host-policy cause.
- Preserve actual browser/context/page creation booleans. Absent, malformed or
  inconsistent private fields become null, never inferred false or true.
- Separate the public-upload declaration, signer dependency/key setup and
  observer setup into fixed preflight stages, restoring `running-checks` before
  the first actual step.
- Private text, exception classes, paths, URLs, keys, argv and environment remain
  outside the public projection. No raw browser/fixture log upload is added.

The launch retains `chromiumSandbox: true`. All original nonzero-exit, failed-step,
uncaught-failure, cleanup, native gate and upload proof requirements remain. An
observed caught failure or incomplete creation can never become a pass, even
when other receipt fields claim success. The exact serialized schema requires
the new observations; historical artifacts retain their historical scope.

Independent negative review additionally reproduced two pre-existing passive
monitor defects: a throwing `code` getter changed Node's original fatal exit,
and a changing getter could bypass the fixed-code allowlist on its second read.
Fatal-code extraction now reads exactly once inside a guard, defaults to
`unknown` and cannot throw or copy the second private value. The original
diagnostic receipt remains historical; the follow-on receipt pins this guard.

## Qualification boundary and possible next action

Finite controls execute the lifecycle recorder, actual catch wiring and actual
public-upload setup prefix with explicit stand-ins. Python controls verify the
closed projection and serialized gate, malformed/private values and failure
association. They launch no browser, service or database. Native gate evidence
and the second hosted failure remain unchanged.

A future action would be one separately approved push of the frozen diagnostic
successor to the existing admitted CI branch, selecting only `public-upload`.
It retains the reviewed 140-minute job, 30-minute browser step and 3600-second
runner bounds, fixed official dependencies, sandbox and sanitized artifact
allowlist. It would test the existing current user journey and distinguish any
new refusal by the fixed observations above. No third run is authorized or
triggered by preparing this source. A sandbox refusal must stop rather than
change security flags or host policy.
