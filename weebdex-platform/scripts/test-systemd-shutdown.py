#!/usr/bin/env python3
"""Opt-in real-manager signal-policy seam with synthetic durable owned work.

Creates only uniquely named transient fixture units. No installed service, database,
network listener, credentials, system setting, or application mutation is used.
"""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time
import uuid

SCRIPT = Path(__file__).resolve()


def durable(path, value):
    temporary = path.with_suffix(path.suffix + ".tmp")
    with temporary.open("w") as file:
        file.write(json.dumps(value)); file.flush(); os.fsync(file.fileno())
    os.replace(temporary, path)
    descriptor = os.open(path.parent, os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def wait_for(predicate, seconds=10):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        if predicate():
            return
        time.sleep(.02)
    raise RuntimeError("fixture observation deadline")


def fixture(role, root, mode):
    if role == "child":
        def interrupted(*_):
            signal.signal(signal.SIGTERM, signal.SIG_IGN)
            durable(root / "child-term.json", {"signal": "SIGTERM"})
            raise SystemExit(74)
        signal.signal(signal.SIGTERM, interrupted)
        durable(root / "pending.json", {"synthetic_mutation": "unconfirmed"})
        durable(root / "child-ready.json", {"pid": os.getpid()})
        wait_for(lambda: (root / "release.json").exists(), 60)
        durable(root / "committed.json", {"synthetic_mutation": "completed"})
        return 0
    stopping = False
    def stop(*_):
        nonlocal stopping
        stopping = True
    signal.signal(signal.SIGTERM, stop)
    child = subprocess.Popen(["timeout", "--kill-after=1", "60", "flock", str(root / "command.lock"),
                              sys.executable, str(SCRIPT), "--fixture", "child", str(root), mode])
    wait_for(lambda: (root / "child-ready.json").exists())
    durable(root / "owner-ready.json", {"pid": os.getpid(), "timeout_pid": child.pid})
    wait_for(lambda: stopping, 30)
    durable(root / "readiness-withdrawn.json", {"ready": False})
    if mode == "forced":
        time.sleep(1)
        durable(root / "application-deadline.json", {"outcome": "unconfirmed", "success": False})
        # Deliberately stubborn owner tests the manager's final kill deadline.
        time.sleep(60)
        return 75
    time.sleep(.25)  # Give a control-group negative control time to signal its child.
    durable(root / "release.json", True)
    result = child.wait(timeout=5)
    if result or (root / "child-term.json").exists():
        durable(root / "drain-failed.json", {"child_exit": result})
        return 74
    durable(root / "drained.json", {"child_exit": 0})
    return 0


def command(args, binary, *parts, check=True):
    return subprocess.run([binary, "--" + args.manager, *parts], capture_output=True,
                          text=True, timeout=15, check=check, env={**os.environ, "LC_ALL": "C"})


def properties(args, unit):
    result = command(args, "systemctl", "show", unit, "--property=MainPID,ControlGroup,KillMode,KillSignal,SendSIGKILL,FinalKillSignal,TimeoutStopUSec")
    return dict(line.split("=", 1) for line in result.stdout.splitlines() if "=" in line)


def members(group, unit):
    if not group.startswith("/") or ".." in Path(group).parts or Path(group).name != unit:
        raise RuntimeError("unexpected transient unit cgroup")
    root = Path("/sys/fs/cgroup") / group.lstrip("/")
    return sorted({int(pid) for path in root.rglob("cgroup.procs") for pid in path.read_text().split()})


def case(args, root, name, kill_mode, mode):
    directory = root / name
    directory.mkdir(mode=0o700)
    if args.manager == "system":
        os.chown(directory, args.fixture_uid, args.fixture_gid)
    unit = "wdx-stop-check-" + uuid.uuid4().hex + ".service"
    run = ["systemd-run", "--" + args.manager, "--wait", "--unit=" + unit,
           "--property=Type=simple", "--property=Restart=no", "--property=RuntimeMaxSec=30",
           "--property=KillMode=" + kill_mode, "--property=KillSignal=SIGTERM",
           "--property=SendSIGKILL=yes", "--property=FinalKillSignal=SIGKILL",
           "--property=TimeoutStopSec=3", "--property=UMask=0077"]
    if args.manager == "system":
        run += ["--uid=" + str(args.fixture_uid), "--gid=" + str(args.fixture_gid)]
    run += [sys.executable, str(SCRIPT), "--fixture", "owner", str(directory), mode]
    group = None
    with (directory / "manager.log").open("w") as log:
        launcher = subprocess.Popen(run, stdout=log, stderr=subprocess.STDOUT, env={**os.environ, "LC_ALL": "C"})
        try:
            wait_for(lambda: (directory / "owner-ready.json").exists())
            fields = properties(args, unit)
            group = fields["ControlGroup"]
            initial = members(group, unit)
            owner = json.loads((directory / "owner-ready.json").read_text())
            child = json.loads((directory / "child-ready.json").read_text())["pid"]
            if int(fields["MainPID"]) != owner["pid"] or not {owner["pid"], owner["timeout_pid"], child}.issubset(initial):
                raise RuntimeError("active owner/timeout/child not in exact unit cgroup")
            if fields["KillMode"] != kill_mode or fields["SendSIGKILL"] != "yes" or fields["KillSignal"] != "15" or fields["FinalKillSignal"] != "9" or fields["TimeoutStopUSec"] != "3s":
                raise RuntimeError("effective transient unit policy differs")
            locked = subprocess.run(["flock", "--nonblock", str(directory / "command.lock"), "true"], timeout=3)
            if locked.returncode != 1 or (directory / "committed.json").exists():
                raise RuntimeError("mutation/owned lock not active at stop")
            started = time.monotonic()
            command(args, "systemctl", "stop", unit, check=False)
            code = launcher.wait(timeout=12)
            elapsed = time.monotonic() - started
            remaining = members(group, unit)
            if remaining or not (directory / "readiness-withdrawn.json").exists():
                raise RuntimeError("readiness or final cgroup cleanup unconfirmed")
            if mode == "forced":
                log.flush()
                if code == 0 or elapsed < 2.5 or "result: timeout" not in (directory / "manager.log").read_text() or not (directory / "application-deadline.json").exists() or (directory / "committed.json").exists():
                    raise RuntimeError("manager deadline did not establish unsuccessful final cleanup")
            elif kill_mode == "control-group":
                if code == 0 or not (directory / "child-term.json").exists() or (directory / "committed.json").exists():
                    raise RuntimeError("negative control did not reproduce premature child interruption")
            elif code != 0 or not (directory / "drained.json").exists() or not (directory / "committed.json").exists() or (directory / "child-term.json").exists():
                raise RuntimeError("mixed graceful owned-work drain failed")
            return {"case": name, "unit": unit, "policy": fields, "initial_cgroup_pids": initial,
                    "manager_exit": code, "stop_seconds": elapsed, "remaining_cgroup_pids": remaining,
                    "synthetic_work_completed": (directory / "committed.json").exists(), "status": "passed"}
        finally:
            # Only this exact newly created transient unit may be stopped/killed/reset.
            command(args, "systemctl", "kill", "--kill-whom=all", "--signal=SIGKILL", unit, check=False)
            command(args, "systemctl", "stop", unit, check=False)
            command(args, "systemctl", "reset-failed", unit, check=False)
            try:
                launcher.wait(timeout=5)
            except subprocess.TimeoutExpired:
                launcher.kill(); launcher.wait(timeout=5)
            if group and members(group, unit):
                raise RuntimeError("fixture cleanup left cgroup members")


def validate_identity(args):
    if args.fixture_uid == 0:
        raise RuntimeError("fixture processes must run as a non-root user")
    if args.manager == "user" and (os.geteuid() == 0 or args.fixture_uid != os.geteuid() or args.fixture_gid != os.getegid()):
        raise RuntimeError("user-manager fixtures must use the actual non-root caller identity")


def main():
    if len(sys.argv) == 5 and sys.argv[1] == "--fixture":
        return fixture(sys.argv[2], Path(sys.argv[3]), sys.argv[4])
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manager", choices=("user", "system"), default="user")
    parser.add_argument("--output-dir", type=Path, required=True, help="new private evidence directory")
    parser.add_argument("--fixture-uid", type=int, default=os.getuid())
    parser.add_argument("--fixture-gid", type=int, default=os.getgid())
    args = parser.parse_args()
    args.output_dir = args.output_dir.resolve()
    args.output_dir.mkdir(mode=0o700, parents=False)  # Never reuse an existing fixture.
    report = {"boundary": "real-manager policy seam, synthetic work; no actual application acceptance", "cases": []}
    try:
        validate_identity(args)
        if args.manager == "system":
            os.chown(args.output_dir, args.fixture_uid, args.fixture_gid)
        command(args, "systemctl", "show", "--property=Version")
        for name, kill_mode, mode in (("old-policy-negative-control", "control-group", "graceful"),
                                     ("mixed-graceful", "mixed", "graceful"), ("mixed-deadline", "mixed", "forced")):
            report["cases"].append(case(args, args.output_dir, name, kill_mode, mode))
        report["status"] = "passed"
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        report.update(status="failed_or_blocked", error=str(error))
    durable(args.output_dir / "receipt.json", report)
    print(json.dumps(report, sort_keys=True))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
