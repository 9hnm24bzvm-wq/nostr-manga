# Combined browser fixture handoff

Source-only preparation. The revised Rust example compiled successfully; runtime
execution remains held until parent audit/UI clearance. No fixture, PostgreSQL,
original backend, listener or browser was launched by this revision's owner.

## Entrypoint and authority

`tests/runtime/browser_fixture.py` accepts `--binary`, `--expected-sha256`,
`--ui-dir`, `--frontend-index-sha256`, `--protocol-provisioner`,
`--provisioner-sha256`, `--frontend-artifact-manifest` (defaults to
`test-evidence/frontend/artifact-manifest.json`), mandatory
`--frontend-artifact-manifest-sha256`, optional `--second-protocol-recipient`, and
bounded `--lifetime-seconds` (600–3600, default2700). Complete-manifest and index
hashes are required for every fixture; combined mode additionally requires both
executable hashes. `--ubunchu-noncommercial` is an explicit optional licensed-input
choice; otherwise PNG are synthetic. Read `ubunchu-fixture.md` first.

Optional `--ui-regression-controls` enables only the named native regression
queue below. It requires the reviewed fixture provisioner and does not seed
bulk data at startup. Leave it off for the original minimal fixture.

The supervisor copies the reviewed public frontend into an immutable private
snapshot. Before copying, and again after copying, it requires exact file and
directory membership, every manifest byte length and SHA256, no links/special
files and bounded totals. The snapshot is read-only and rechecked before every
listener generation and browser phase. PG, real Rust platform with its embedded original listener, local mail
and browser are children of one owned loopback execution session. The test-only
Rust example verifies the direct private `test-runtime/browser-final-*` root,
fixture marker, actual reserved PG port and exact password-free disposable DSN.
It accepts only `weebdex_browser_test` or, for recipient actions, the distinct
`weebdex_browser_recipient_test`. It rejects arbitrary roots/databases/hosts and
an unknown backend binary. Original `Repository` migration/admission/publication
services establish authority; native fixture SQL creates native rows only.

Signed1002/1001 canonical fixtures are published by real `PublicationRuntime`.
Exact signed event, canonical CBOR and original input paths remain in the private
runtime root for restoration. Its exclusive original volume owner is dropped
before the real platform opens its shared original runtime. Success readiness
requires actual original visibility/projection, genuine importer aliases and
resources/capabilities responses. Pending acceptance is never public success.

## Browser contract

Private `supervisor.json` supplies:
- `platform_origin`, `mail_origin`, `protocol_origin` (exact loopback HTTP)
- `relay_origin` (exact loopback WS root), public `service_key`/`operator_pubkey`
- `protocol_seed.series`/`.chapters`: real64hex `event_id`, imported10character
  `native_id`, `visible`, `projected`, `mapped`; `fully_ready` and namespace
- `protocol_capabilities`, read from the genuine platform endpoint
- Native `ids`, including two same-title/same-number en/ja alternative chapters
  and a genuine600×3600 first page; synthetic canonical seed also has a tall page
- `protocol_recipients`: the primary `{origin,relay_origin,service_key}` and the
  actual separate recipient when explicitly enabled
- `fixture_control`: private command/result paths and enabled action names
- `active_browser_phase` and `browser_phase_deadline_unix` during an owned phase;
  use this phase label for private controls, rather than guessing from run IDs

Local sink POST `/fixture/protocol/account` accepts exactly
`{"pubkey":"<synthetic64hex>","allowed":true|false}` and calls actual original
account admission. Optional `/fixture/protocol/recipient-account` has the same
schema for the second original service. At most16 distinct public synthetic
accounts are accepted. Neither receives a private signer. Native password
registration, verification, recovery and role fixtures remain separate flows.

The optional second service has a distinct original database, storage owner,
service/operator keys, backend directory and listener. It uses the original
injected-store listener seam with owned worker/operation drain. Exact mutual
`Repository::set_peer` approvals are established only between the two recorded
services. Real browser account admission is still an explicit separate step.
No fabricated relay or delivery acknowledgment is introduced.

