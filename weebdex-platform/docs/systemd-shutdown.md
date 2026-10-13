# Installed systemd shutdown ownership

Issue: [#24](https://github.com/9hnm24bzvm-wq/nostr-manga/issues/24).

The platform's main process owns shutdown: it withdraws readiness and waits for
admitted requests and the embedded native listener/owned work before reporting
successful drain. `run-platform.sh` uses `exec`, preserving that main-process
identity. The projection command's Unix process group does not remove it from
its service cgroup.

The example unit now explicitly uses `KillMode=mixed`. systemd sends the initial
SIGTERM to the main process and retains final whole-cgroup SIGKILL cleanup when
the main process exits or `TimeoutStopSec` expires. The prior implicit
`control-group` policy could signal timeout/flock/strfry children immediately,
interrupting work the application was still draining. Do not disable final kill
cleanup or substitute `process`/`none`. See the
[systemd kill contract](https://github.com/systemd/systemd/blob/main/man/systemd.kill.xml).

`TimeoutStopSec=225` exceeds the default application `shutdown_grace_seconds=210`.
Keep this relationship for the **actual** runtime configuration, including every
operator override. Reserve additional time for exit cleanup and scheduling. A
longer configured application grace requires a longer manager stop budget.

## Source/config check

From `weebdex-platform`, run:

```sh
python3 -m unittest discover -s tests/contracts -v
python3 scripts/check-service-shutdown.py --unit deployment/weebdex.service.example --config /etc/weebdex/platform.json
```

The check reads only the deadline field from the runtime configuration for its
report; it prints no JSON configuration or secrets. It is a deliberately narrow
unit-file/config contract check, not a complete systemd parser. Check the effective
installed properties and drop-ins separately with `systemctl show weebdex.service
--property=MainPID,KillMode,KillSignal,SendSIGKILL,FinalKillSignal,TimeoutStopUSec`
and confirm ExecStart still execs the platform binary. No configuration check
establishes a real manager-stop pass.

## Opt-in disposable real-manager seam

On an authorized disposable Linux host with cgroup v2, GNU timeout/flock and an
already usable systemd user manager:

```sh
python3 scripts/test-systemd-shutdown.py --manager user --output-dir /absolute/new-private-evidence-directory
```

The harness creates uniquely named transient fixture units only. Its non-root
synthetic owner holds an active timeout/flock command and fsync-backed pending
mutation marker. It checks real effective manager properties and cgroup members,
then calls `systemctl stop` for that exact unit. Three cases must pass:

- Old `control-group` negative control interrupts the child, reports failure and
  never records mutation completion.
- `mixed` lets the owner withdraw synthetic readiness, observe child completion,
  finish its synthetic mutation and leave an empty cgroup after successful stop.
- A deliberately stubborn synthetic owner records an unsuccessful application
  deadline at 1 s; the manager's 3 s stop deadline must report timeout and kill all
  remaining cgroup members. The pending marker remains unconfirmed.

Evidence includes effective policy, exact cgroup membership before/after stop,
manager exit/logs and durable synthetic markers. The harness's 30 s runtime cap
also bounds units if the harness is interrupted; an interrupted run is not a
verified cleanup receipt. It never installs/enables services, touches a real
application database, changes host systemd/network/security settings, or proves
actual strfry/application transaction correctness. Local fixture tests use direct
signals and are only sanity checks for the harness.

## Hosted execution proposal, not performed

The official [GitHub Ubuntu 24.04 image inventory](https://github.com/actions/runner-images/blob/main/images/ubuntu/Ubuntu2404-Readme.md)
currently includes systemd. A reviewed standard `ubuntu-24.04` VM job could run
this seam against its disposable system manager, after proving PID1/system bus
and cgroup v2 access. A container job is insufficient. The harness supports
`--manager system --fixture-uid <non-root UID> --fixture-gid <GID>` when invoked by
an authorized privileged operator; it runs fixture services under that non-root
identity. Prefer an already usable user manager. Do not enable lingering, relax
Polkit/security, or alter an existing service to make the test work. Workflow
creation/publication/execution still requires separate review and authorization.
No workflow has been added or run by this change.

## Failure and actual application acceptance

An application drain deadline or manager final kill is an unsuccessful,
**unconfirmed** operation boundary. Process absence is not evidence that a
transaction committed, rolled back, or became safe to replay. Preserve pending
upload/cleanup/publication journals and original durable state for the existing
startup reconciliation. Do not remove markers, release storage charges, or replay
an uncertain user mutation merely because the cgroup is empty. `Restart=on-failure`
may restart the instance; recheck readiness and reconciled public outcomes.

Existing `protocol_bridge_http` coverage of native owned mutation drain supports
the application side separately. Actual installed-unit acceptance remains open:
with a real admitted platform mutation and active owned projection child, stop
through the manager, observe `/readyz` withdrawal, real child terminal state and
committed/reconciled public outcome; then verify both listeners, all cgroup
children and storage ownership close. Repeat with a deliberate actual application
deadline failure and verify unsuccessful stop/final cleanup plus restart journal
reconciliation. Synthetic manager evidence and direct application signals cannot
be combined into a claim that this integrated test ran.
