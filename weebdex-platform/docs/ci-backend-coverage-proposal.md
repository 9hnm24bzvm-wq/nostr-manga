# Bounded backend release-gate proposal

## Current coverage boundary

The existing `hosted-browser-e2e.yml` only starts on its two exact reviewed
CI branches or a permitted manual dispatch on those branches. It restricts the
job to the named public repository, a standard Ubuntu runner, read-only GitHub
permissions and bounded timeouts. It prepares locked dependencies, runs the
compact-feed PostgreSQL parity fixture and executes the selected genuine-browser
batches. The finite deployment proxy route/header contract now runs before that
preparation, with a two-minute ceiling and no dependencies or service startup.

That workflow does **not** automatically qualify all backend integration targets.
Local committed receipts for 139 units, the native-status case, seven API seams,
113 route probes and the separate 19-case publication/editorial matrix remain
exact-source evidence, not proof that every future change passes them in CI.
Later features add their own cases and must be inventoried from the frozen tree.
An old successful executable is never evidence for modified source.

## Proposed additional manual gate (not enabled by this change)

Add one separately reviewed, opt-in backend gate on the same restricted public
repository/branches and runner class. Do not add push triggers, deployment,
private/self-hosted fallback, credentials, writable repository permissions or
automatic paid capacity. The operator must review and authorize the workflow
before it can run. Keep the existing browser gate distinct.

The proposed runner should:

1. Freeze and hash the checkout, migration manifests, exact runtime tools, Cargo
   lockfile and mandatory production adapter/import closure. Prepare the already
   reviewed official PostgreSQL/original projection binaries in the bounded
   existing preparation path. Do not use licensed manga downloads or private
   keys; create only isolated synthetic identities/data.
2. Build all named targets once with `--locked`, one job, no incremental output
   and the checked-in debug settings. Pin every resulting executable. A maximum
   90-minute preparation timeout and finite disk/memory guard must fail closed;
   no repeated blind full rebuilds or automatic disk deletion.
3. Run the unit executable and explicit, serial ignored HTTP cases with the
   existing `scripts/run-auth-tests.sh`/`auth_fixture_guard.py` supervisor. Use a
   fresh reserved synthetic PostgreSQL fixture for each case, numeric loopback
   listeners only, normal physical shutdown verification and retained failure
   receipts. Set a finite suite timeout (proposed 30 minutes after compilation)
   and per-case deadline (proposed 180 seconds; capacity cases need their measured
   larger fixture budget, while each HTTP request keeps its existing 60-second
   transport and 5-second SQL ceilings).
4. Preserve at least a 4 GiB live disk floor plus 128 MiB cleanup reserve and
   require 256 MiB start headroom. A reserve stop is inconclusive, never a skip
   or pass. Retained stopped databases cannot be deleted without the applicable
   explicit approval; ephemeral runner destruction is a separate platform policy.
5. Run the actual current production relation adapter for the complete 4,096-edge
   crawl, not a duplicate helper or old sibling checkout. Record its exact hash,
   page/byte/time totals, owner-version/snapshot consistency and no omissions.
6. Emit a closed-schema allowlisted result containing source/tool/executable
   hashes, case results, peak resources and physical cleanup receipts. Extend the
   existing sanitizer explicitly before artifact publication. Never glob raw
   fixture trees, databases, signing keys, connection strings or HTTP bodies into
   public artifacts. A failed or missing cleanup receipt fails the gate.

The already qualified matrix inventory is in
`test-evidence/integration/final-backend-matrix-20261018/plan.json` (19 named cases,
11 Rust targets). It covers seven generated-image native upload journeys, three
signed upload/replacement seams, original bridge lifecycle, chapter/title writes,
private/public relation capacity, editorial drafts, group claim, reports and
genuine signed-library status. Keep the genuine signed-library 200 case: a
native-only unavailable-original 503 baseline does not replace it. Combine this
inventory with current API/status/popularity/blocklist fixtures when authoring
the final workflow; do not silently claim that a fixed historical count is the
whole current system.

## Finite operator-script seam

The private checkpoint script has a separate finite suite:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s weebdex-platform/tests/ci -p test_private_checkpoint.py -v
```

It uses only tiny synthetic filesystem data, a fake PostgreSQL command boundary
and the already-installed official libpq parser (no connection). Register it in
the additional gate after verifying/pinning the libpq client prerequisite.
It cannot replace a real isolated backup/restore drill, official `nginx -t`, TLS,
WebSocket/range checks or deployment acceptance. None of those operations or a
remote CI trigger was performed by this change.
