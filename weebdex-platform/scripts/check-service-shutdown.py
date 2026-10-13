#!/usr/bin/env python3
"""Read-only check of the shutdown contract in one unit file and runtime config.

This intentionally does not resolve systemd drop-ins or prove manager operation.
"""
import argparse
import json
import math
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIELDS = {"KillMode", "KillSignal", "SendSIGKILL", "FinalKillSignal", "TimeoutStopSec", "Type", "ExecStart", "SendSIGHUP", "RestartKillSignal", "ExecStop"}


def seconds(value):
    # Accept the units used by this template and systemd's common time syntax.
    units = {"": 1, "s": 1, "sec": 1, "min": 60, "h": 3600, "ms": .001}
    tokens = re.findall(r"(\d+(?:\.\d+)?)\s*(ms|sec|min|s|h)?", value)
    if not tokens or re.sub(r"\d+(?:\.\d+)?\s*(?:ms|sec|min|s|h)?", "", value).strip():
        raise ValueError("finite TimeoutStopSec required")
    total = sum(float(number) * units[unit] for number, unit in tokens)
    if not math.isfinite(total):
        raise ValueError("finite TimeoutStopSec required")
    return total


def service_fields(text):
    fields, section = {}, None
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith(("#", ";")):
            continue
        if line.startswith("["):
            section = line
        elif section == "[Service]" and "=" in line:
            key, value = line.split("=", 1)
            if key in FIELDS:
                if key in fields:
                    raise ValueError("ambiguous repeated shutdown directive: " + key)
                fields[key] = value.strip()
    return fields


def check(unit_text, config, default_grace):
    if not isinstance(config, dict):
        raise ValueError("runtime config must be an object")
    fields = service_fields(unit_text)
    for key, expected in {"Type": "simple", "KillMode": "mixed", "KillSignal": "SIGTERM"}.items():
        if fields.get(key) != expected:
            raise ValueError(key + " must be " + expected)
    if fields.get("SendSIGHUP", "no") != "no" or fields.get("RestartKillSignal", "SIGTERM") != "SIGTERM" or fields.get("ExecStop", ""):
        raise ValueError("initial stop/restart must remain main-owned SIGTERM without a separate stop hook")
    if fields.get("SendSIGKILL", "yes") != "yes" or fields.get("FinalKillSignal", "SIGKILL") != "SIGKILL":
        raise ValueError("final whole-cgroup SIGKILL cleanup must remain enabled")
    start = fields.get("ExecStart", "").split()
    if len(start) != 2 or not start[0].startswith("/") or not start[0].endswith("/scripts/run-platform.sh") or start[1] != "serve":
        raise ValueError("ExecStart must preserve the exec-based main-process entrypoint")
    grace = config.get("shutdown_grace_seconds", default_grace)
    if type(grace) is not int or not 10 <= grace <= 900:
        raise ValueError("application shutdown_grace_seconds must be an integer in 10..900")
    stop = seconds(fields.get("TimeoutStopSec", ""))
    if stop <= grace:
        raise ValueError("TimeoutStopSec must exceed the configured application shutdown_grace_seconds")
    return {"kill_mode": "mixed", "application_grace_seconds": grace, "manager_stop_seconds": stop,
            "boundary": "unit/config contract only; installed overrides and real manager stop unverified"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--unit", type=Path, default=ROOT / "deployment/weebdex.service.example")
    parser.add_argument("--config", type=Path, required=True, help="actual runtime JSON; never printed")
    args = parser.parse_args()
    try:
        defaults = re.findall(r"shutdown_grace_seconds:\s*(\d+)\s*,", (ROOT / "backend/src/config.rs").read_text())
        if len(defaults) != 1:
            raise ValueError("cannot establish application's source default grace")
        print(json.dumps(check(args.unit.read_text(), json.loads(args.config.read_text()), int(defaults[0])), sort_keys=True))
    except (OSError, json.JSONDecodeError):
        parser.exit(1, "Cannot read unit or runtime JSON; configuration contents are withheld.\n")
    except (ValueError, TypeError) as error:
        parser.exit(1, "Shutdown contract failed: " + str(error) + "\n")


if __name__ == "__main__":
    main()
