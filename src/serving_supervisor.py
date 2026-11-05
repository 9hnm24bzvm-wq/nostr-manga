"""Managed loopback nginx reader family. Experimental Linux control-plane helper.

State is persisted before the server child
is permitted to exec, so a killed supervisor cannot leave an unknown reader.
"""

import argparse, asyncio, ctypes, fcntl, hashlib, ipaddress, json, os, re, signal, socket, stat, sys, time, uuid, select
from pathlib import Path


class OwnerProtocol(asyncio.StreamReaderProtocol):
    def __init__(self, reader, stopping):
        super().__init__(reader)
        self.stopping = stopping

    def eof_received(self):
        self.stopping.set()
        return super().eof_received()

    def connection_lost(self, error):
        self.stopping.set()
        super().connection_lost(error)


class ServingError(Exception):
    pass


def process(pid):
    try:
        text = Path(f"/proc/{pid}/stat").read_text()
        parts = text[text.rfind(")") + 2 :].split()
        return dict(
            pid=pid,
            state=parts[0],
            group=int(parts[2]),
            session=int(parts[3]),
            birth=int(parts[19]),
        )
    except (FileNotFoundError, ProcessLookupError):
        return None


def family(sid):
    result = []
    for path in Path("/proc").iterdir():
        if path.name.isdigit():
            p = process(int(path.name))
            if p and p["session"] == sid and p["state"] != "Z":
                result.append(p)
    return result


def same(saved):
    current = process(saved["pid"])
    return bool(
        current and current["birth"] == saved["birth"] and current["state"] != "Z"
    )


def signal_owned(saved, sig):
    # Bind the signal to a kernel process handle, then verify birth. A PID may be
    # reused between a /proc read and kill(pid); pidfds remove that signal race.
    try:
        fd = os.pidfd_open(saved["pid"], 0)
    except ProcessLookupError:
        return
    try:
        if same(saved):
            try:
                signal.pidfd_send_signal(fd, sig)
            except ProcessLookupError:
                pass
    finally:
        os.close(fd)


def save(path, value):
    temp = path.with_name(path.name + "." + uuid.uuid4().hex + ".tmp")
    fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(fd, "w") as f:
            json.dump(value, f, separators=(",", ":"))
            f.flush()
            os.fsync(f.fileno())
        os.replace(temp, path)
        fd = os.open(path.parent, os.O_RDONLY)
        os.fsync(fd)
        os.close(fd)
    finally:
        if temp.exists():
            temp.unlink()


def load(path):
    m = path.lstat()
    if (
        not stat.S_ISREG(m.st_mode)
        or m.st_mode & 0o077
        or m.st_uid != os.geteuid()
        or m.st_size > 16384
    ):
        raise ServingError("unsafe serving state file")
    return json.loads(path.read_text())


def directory(value):
    path = Path(value)
    if not path.is_absolute() or not re.fullmatch(r"[A-Za-z0-9_./-]+", str(path)):
        raise ServingError("absolute safe directory required")
    path.mkdir(parents=True, exist_ok=True, mode=0o700)
    if path.resolve() != path or path.is_symlink():
        raise ServingError("canonical nonsymlink directory required")
    # The owner may expose read-only image paths; other users must not mutate them.
    m = path.stat()
    if m.st_uid != os.geteuid() or m.st_mode & 0o022:
        raise ServingError("directory owner or write permissions unsafe")
    return path


async def until(check, seconds):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = check()
        if value:
            return value
        await asyncio.sleep(0.02)
    raise ServingError("reader process deadline; no drain proof")


def reap():
    while True:
        try:
            pid, _ = os.waitpid(-1, os.WNOHANG)
            if not pid:
                return
        except ChildProcessError:
            return


