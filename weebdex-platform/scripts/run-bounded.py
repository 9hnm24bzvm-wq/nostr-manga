#!/usr/bin/env python3
"""Run one owned subprocess with a deadline and durable lifecycle evidence."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

parser = argparse.ArgumentParser()
parser.add_argument("--cwd", required=True)
parser.add_argument("--deadline", type=float, default=20)
parser.add_argument("--receipt", required=True)
parser.add_argument("--log", required=True)
parser.add_argument("command", nargs=argparse.REMAINDER)
args = parser.parse_args()
command = args.command[1:] if args.command[:1] == ["--"] else args.command
if not command:
    parser.error("a command is required")
receipt_path = Path(args.receipt).resolve()
log_path = Path(args.log).resolve()
receipt_path.parent.mkdir(parents=True, exist_ok=True)
log_path.parent.mkdir(parents=True, exist_ok=True)

def live_group_members(group):
    members = []
    for entry in Path("/proc").glob("[0-9]*"):
        try:
            fields = (entry / "stat").read_text().rsplit(")", 1)[1].split()
            if int(fields[2]) == group and fields[0] != "Z":
                members.append(int(entry.name))
        except (OSError, ValueError, IndexError):
            pass
    return sorted(members)

started = time.monotonic()
timed_out = False
signals = []
with log_path.open("w") as log:
    proc = subprocess.Popen(command, cwd=args.cwd, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    group = proc.pid
    try:
        proc.wait(timeout=args.deadline)
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(group, signal.SIGTERM)
        signals.append("SIGTERM to newly owned process group")
        try:
            proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            os.killpg(group, signal.SIGKILL)
            signals.append("SIGKILL to newly owned process group")
            proc.wait(timeout=2)
    survivors = live_group_members(group)
    if survivors:
        os.killpg(group, signal.SIGKILL)
        signals.append("SIGKILL to remaining newly owned group members")
        time.sleep(0.2)
        survivors = live_group_members(group)

receipt = {
    "command": command,
    "cwd": str(Path(args.cwd).resolve()),
    "deadline_seconds": args.deadline,
    "elapsed_seconds": round(time.monotonic() - started, 3),
    "process_exit_code": proc.returncode,
    "deadline_exceeded": timed_out,
    "terminal_receipt": True,
    "owned_process_group": group,
    "cleanup_signals": signals,
    "surviving_owned_process_ids": survivors,
    "cleanup_verified": not survivors,
    "prior_or_unrelated_processes_touched": False,
}
receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
print(json.dumps(receipt))
sys.exit(125 if survivors else 124 if timed_out else proc.returncode)