Original read-policy is public; CORS permits only the UI origin. CSP contains only
reviewed exact HTTP/WS service origins. Browser network routing/deny proxy must
validate the declared recipients and reject all other destinations. The browser
performs genuine signature/service-proof validation; supervisor recipient startup
checks the actual proof endpoint's identity, nonce and endpoint, not its signature.
NIP07 test signer secrets remain exclusively runner memory.

## Private control queue

The Node runner atomically writes `fixture_control.command_file`, mode private
through the containing0700 root. No moderation/rebuild HTTP endpoint exists.
One command at a time; JSON is bounded4096bytes. Schema:

```json
{
  "request_id": "unique_safe_ascii_1_to_48",
  "action": "source-revoke",
  "event_id": "one recorded canonical seed64hex",
  "browser_phase": "actual supervisor phase label"
}
```

Allowed actions:
1. `source-revoke`: actual owner-signed kind5, original deletion request and
   operator approval. Completion requires terminal original removal/no pending
   projection, original public404 and absence of the actual native resource map.
2. `source-restore`: additionally `allow_listener_restart:true`. First signal and
   join both owned services, require exit0/no surviving process-group members and
   closed listeners, then acquire the original exclusive owner. Invoke actual
   `restore_publication` and publish the recorded exact CBOR/images with original
   `created_at`. Require the identical signed event ID, release ownership, restart
   and observe genuine public projection/import/mapping. An exact pending restore
   may retry the same publication; elapsed time never implies success.
3. `catalogue-rebuild`: additionally `allow_listener_restart:true`. Join all
   listeners, drop only the disposable platform schema and apply actual platform
   migrations. Preserve original schema/service/storage/keys and reseed native
   fixture rows. Restart and verify the genuinely migration-generated different
   namespace and matching real capabilities/resources. Native password users and
   sessions deliberately disappear with this disposable schema. No UUID UPDATE.
4. `runtime-restart`: additionally `allow_listener_restart:true`. Joined restart
   using the same identities, origins, UI snapshot and runtime stores.
5. Optional `peer-policy`: `allowed:true|false`, actual mutual approval/revocation
   of only the two recorded exact recipients. No arbitrary endpoint field.

While a browser is active, every command must acknowledge its exact supervisor
`browser_phase`. Restore/rebuild/restart require at least430seconds remaining in
both the phase and fixture observation window. Such commands should run early
in a signed phase queued with600seconds; the exact eight allowed signed phases
are `signed-library`, `signed-profile`, `signed-social`, `signed-context`,
`signed-source-controls`, `signed-namespace`, `signed-recipients`, `signed-restart`.
Ordinary runner phases retain300seconds
maximum (initial startup remains150seconds). Drain is bounded220seconds, the
provisioner action90seconds and combined restart readiness90seconds. Failure
writes a false receipt and triggers owned cleanup, never a success-shaped reply.

Accepted requests are archived privately under `control-commands`; the runner
polls `control-results/<request_id>.json`, requires `success:true`, then reloads
`supervisor.json`. Results report real public IDs/state/namespace and joined
listener receipts, never signer material or signed HTTP authorization bodies.
A rejected malformed/unsafe command is moved to the private rejected file and
reported; it cannot perform a control. Use one source revocation at a time and
restore it before another revocation/restart/rebuild. During intentional removal,
`protocol_seed.fully_ready` is truthfully false and supervisor status explicitly
records the revoked fixture source. Browser tests must retain old saved device
records across rebuild and test the actual namespace/source fences.

## Optional named native UI regression queue

With `--ui-regression-controls`, the same private queue additionally accepts:
- `ui-regressions-seed`, exactly `scenario:"pagination"`. Calls the guarded Rust
  writer against the existing native baseline. Establishes505 es compact chapters,
  101 fr terminal chapters,101 isolated native entity uploads and10,000 metadata-
  only title rows. Real original media is reused; no synthetic protocol IDs or
  popularity/activity counters are introduced.
- `ui-regressions-readers`, no scenario/target fields. Invoke only after compact-es
  pagination checks. Adds genuine en/es alternatives and4-portrait/8-mixed readers
  with at most1MiB of generated900×600 landscape original; compact-es becomes506.
