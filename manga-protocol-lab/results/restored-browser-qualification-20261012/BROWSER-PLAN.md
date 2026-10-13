# Fresh native browser qualification plan

Prepared2026-10-12 after the restored4c499 runtime and service smoke. This plan and
new orchestration are not executed or qualified. All process/PG/browser launches are
held because a separate npm installation lost its resumable session and has no terminal
cleanup receipt. Expiration of its planned deadline is not proof that it stopped.

## Current inspection and preserved scope

Supported CUA inventory at21:23UTC shows the dot cloud Chromium connected with one
fresh chrome://newtab tab. No old file chooser is present or resumed. This proves tool
connectivity to that browser, not reachability of an executor-local HTTP listener.
No user's computer, saved coding environment or Work/Codex task is used.

Accepted executable: build-artifacts/linux-x86_64/manga-node,
SHA4c499f724af4db1843f54c51e47746114f777de15b446979a3e8e8a0e7a3e23a.
Browser public assets remain build e4f4862bd75c26bb. Restored official PG18.6/strfry
and Python runtime are usable without a Rust or browser asset rebuild.

The historical four-record native report and exports are restored under
results/rebuild-browser-publication. Its fixture-controller.py,public-operator.py and
public-seam-driver.py were explicitly excluded from that checkpoint. There is no
restored30k/500k native launcher. New source here must be reviewed and tested as new.
The old browser-client/tests/local-browser.cjs launches headless Playwright and uses
private/model adapters; it is not the supported native UI route for this task.

Reusable existing seams: runtime/run-demo.sh for real service setup; the now-tested
same-launch ancestor/pidfd/subreaper guard in ../restored-service-smoke-20261012;
src.postgres_migration.Client for pinned signed HTTP proofs; src.cbor_profile.decode
for an independent bounded CBOR decoder; the documented backup v2 format and rendered
production controls. The synthetic signer protocol follows the restored fixture
semantics, while private key material is newly generated only in the private runtime.

## Serial stages, after the execution hold is resolved

1. Review fresh source and run controlled CLI/process/HTTP cases for controller
   readiness, strict signature/route/body limits, normal stop, startup failure and
   deadline cleanup. Keep public-status output distinct from private configuration.
2. Start a tiny bounded loopback-only probe, display its unique nonce in cloud Chromium,
   and record the actual launch context. Prior historical evidence found executor-local
   listeners inaccessible from this browser and used the supported native Terminal.
   Do not assume either current route works. If a supported native Terminal launch is
   needed, use the same frozen command and verified project/interpreter paths; do not
   change network settings, tunnel or evade an access denial. No large fixture yet.
3. Run a fresh four-record case: one chapter,series,list,membership. Observe default
   local-only behavior, real file import, local save/reload and actual browser export.
   Use the clearly labelled synthetic NIP07 interface, verify/remember the exact local
   service identity, explicitly consent to public publication, and wait for its actual
   accepted event. Independently verify the signed public head and decode all immutable
   root/chunk objects through public HTTP. Then make a local-only edit, confirm that
   neither reload nor local save changes the public head, preview recovery, confirm
   replacement, reload and export. Verify every record field against the frozen input.
4. Stop and review the small-case evidence before any large import.
5. Sequential large cases use separate fresh browser origins and private runtimes:
   30000 chapters+100 series+2 lists+100 memberships=30202 total; then
   500000 chapters+100 series+2 lists+100 memberships=500202 total. Rotate the allowed
   series statuses and use two lists with50 distinct memberships each. Freeze exact
   input bytes, distribution and canonical per-family hashes before importing. The
   four-family coverage is explicitly quantified rather than described as equal stress.
6. Repeat the same actual import/local/reload/export/publication/public-decode/recovery
   checks for each size. Every export is streamed and checked for field equality,
   exact counts and duplicates. Envelope export time/revision may differ; record data
   must not. Record observed phase durations without a throughput/SLO claim.

No direct IndexedDB insertion, private client-state oracle or JS shortcut replaces the
rendered import/publication/recovery flow. Browser interactions use supported CUA only.
Independent public reads and file verification use the fixed browser_evidence.py CLI
outside the browser UI, in the same reachable host context where necessary.

## Limits and stop rules

- Private runtime under .runtime-final; no credentials,private keys,DB or key-bearing
  configuration is included in source/evidence backups. HTTP exposes no generic command
  or admin operation. Sign requests are restricted to fixed fixture snapshot/auth kinds,
  exact origins/routes and bounded shapes; only signatures/public keys leave the signer.
- New client origin and all server ports remain127.0.0.1. Exact CORS origin is fixture
  configuration. No public exposure, extension installation or browser security change.
- The initial8GiB/4GiB disk proposal is superseded by the controller's more conservative
 24GiB sampled preflight and stop floor; require≥2GiB available memory at launch, with
 a1GiB sampled memory stop floor. Current free was30.94GB after smoke. Resource limits are
  safeguards, not an exclusive-memory guarantee. No concurrent load/build/compression.
- Source/staging budget must exceed16MiB for the500k case; propose1GiB physical and
 1GiB staging with existing1,000,000-byte per-chunk limits. Do not alter browser/server
  protocol limits to pass. The existing graph Worker120000ms deadline, signer/request
  timeouts,2M-record/256MiB graph limits remain unchanged.
- Propose a45-minute enforced owner lifetime per case and bounded joined cleanup.
  Each synchronous HTTP dispatch also has an absolute5-second interrupt timer,
  capped by the remaining controller lifetime; the2-second socket inactivity timeout
  is not treated as a total deadline. Expiry is failure with preserved cleanup evidence.
  Any approval barrier stops dependent actions and preserves the exact state/evidence;
  it does not authorize silent deadline extension. Close only newly owned tabs through
  supported UI; never kill an unrelated browser process or use guessed host PIDs.
- Synthetic JSONL inputs/exports are expected below100MiB each; enforce a96MiB file
  limit for this declared dataset,≤64KiB lines and a finite retained-output allowance.
  Verify input size before UI selection. Fixed public read responses use the existing
 20-second/2MiB Client cap and single-object batches; full verifier additionally needs a
  finite command deadline. Final byte counts/caps are frozen with the actual controller.

Success requires actual UI checkpoints, complete public proof/record checks, no hidden
errors, unchanged accepted assets/binary, and a terminal same-launch cleanup receipt.
An unverified process lifecycle or missing export/confirmation is incomplete acceptance.

## Permission boundaries and remaining gaps

Root's scoped GO covers the fresh local synthetic test preparation. The current process
hold must be cleared before execution. A fresh known synthetic backup is selected only
through the supported file control; the old pending chooser action is not restarted.
Any actual policy denial or unexpected browser permission prompt is reported with its
exact action/target. No alternate route is used to defeat a denial.

This qualifies the reference client on the dot cloud browser with a synthetic signer.
It does not qualify real extension prompts, real accounts, mobile/eviction/disk-full,
physical power loss, a full reader website, WAN behavior or target mixed-load capacity.
