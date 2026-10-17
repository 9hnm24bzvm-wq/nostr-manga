# Separate public-upload hosted profile

The current scheduler now covers 40 distinct current phases in 12 fresh-fixture
batches, with at most 52 scheduled executions. This is an explicit extension of
the retained 39-phase/51-execution plan. It is not evidence of historical
forty-case equivalence or of a passing browser run.

The `public-upload` batch contains only `public-upload-recovery`. It runs the
reviewed positive replacement journey with a separate gate-on UUID catalogue
artifact. It does not run the native Startup phase: that phase uses the retained
`SeedManga1` native routes and belongs to the default-off native profile. All
previous batches keep their existing Startup, ordering and cleanup contracts.

## Separate preparation

Hosted preparation explicitly builds the native profile with
`PUBLIC_MANGADEX_CATALOGUE=0`. For selection `all` or `public-upload`, it then uses
the same installed, pinned dependencies and owned build runner to create a
second profile with the flag set to `1`. No second dependency fetch is added.
The second build has a 900-second bound and its artifact recorder has a
60-second bound. Existing global preparation/job budgets still apply; exhaustion
does not become a successful build or a passing suite.

The gate-on artifact, complete manifest and build receipt are stored separately
under the hosted build stage and hash-associated in `plan.json`. The original
native artifact and its five integration/artifact/CSP/status receipts are restored
byte-for-byte even if the supplemental build fails. Product defaults stay off.
The source snapshot must remain unchanged across both builds.

Before allocating the public fixture, the scheduler checks the exact profile
paths, gate-on build receipt, complete regular-file membership, every member's
size/hash and the index hash. A missing, gate-off, changed or linked profile is a
failure. Native artifacts cannot qualify by being relabelled as public artifacts.

## Fixture and evidence gates

The public batch opts into `--public-upload-controls` on one new synthetic
combined fixture. The normal supervisor owns all descendants, both protocol
recipients, database, browser/proxy lifecycle and actual port closure. Browser
security flags and external-request guards are unchanged.

Before submitting the browser command, current-fixture readiness must include:

- the matching current backend/frontend generation and real protocol readiness;
- the enabled `public-upload-v1` declaration;
- the exact newly hashed operational receipt associated with this fixture;
- a complete genuine HTTP413 early-response check, including body-send facts,
  completed response and settled deadline worker.

The recovery phase retains its 600-second browser deadline. The fixture itself
retains the scheduler's existing 3600-second lifetime and normal cleanup reserve.
It uses fresh UUID-bound resources created by its private fixture controls.

Public artifacts contain only a closed boolean/count projection. A passing row
also requires the exact 21 mutations; stable replacement identity/order/neighbours
and version; actual signed new-document preview requests; matching committed
originals; closed reader/observer/context; existing finalized process diagnostics;
and the scheduler's normal fixture/lifecycle gates. Raw keys, identity values,
paths, payloads, original events and private runtime records are not projected.

An isolated public batch may establish only its selected phase. The current full
suite remains false unless all 40 current phases and every required lifecycle
pass. The retained previous 39-phase scope cannot satisfy that claim.

## Local checks and remaining execution

The deterministic profile/schema tests use explicit build/report stand-ins.
The adjacent hosted-controller suite also exercises its existing short-lived
owned OS-process cleanup seams. These checks do not run Vite, PostgreSQL,
strfry or Chromium and cannot establish an actual second-profile build or UI pass.

```sh
python3 -B -m unittest discover -s tests/ci -p 'test*hosted*.py' -v
python3 -B scripts/ci/run-hosted-browser.py --plan-only --batch all
python3 -B scripts/ci/run-hosted-browser.py --plan-only --batch public-upload
```

The new local scheduler/profile implementation has not been published or run on
hosted CI. The separately approved frozen local runner and its checkout remain
unchanged. Its execution-bootstrap blocker still applies to that exact run.