- `ui-regressions-status`, no scenario/target fields. Queries actual current totals
  and reads the recorded native descriptor.
- `ui-regressions-withdraw`, exactly `scenario:"terminal"` or `"entity"`. The Rust
  provisioner resolves only its recorded hardcoded target, verifies current
  persisted staff/native ownership, and calls production catalogue deletion.
  This genuine101→100 instance soft-hide is separate from signed-source removal.

Only `request_id`, `action`, `browser_phase` and the stated `scenario` are accepted.
UI origin always comes from the supervisor. IDs, SQL, credentials and arbitrary
paths cannot be supplied. Seed/readers actions have120second deadlines; status/
withdraw have30seconds. An active ordinary<=300second browser phase and the
fixture lifetime must each have the action deadline plus20seconds remaining.
Successful controls publish the genuine descriptor as `supervisor.ui_regressions`
and `control-results/<request_id>.json.ui_regressions`. Require success and reload
the supervisor receipt; seeded metadata is not evidence a UI assertion passed.
The descriptor contains compact/terminal/entity/search/preference sections and,
after explicit activation, bilingual/readers sections. Original signing authority
and all reviewed ownership/cleanup/manifest behavior are unchanged.

A disposable namespace rebuild clears this native metadata and its exact private
cached descriptor files; a fresh named seed is required afterward. No new genuine
home-grid503 fault control is part of the four-control extension's reviewed scope.
The separate optional seam below requires its own source review. All regression
runtime execution remains gated.

## Optional genuine homepage dependency503 control

`--pg-home-fault-controls` independently enables exactly `pg-home-fault-begin` and
`pg-home-fault-resume` on the existing private file queue. Only `request_id`,
`action` and `browser_phase` are accepted. There is no production endpoint, SQL,
PID, command, credential, duration or path argument. The begin command requires
the active owned runner kind `ui14-home-failures`, an intact ready signed seed,
at least80seconds remaining in the phase and fixture, and no prior PG fault.
Its descriptor is `fixture_control.pg_home_fault`, with `enabled:true`,
`outage_seconds:15`, `maximum_faults:1` and `independent_restart_watchdog:true`.

Before stopping the known disposable PG, the supervisor arms a registered child
watchdog. The watchdog verifies the private fixture marker/root, actual original
postmaster PID/start/session identity, parent pidfd, exact original executable
SHA256, recorded launch argv, data directory/port and unchanged config hashes.
The original postmaster is SIGINT-fast-stopped and joined, including its owned
session; no successful outage receipt is possible before it is gone. The watchdog
then starts the exact same fixed binary/cluster/arguments after15seconds or an
earlier resume request. No authentication/network policy or config is modified.
Restart does not require a working database command. Browser failure, supervisor
stop and exact-parent death all take a bounded recovery/cleanup path. Parent death
during the arm/stop race also stops the exactly identified original sibling PG.

The begin result's `pg_home_fault` has `status:"outage"`, `active:true` and
`native503_observed:{path:"/api/chapter/updates?limit=1",status:503,code:"unavailable"}`
only after observing the actual application response. No status is fabricated.
Resume has a60second recovery observation budget. Its `pg_home_fault` has
`status:"ready"`, `active:false`, `fixed_cluster_preserved:true` and
`native_recovery:{readyz_status:200,read_status:200}` only after actual PG/native
readiness, public read, unchanged protocol namespace and real source mappings
recover. Use a60second private-command deadline plus20seconds phase reserve.
Other private controls and fixture mutations are held until explicit recovery.
The fixed15second automatic restart still occurs if the browser never resumes.

