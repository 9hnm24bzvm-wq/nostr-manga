"""Owned short browser temporary directory; standard library only.

BrowserTemp/constants below are copied byte-for-byte from reviewed preflight
source c1a5793883c4ee523884f18010c29b6a89842c9e. Importing creates no resources.
"""
import os
from pathlib import Path
import secrets
import stat
import time


class Refused(RuntimeError):
    """Fixed refusal; no private path is needed in a public error category."""


# Reserve 64 bytes for Chromium's ProcessSingleton directory/socket suffix
# beneath a UTF-8 base of at most 32, below Linux sun_path[108] including NUL.
MAX_BROWSER_TMP_BYTES = 32
BROWSER_SOCKET_SUFFIX_BYTES = 64
LINUX_UNIX_PATH_BYTES = 108
MAX_BROWSER_TMP_ENTRIES = 4096
MAX_BROWSER_TMP_DEPTH = 32


class BrowserTemp:
    """One fresh 0700 /tmp leaf, held by identity; never traverse a symlink."""
    def __init__(self):
        self.parent = Path('/tmp')
        self.parent_fd = self.fd = None
        self.path = self.identity = self.parent_identity = None
        self.created = self.admitted = self.removed = False
        self.entries = self.bytes = 0

    @staticmethod
    def identity_of(value):
        return (value.st_dev, value.st_ino, value.st_uid)

    def _parent_check(self):
        value = os.fstat(self.parent_fd)
        named = os.stat(self.parent, follow_symlinks=False)
        if (not stat.S_ISDIR(value.st_mode) or not stat.S_ISDIR(named.st_mode) or
                self.identity_of(value) != self.parent_identity or
                self.identity_of(named) != self.parent_identity or
                value.st_uid not in (0, os.geteuid()) or
                (value.st_mode & 0o022 and not
                 (value.st_uid == 0 and value.st_mode & stat.S_ISVTX))):
            raise Refused('browser-temp-parent-refused')

    def create(self):
        if self.parent_fd is not None or self.created:
            raise Refused('browser-temp-already-created')
        self.parent_fd = os.open(self.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        self.parent_identity = self.identity_of(os.fstat(self.parent_fd))
        self._parent_check()
        for _ in range(8):
            token = secrets.token_hex(6)
            if len(token) != 12 or any(c not in '0123456789abcdef' for c in token):
                raise Refused('browser-temp-name-refused')
            path = self.parent / ('sbp-' + token)
            size = len(str(path).encode('utf-8'))
            if size > MAX_BROWSER_TMP_BYTES or size + BROWSER_SOCKET_SUFFIX_BYTES >= LINUX_UNIX_PATH_BYTES:
                raise Refused('browser-temp-path-too-long')
            try:
                os.mkdir(path.name, mode=0o700, dir_fd=self.parent_fd)
            except FileExistsError:
                continue  # Never admit, reuse or delete a pre-existing name.
            self.path, self.created = path, True
            self.fd = os.open(path.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                              dir_fd=self.parent_fd)
            self.identity = self.identity_of(os.fstat(self.fd))
            self.check()
            self.admitted = True
            return self.path
        raise Refused('browser-temp-collision-limit')

    def check(self):
        self._parent_check()
        held = os.fstat(self.fd)
        named = os.stat(self.path.name, dir_fd=self.parent_fd, follow_symlinks=False)
        if (not stat.S_ISDIR(held.st_mode) or not stat.S_ISDIR(named.st_mode) or
                self.identity_of(held) != self.identity or
                self.identity_of(named) != self.identity or
                held.st_uid != os.geteuid() or
                stat.S_IMODE(held.st_mode) != 0o700 or stat.S_IMODE(named.st_mode) != 0o700):
            raise Refused('browser-temp-identity-refused')

    def _check_chain(self, chain, deadline):
        if time.monotonic() >= deadline:
            raise Refused('browser-temp-cleanup-deadline')
        self.check()
        for parent, name, identity in chain:
            value = os.stat(name, dir_fd=parent, follow_symlinks=False)
            if not stat.S_ISDIR(value.st_mode) or self.identity_of(value) != identity:
                raise Refused('browser-temp-descendant-changed')

    def _clear(self, fd, chain, depth, deadline):
        self._check_chain(chain, deadline)
        if depth > MAX_BROWSER_TMP_DEPTH:
            raise Refused('browser-temp-depth-limit')
        with os.scandir(fd) as items:
            for item in items:
                self._check_chain(chain, deadline)
                self.entries += 1
                if self.entries > MAX_BROWSER_TMP_ENTRIES:
                    raise Refused('browser-temp-entry-limit')
                value = os.stat(item.name, dir_fd=fd, follow_symlinks=False)
                identity = self.identity_of(value)
                if value.st_uid != os.geteuid() or value.st_dev != self.identity[0]:
                    raise Refused('browser-temp-descendant-refused')
                self.bytes += max(value.st_size, value.st_blocks * 512)
                if stat.S_ISDIR(value.st_mode):
                    child = os.open(item.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
                    try:
                        if self.identity_of(os.fstat(child)) != identity:
                            raise Refused('browser-temp-descendant-changed')
                        self._clear(child, chain + [(fd, item.name, identity)], depth + 1, deadline)
                    finally:
                        os.close(child)
                    self._check_chain(chain + [(fd, item.name, identity)], deadline)
                    os.rmdir(item.name, dir_fd=fd)
                else:
                    # Unlink the entry itself, including Chromium socket symlinks.
                    # A symlink's target is never opened, followed or deleted.
                    self._check_chain(chain, deadline)
                    if self.identity_of(os.stat(item.name, dir_fd=fd, follow_symlinks=False)) != identity:
                        raise Refused('browser-temp-descendant-changed')
                    os.unlink(item.name, dir_fd=fd)

    def cleanup(self, deadline):
        if not self.created:
            return
        if not self.admitted:
            raise Refused('browser-temp-admission-unverified')
        self._clear(self.fd, [], 0, deadline)
        self._check_chain([], deadline)
        os.rmdir(self.path.name, dir_fd=self.parent_fd)
        if os.fstat(self.fd).st_nlink != 0:
            raise Refused('browser-temp-removal-unverified')
        try:
            os.stat(self.path.name, dir_fd=self.parent_fd, follow_symlinks=False)
        except FileNotFoundError:
            self.removed = True
        else:
            raise Refused('browser-temp-removal-unverified')

    def journal(self):
        return dict(created=self.created, admitted=self.admitted, removed=self.removed,
                    identity=self.identity, entries=self.entries, bytes=self.bytes)

    def close(self):
        for name in ('fd', 'parent_fd'):
            fd = getattr(self, name)
            if fd is not None:
                os.close(fd)
                setattr(self, name, None)
