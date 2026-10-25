"""Outer process owner using the reviewed fixture's existing pidfd methods.

This constructor does not construct a Fixture, create a root, bind a socket,
start a server, or inspect credentials. Its dedicated caller launches only one
registered supervisor at a time. Adoption therefore stays inside that caller's
own newly created child tree, never another process's siblings or broad pkill.
"""
import ctypes
import importlib.util
import os
from pathlib import Path
import signal
import threading
import time

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('reviewed_browser_fixture_owner', ROOT / 'tests/runtime/browser_fixture.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class OwnedFixtureProcesses(module.Fixture):
    def __init__(self):
        libc = ctypes.CDLL(None, use_errno=True)
        if libc.prctl(36, 1, 0, 0, 0) != 0:
            raise OSError(ctypes.get_errno(), 'outer fixture subreaper unavailable')
        if not hasattr(os, 'pidfd_open') or not hasattr(signal, 'pidfd_send_signal'):
            raise RuntimeError('outer identity-safe signalling unavailable')
        self.children = {}
        self.receipt = {'children': {}}
        self.process_roots = {}
        self.owned_identities = {}
        self.process_actions = []
        self.reaped_descendants = []
        self.process_tracking_errors = []
        self.process_ownership_lock = threading.RLock()
        self.lifecycle_ever_forced = False

    def wait_terminal(self, process, seconds):
        deadline = time.monotonic() + seconds
        while process.poll() is None and time.monotonic() < deadline:
            self.owned_members()
            time.sleep(.1)
        self.reap_owned()
        return process.poll() is not None

    def fallback_cleanup(self):
        # Same reviewed SIGTERM/SIGKILL, /proc birth fences, pidfds and adopted
        # child reaping as the supervisor. No process-name or broad-group kill.
        self.finish_owned_group(None)
        remaining = self.process_members(None)
        return {'forced': True, 'verified_stopped': not remaining and not self.process_tracking_errors, 'remaining_owned_process_count': len(remaining)}