`watchdog_terminal:false` is intentional on both command receipts: the watchdog
owns the actual restarted PG in the existing `postgres` lifecycle slot until final
teardown. Cleanup requests recovery before draining native work, then grants the
watchdog30seconds to join actual PG and descendants. Exact parent pidfd death or a
fixed4200second outer bound also ends ownership. Its separate terminal receipt is
`cleanup.json.pg_home_fault_terminal`; success requires `terminal:true`,
`status:"verified_stopped"`, actual PG `returncode:0`, `forced:false`, empty owned
descendants and closed port, plus explicit `error:null` and the registered
watchdog Popen's actual `watchdog_returncode:0`. An unexpected clean PG exit still
fails if its watchdog reported an error/exited nonzero. Intentional supervisor
SIGINT/SIGTERM goes to the watchdog, which requests and joins the actual PG stop;
this expected teardown remains separate from a PG exit observed before that stop.
The watchdog's first cleanup poll classifies an already-terminal replacement PG
as unexpected, retaining an error and exit1 even if cancellation bypassed the
ready loop's poll. Actual PG status, force and descendant evidence remain separate.
Missing/failed/forced terminal evidence makes cleanup
unconfirmed/nonzero, retaining cumulative lifecycle history. Source/AST checks
alone establish none of these runtime assertions. The fault seam has a separate
source index, `test-evidence/security/pg-home-fault-fixture-source.json`.

## Lifecycle and remaining validation

The private `browser-command.json` queue runs only reviewed runner scripts as
owned children in the same execution session. Each phase has a terminal receipt,
owned session/descendant cleanup and deadline notice. The dedicated supervisor
is a Linux child subreaper. It records each leader's PID/start ticks/session,
tracks inherited-session members even with a different PGID, captures descendant
birth identities before signalling leaders, and adopts orphaned descendants.
Every signal uses an individual pidfd after checking the exact current identity;
no process-name matching or unrelated process/group sweeping is used. Adopted
zombies are reaped and surviving historical/detached descendants are included in
terminal cleanup. A surviving owned child or unconfirmed ownership prevents a
clean offline marker or `verified_stopped` result, even when listeners are closed.
All `Popen` births and registrations are serialized with orphan-reaper selection
and `waitpid`; the lock is never held across `communicate` or a blocking drain.
Synchronous psql/initdb/readiness/migration commands use the same registered
owned-child path. Their direct leaders are excluded from orphan `waitpid`, so
the owning Popen retains genuine exit status for every caller and a failed SQL
command cannot become Python's ECHILD fallback zero. The exclusion stays locked
through the final exact-identity check and orphan wait, fencing stale snapshots.
Intentional listener restart is explicitly acknowledged by the active runner.
Complete service/backend descendant sets and ports must terminate before a new
generation or namespace mutation is allowed.
Fixture mutation/mail is withdrawn during rebuild and cleanup. Cleanup stops
all browsers first, serially joins admitted provisioner work, drains both original
services, stops PG, checks owned groups/ports and removes both key pairs.
Per-child `forced`/`ever_forced` is cumulative and never reset by a later cleanup
which finds the child already stopped. Archived generations and the fixture's
`lifecycle_ever_forced` retain earlier forced work. Final receipts distinguish
`final_teardown_clean` (only the final shutdown operation) from `lifecycle_clean`
(no earlier force, fixture error or failed browser phase). These are process
lifecycle statements, not feature-acceptance results. A clean final shutdown
cannot erase a prior failed/forced phase. The supervisor exits nonzero for
unconfirmed/failed cleanup or an unclean lifecycle even if no earlier exception
was raised; consumers must still inspect the detailed terminal receipt.

Sanitized child environments intentionally omit `NODE_PATH`. The browser owner
must load the reviewed installed Playwright package by its explicit absolute
path, `/opt/codex/runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright`.
Do not restore arbitrary dependency paths or live credential variables.

The signed journey module still needs authored calls/assertions for these new
control seams and two-recipient cases. Compiled/source-ready controls do not
establish browser acceptance. Final UI hash, source review clearance, real startup,
projection, genuine delivery/recovery/revocation/rebuild and terminal lifecycle
receipts remain required. Actual >1MiB replacement through the reviewed nginx
binary is a separate pending proxy acceptance check.

Keep all runtime databases/keys, private control data, licensed PNG/derivatives
and artwork-bearing screenshots out of source/checkpoints/evidence uploads.
`test-evidence/security/combined-browser-fixture-source.json` records frozen source
hashes, compiler outcome and the explicit unexecuted runtime scope.