class Supervisor:
    def __init__(self, args):
        if os.geteuid() == 0:
            raise ServingError("run as a dedicated unprivileged service user")
        if not re.fullmatch("[0-9a-f]{64}", args.owner):
            raise ServingError("owner key required")
        if not 1 <= args.workers <= 16 or not 1 <= args.shutdown_seconds <= 120:
            raise ServingError("worker/shutdown bounds")
        address, port = args.bind.rsplit(":", 1)
        if not ipaddress.ip_address(address).is_loopback or not 1 <= int(port) <= 65535:
            raise ServingError("loopback serving address required")
        self.address = address
        self.port = int(port)
        self.args = args
        self.runtime = directory(args.runtime)
        self.public = directory(args.public)
        if self.runtime != self.public / ".serving":
            raise ServingError("runtime must be the public tree .serving directory")
        self.binary = Path(args.nginx).resolve(strict=True)
        if not self.binary.is_file() or not os.access(self.binary, os.X_OK):
            raise ServingError("nginx executable required")
        self.state_path = self.runtime / "state.json"
        self.socket_path = self.runtime / "control.sock"
        self.lock = os.open(
            self.runtime / "owner.lock", os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600
        )
        try:
            fcntl.flock(self.lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            os.close(self.lock)
            raise ServingError("serving supervisor still owns this volume") from None
        self.context = dict(
            boot=Path("/proc/sys/kernel/random/boot_id").read_text().strip(),
            pid_namespace=os.readlink("/proc/self/ns/pid"),
        )
        self.state = None
        self.commands = asyncio.Lock()
        self.stop_event = asyncio.Event()
        if ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) != 0:
            raise ServingError("subreaper setup unavailable")

    def owned_family(self, state):
        if state.get("owner") != self.args.owner or state.get("binary") != str(
            self.binary
        ):
            raise ServingError("serving owner/binary mismatch")
        if state.get("context") != self.context:
            raise ServingError(
                "serving host/namespace changed; offline review required"
            )
        master = state.get("master")
        if not master:
            return []
        current = process(master["pid"])
        members = family(master["session"])
        if current and current["birth"] != master["birth"]:
            # Never signal a reused PID/session. An ambiguous record requires review.
            if members:
                raise ServingError("serving PID/session reused; manual review required")
            return []
        for child in members:
            if (
                child["session"] != master["pid"]
                or child["group"] != master["pid"]
                or child["birth"] < master["birth"]
            ):
                raise ServingError("reader family identity mismatch")
            try:
                exe = Path(f'/proc/{child["pid"]}/exe').resolve(strict=True)
            except FileNotFoundError:
                continue
            allowed = (
                exe == self.binary
                or state.get("phase") == "starting"
                and child["pid"] == master["pid"]
            )
            if not allowed:
                raise ServingError("unexpected process in owned reader session")
        return members

    async def stop_family(self, state):
        members = self.owned_family(state)
        for child in members:
            signal_owned(child, signal.SIGQUIT)
        try:
            await until(
                lambda: not self.owned_family(state), self.args.shutdown_seconds + 2
            )
        except ServingError:
            # Exact birth identities are rechecked; only this recorded family is forced.
            for child in self.owned_family(state):
                signal_owned(child, signal.SIGTERM)
            await until(lambda: not self.owned_family(state), 5)
        reap()

    async def recover(self):
        if self.state_path.exists():
            state = load(self.state_path)
            await self.stop_family(state)
            save(
                self.state_path,
                dict(
                    owner=self.args.owner,
                    binary=str(self.binary),
                    context=self.context,
                    generation=state.get("generation"),
                    master=None,
                    phase="stopped",
                ),
            )
        if self.socket_path.exists():
            if not stat.S_ISSOCK(self.socket_path.lstat().st_mode):
                raise ServingError("control path is not a socket")
            self.socket_path.unlink()

    def config(self):
        for name in ("client_temp", "proxy_temp", "fastcgi_temp", "uwsgi_temp", "scgi_temp"):
            directory(self.runtime / name)
        config = self.runtime / "nginx.conf"
        text = f"""daemon off; master_process on; worker_processes {self.args.workers};
worker_shutdown_timeout {self.args.shutdown_seconds}s;
pid {self.runtime}/nginx.pid; error_log {self.runtime}/nginx.log notice;
events {{worker_connections 4096;}}
http {{access_log off; sendfile on; open_file_cache off; gzip off; autoindex off; server_tokens off;
# Keep package-compiled absolute cache defaults out of an unprivileged reader.
client_body_temp_path {self.runtime}/client_temp;
proxy_temp_path {self.runtime}/proxy_temp;
fastcgi_temp_path {self.runtime}/fastcgi_temp;
uwsgi_temp_path {self.runtime}/uwsgi_temp;
scgi_temp_path {self.runtime}/scgi_temp;
types {{image/png png; image/jpeg jpg; image/webp webp;}}
server {{listen {self.address}:{self.port};
add_header Cache-Control "no-store" always; add_header X-Content-Type-Options "nosniff" always;
location = /_serving {{return 204;}}
location ~ "^/(?<image>(?<one>[0-9a-f]{{2}})(?<two>[0-9a-f]{{2}})[0-9a-f]{{60}})\\.(?<ext>png|jpg|webp)$" {{alias {self.public}/$one/$two/$image.$ext; limit_except GET {{deny all;}}}}
location / {{return 404;}}
}}}}
"""
        fd = os.open(
            config, os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_NOFOLLOW, 0o600
        )
        with os.fdopen(fd, "w") as f:
            f.write(text)
            f.flush()
            os.fsync(f.fileno())
        return config

    async def prepare(self):
        await self.recover()
        self.state = dict(
            owner=self.args.owner,
            binary=str(self.binary),
            context=self.context,
            generation=uuid.uuid4().hex,
            master=None,
            phase="prepared",
        )
        save(self.state_path, self.state)

    async def suspend(self):
        await self.stop_family(self.state)
        self.state["master"] = None
        self.state["phase"] = "prepared"
        save(self.state_path, self.state)

    async def start(self):
        if not self.state or self.state.get("phase") != "prepared":
            await self.prepare()
        config = self.config()
        generation = self.state["generation"]
        # The child cannot exec until its PID/birth/session are durably recorded.
        gate_r, gate_w = os.pipe()
        ready_r, ready_w = os.pipe()
        pid = os.fork()
        if pid == 0:
            try:
                os.close(gate_w)
                os.close(ready_r)
                os.setsid()
                os.write(ready_w, b"1")
                os.close(ready_w)
                go = os.read(gate_r, 1)
                os.close(gate_r)
                if go != b"1":
                    os._exit(0)
                dev = os.open(os.devnull, os.O_RDWR)
                os.dup2(dev, 0)
                os.dup2(dev, 1)
                os.dup2(dev, 2)
                os.close(dev)
                os.execv(
                    str(self.binary),
                    [
                        str(self.binary),
                        "-e",
                        str(self.runtime / "nginx.log"),
                        "-p",
                        str(self.runtime) + "/",
                        "-c",
                        str(config),
                    ],
                )
            except BaseException:
                os._exit(127)
        os.close(gate_r)
        os.close(ready_w)
        try:
            if (
                not select.select([ready_r], [], [], 5)[0]
                or os.read(ready_r, 1) != b"1"
            ):
                raise ServingError("reader child startup failed")
            master = process(pid)
            if not master or master["session"] != pid or master["group"] != pid:
                raise ServingError("reader session startup failed")
            self.state = dict(
                owner=self.args.owner,
                binary=str(self.binary),
                context=self.context,
                generation=generation,
                master=master,
                phase="starting",
            )
            save(self.state_path, self.state)
            os.write(gate_w, b"1")
        finally:
            os.close(ready_r)
            os.close(gate_w)
        await until(
            lambda: any(p["pid"] != pid for p in self.owned_family(self.state)), 5
        )
        self.state["phase"] = "ready"
        save(self.state_path, self.state)

    async def drain(self):
        if (
            self.state
            and self.state.get("phase") == "prepared"
            and self.state.get("master") is None
        ):
            return dict(
                drained=True,
                generation=self.state["generation"],
                workers=[],
                inactive=True,
            )
        if not self.state or not same(self.state["master"]):
            raise ServingError("master unavailable; recovery required")
        old = [
            p
            for p in self.owned_family(self.state)
            if p["pid"] != self.state["master"]["pid"]
        ]
        if not old:
            raise ServingError("no serving workers; recovery required")
        signal_owned(self.state["master"], signal.SIGHUP)
        old_ids = {(p["pid"], p["birth"]) for p in old}
        await until(
            lambda: any(
                (p["pid"], p["birth"]) not in old_ids
                and p["pid"] != self.state["master"]["pid"]
                for p in self.owned_family(self.state)
            ),
            5,
        )
        await until(
            lambda: all(not same(p) for p in old), self.args.shutdown_seconds + 5
        )
        reap()
        return dict(drained=True, generation=self.state["generation"], workers=old)

    async def command(self, raw):
        response = {}
        try:
            if len(raw) > 16384:
                raise ServingError("control request budget")
            request = json.loads(raw)
            if set(request) != {"op", "nonce"} or not re.fullmatch(
                "[0-9a-f]{64}", request["nonce"]
            ):
                raise ServingError("invalid control request")
            response["nonce"] = request["nonce"]
            async with self.commands:
                if request["op"] == "status":
                    result = dict(
                        generation=self.state["generation"],
                        workers=self.owned_family(self.state),
                        active=bool(
                            self.state.get("master")
                            and self.state.get("phase") == "ready"
                            and same(self.state["master"])
                        ),
                    )
                elif request["op"] == "activate":
                    if self.state["phase"] != "ready":
                        await self.start()
                    if not self.state.get("master") or not same(self.state["master"]):
                        raise ServingError(
                            "serving master unavailable; recovery required"
                        )
                    result = dict(active=True, generation=self.state["generation"])
                elif request["op"] == "suspend":
                    await self.suspend()
                    result = dict(inactive=True, generation=self.state["generation"])
                elif request["op"] == "drain":
                    result = await self.drain()
                elif request["op"] == "stop":
                    await self.stop_family(self.state)
                    self.stop_event.set()
                    result = dict(stopped=True)
                else:
                    raise ServingError("unsupported control operation")
            response.update(ok=True, result=result)
        except Exception as error:
            response.update(ok=False, error=str(error)[:240])
        return response

    async def run(self):
        # A single owning Rust parent needs no listening control socket. The inherited
        # pipes are its bounded request/response channel; EOF also fences parent death.
        loop = asyncio.get_running_loop()
        transport = None
        command_task = None

        async def commands():
            reader = asyncio.StreamReader(limit=16385)
            nonlocal transport
            transport, _ = await loop.connect_read_pipe(
                lambda: OwnerProtocol(reader, self.stop_event), sys.stdin.buffer
            )
            while not self.stop_event.is_set():
                try:
                    raw = await reader.readline()
                except ValueError:
                    raise ServingError("control request budget") from None
                if not raw:
                    self.stop_event.set()
                    return
                response = await self.command(raw)
                # Replies are <=16KiB. Blocking stdout is bounded by the owning parent's
                # pipe capacity; write via a nonblocking fd and deadline below.
                await self.reply(response)

        try:
            if self.args.action == "prepare":
                await self.prepare()
            else:
                await self.start()
            for sig in (signal.SIGTERM, signal.SIGINT):
                loop.add_signal_handler(sig, self.stop_event.set)
            os.set_blocking(sys.stdout.fileno(), False)
            await self.reply(dict(ready=True, generation=self.state["generation"]))
            command_task = asyncio.create_task(commands())
            stopping = asyncio.create_task(self.stop_event.wait())
            done, _ = await asyncio.wait(
                [command_task, stopping], return_when=asyncio.FIRST_COMPLETED
            )
            if command_task in done:
                await command_task
            stopping.cancel()
        finally:
            if command_task:
                command_task.cancel()
                try:
                    await command_task
                except (asyncio.CancelledError, Exception):
                    pass
            if transport:
                transport.close()
            if self.state:
                async with self.commands:
                    await self.stop_family(self.state)
                self.state["master"] = None
                self.state["phase"] = "stopped"
                save(self.state_path, self.state)

    async def reply(self, value):
        raw = (json.dumps(value, separators=(",", ":")) + "\n").encode()
        if len(raw) > 16384:
            raise ServingError("control reply budget")
        deadline = time.monotonic() + 3
        while raw:
            try:
                raw = raw[os.write(sys.stdout.fileno(), raw) :]
            except BlockingIOError:
                if time.monotonic() > deadline:
                    raise ServingError("control owner stopped reading")
                await asyncio.sleep(0.01)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("action", choices=["serve", "prepare", "recover"])
    p.add_argument("--runtime", required=True)
    p.add_argument("--public", required=True)
    p.add_argument("--nginx", required=True)
    p.add_argument("--bind", default="127.0.0.1:19670")
    p.add_argument("--owner", required=True)
    p.add_argument("--workers", type=int, default=1)
    p.add_argument("--shutdown-seconds", type=int, default=30)
    args = p.parse_args()
    supervisor = Supervisor(args)
    try:
        asyncio.run(
            supervisor.recover() if args.action == "recover" else supervisor.run()
        )
    finally:
        os.close(supervisor.lock)


if __name__ == "__main__":
    main()
