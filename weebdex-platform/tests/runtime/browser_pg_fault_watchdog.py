#!/usr/bin/env python3
"""Fixed-cluster disposable PG outage/restart owner; never a deployment tool."""
from __future__ import annotations
import ctypes, hashlib, json, os, select, shlex, signal, socket, subprocess, sys, tempfile, time
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]
PG_BIN=ROOT.parent/'manga-protocol-lab/vendor/pgsql/bin'
OUTAGE_SECONDS=15
ARM_SECONDS=20
class Disarmed(Exception):pass

def row(pid):
    try:
        fields=Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].split()
        return {'pid':pid,'state':fields[0],'ppid':int(fields[1]),'group':int(fields[2]),'session':int(fields[3]),'start_ticks':int(fields[19])}
    except (FileNotFoundError,ProcessLookupError):return None

def identity(value):return {key:value[key]for key in ['pid','start_ticks','group','session']}

def write(path,value):
    with tempfile.NamedTemporaryFile(mode='w',dir=path.parent,prefix=path.name+'.',suffix='.new',delete=False)as output:
        temporary=Path(output.name);output.write(json.dumps(value,indent=2)+'\n');output.flush();os.fsync(output.fileno())
    try:temporary.replace(path)
    finally:
        if temporary.exists():temporary.unlink()

def same(value):
    current=row(value['pid']);return current if current and identity(current)==value else None

def send(value,signum):
    fd=None
    try:
        fd=os.pidfd_open(value['pid'])
        if same(value):signal.pidfd_send_signal(fd,signum)
    except ProcessLookupError:pass
    finally:
        if fd is not None:os.close(fd)

def members():
    rows={};result=[]
    for entry in Path('/proc').iterdir():
        if entry.name.isdigit():
            value=row(int(entry.name))
            if value:rows[value['pid']]=value
    for value in rows.values():
        if value['pid']==os.getpid():continue
        ancestor=value;seen=set();owned=value['session']==os.getpid()
        while not owned and ancestor and ancestor['pid']not in seen:
            seen.add(ancestor['pid'])
            if ancestor['ppid']==os.getpid():owned=True;break
            ancestor=rows.get(ancestor['ppid'])
        if owned:result.append(value)
    return result

def reap(leader):
    if leader:leader.poll()
    for value in members():
        if value['state']=='Z'and value['ppid']==os.getpid()and (not leader or value['pid']!=leader.pid):
            try:os.waitpid(value['pid'],os.WNOHANG)
            except ChildProcessError:pass

def cleanup(leader):
    forced=False;exited_before_stop=False
    if leader:
        # Cancellation may bypass the ready loop's next poll. An already
        # terminal PG at this pre-stop poll is never watchdog-requested teardown.
        exited_before_stop=leader.poll()is not None
        if not exited_before_stop:
            current=row(leader.pid)
            if current:send(identity(current),signal.SIGINT)
            try:leader.wait(timeout=15)
            except subprocess.TimeoutExpired:forced=True
    for signum in [signal.SIGTERM,signal.SIGKILL]:
        deadline=time.monotonic()+5
        while time.monotonic()<deadline:
            reap(leader);remaining=members()
            if not remaining:return forced,[],exited_before_stop
            forced=True
            for value in remaining:
                if value['state']!='Z':send(identity(value),signum)
            time.sleep(.05)
    reap(leader);return forced,[identity(value)for value in members()],exited_before_stop

def original_members(expected):
    # The original PG is our verified sibling, not our waitpid child. Its
    # dedicated session remains an ownership fence even if the outer owner dies.
    leader=row(expected['pid'])
    if leader and identity(leader)!=expected:return []
    result=[]
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit():continue
        current=row(int(entry.name))
        if current and current['session']==expected['session']and current['start_ticks']>=expected['start_ticks']and current['state']!='Z':result.append(current)
    return result

def stop_orphaned_original(expected):
    current=same(expected)
    if current and current['state']!='Z':send(expected,signal.SIGINT)
    deadline=time.monotonic()+15
    while original_members(expected)and time.monotonic()<deadline:time.sleep(.05)
    forced=False
    for signum in [signal.SIGTERM,signal.SIGKILL]:
        deadline=time.monotonic()+5
        while time.monotonic()<deadline:
            remaining=original_members(expected)
            if not remaining:return forced,[]
            forced=True
            for value in remaining:send(identity(value),signum)
            time.sleep(.05)
    return forced,[identity(value)for value in original_members(expected)]

def closed(port):
    with socket.socket()as probe:probe.settimeout(.5);return probe.connect_ex(('127.0.0.1',port))!=0

def run():
    if len(sys.argv)!=2:raise RuntimeError('private watchdog request required')
    request_path=Path(sys.argv[1]);meta=request_path.lstat()
    if request_path.is_symlink():raise RuntimeError('invalid request')
    if not request_path.is_file()or meta.st_size>4096:raise RuntimeError('private watchdog request bounds')
    request=json.loads(request_path.read_text())
    if set(request)!={'root','request_id','pg_identity','supervisor_identity','postgres_binary_sha256'}:raise RuntimeError('watchdog request fields')
    root=Path(request['root']).resolve();parent=(ROOT/'test-runtime').resolve()
    if root.parent!=parent or not root.name.startswith('browser-final-')or root.is_symlink()or root.stat().st_mode&0o077:raise RuntimeError('owned private fixture required')
    request_id=request['request_id']
    if not isinstance(request_id,str)or not request_id or len(request_id)>48 or not all(c.isascii()and(c.isalnum()or c in '_-')for c in request_id):raise RuntimeError('watchdog identity bounds')
    if request_path.resolve()!=root/f'pg-home-fault-{request_id}.json':raise RuntimeError('fixed private request path required')
    marker=json.loads((root/'fixture-owned.json').read_text());port=marker.get('pg_port')
    if marker.get('fixture_only')is not True or marker.get('database')!='weebdex_browser_test'or not isinstance(port,int)or not 1024<port<65536 or port==5432:raise RuntimeError('owned fixture port required')
    if (root/'data').is_symlink()or not(root/'data/PG_VERSION').is_file():raise RuntimeError('fixed disposable PG root required')
    expected=request['pg_identity'];supervisor=request['supervisor_identity']
    for value in [expected,supervisor]:
        if not isinstance(value,dict)or set(value)!={'pid','start_ticks','group','session'}or any(isinstance(n,bool)or not isinstance(n,int)or n<=0 for n in value.values()):raise RuntimeError('process birth identity required')
    original=same(expected)
    if not original or original['ppid']!=os.getppid()or not same(supervisor)or supervisor['pid']!=os.getppid()or os.getsid(0)!=os.getpid():raise RuntimeError('owned direct-parent process identities required')
    binary=(PG_BIN/'postgres').resolve()
    if Path(f'/proc/{expected["pid"]}/exe').resolve()!=binary or hashlib.sha256(binary.read_bytes()).hexdigest()!=request['postgres_binary_sha256']:raise RuntimeError('reviewed original PG executable required')
    command=[str(binary),'-D',str(root/'data'),'-k','','-h','127.0.0.1','-p',str(port),'-c','max_connections=32','-c','shared_buffers=16MB','-c','fsync=off','-c','full_page_writes=off']
    # PostgreSQL rewrites its process title. Its private postmaster.opts records
    # the actual launch argv without relying on mutable /proc/cmdline display.
    options=root/'data/postmaster.opts';pidfile=root/'data/postmaster.pid'
    if options.is_symlink()or pidfile.is_symlink()or options.stat().st_size>65536 or pidfile.stat().st_size>65536:raise RuntimeError('private PG launch records required')
    actual=shlex.split(options.read_text());lines=pidfile.read_text().splitlines()
    if len(actual)!=len(command)or Path(actual[0]).resolve()!=binary or actual[1:]!=command[1:]or len(lines)<6 or lines[0]!=str(expected['pid'])or Path(lines[1]).resolve()!=root/'data'or lines[3]!=str(port):raise RuntimeError('exact disposable PG command/data identity required')
    config_hashes={}
    for name in ['postgresql.conf','pg_hba.conf']:
        path=root/'data'/name
        if path.is_symlink()or not path.is_file()or path.stat().st_size>1024*1024:raise RuntimeError('private fixed PG config required')
        config_hashes[name]=hashlib.sha256(path.read_bytes()).hexdigest()
    libc=ctypes.CDLL(None,use_errno=True)
    if libc.prctl(36,1,0,0,0)!=0:raise RuntimeError('watchdog subreaper unavailable')
    parent_fd=os.pidfd_open(supervisor['pid']);state_path=root/f'pg-home-fault-{request_id}-state.json';start=root/f'pg-home-fault-{request_id}.start';resume=root/f'pg-home-fault-{request_id}.resume'
    cancelled=False;leader=None;error=None;forced=False;remaining_original=[];disarmed=False
    def cancel(*_):
        nonlocal cancelled;cancelled=True
    signal.signal(signal.SIGTERM,cancel);signal.signal(signal.SIGINT,cancel)
    state={'fixture_only':True,'request_id':request_id,'status':'armed','outage_seconds':OUTAGE_SECONDS,'original_postmaster':expected,'postgres_binary_sha256':request['postgres_binary_sha256'],'cluster_config_hashes':config_hashes,'independent_restart_watchdog':True,'terminal':False}
    write(state_path,state)
    try:
        deadline=time.monotonic()+ARM_SECONDS
        while not start.exists()and same(expected)and time.monotonic()<deadline and not cancelled and not select.select([parent_fd],[],[],0)[0]:time.sleep(.05)
        if not start.exists()and same(expected):
            if select.select([parent_fd],[],[],0)[0]:
                # The exact outer owner is gone. Finish its original PG too;
                # never leave a sibling listener merely because activation raced.
                forced,remaining_original=stop_orphaned_original(expected)
            else:
                # No outage started. The live outer owner still owns the PG.
                disarmed=True;raise Disarmed()
            raise RuntimeError('outer owner exited before outage activation')
        # Refuse a duplicate cluster. The main owner must have joined the exact
        # original postmaster/session before releasing the outage start marker.
        if same(expected)or not closed(port):raise RuntimeError('original cluster not joined')
        state.update(status='outage',restart_deadline_unix=time.time()+OUTAGE_SECONDS);write(state_path,state)
        restart_at=time.monotonic()+OUTAGE_SECONDS
        while time.monotonic()<restart_at and not resume.exists()and not cancelled and not select.select([parent_fd],[],[],0)[0]:time.sleep(.05)
        if hashlib.sha256(binary.read_bytes()).hexdigest()!=request['postgres_binary_sha256']or any(hashlib.sha256((root/'data'/name).read_bytes()).hexdigest()!=value for name,value in config_hashes.items()):raise RuntimeError('fixed PG executable/config changed before restart')
        leader=subprocess.Popen(command,cwd=ROOT,stdin=subprocess.DEVNULL)
        current=row(leader.pid)
        if not current:raise RuntimeError('replacement postmaster identity unavailable')
        state.update(status='restarting',postmaster=identity(current));write(state_path,state)
        ready_until=time.monotonic()+25
        while time.monotonic()<ready_until:
            if leader.poll()is not None:raise RuntimeError('replacement postmaster exited')
            probe=subprocess.run([str(PG_BIN/'pg_isready'),'-h','127.0.0.1','-p',str(port),'-d','postgres'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=2)
            if probe.returncode==0:break
            time.sleep(.1)
        else:raise RuntimeError('replacement PG readiness deadline')
        state.update(status='ready',ready_unix=time.time(),active=False);write(state_path,state)
        # Keep ownership of the actual restarted PG until the outer supervisor
        # drains it. A dead outer owner cannot leave this listener indefinitely.
        # The outer fixture's maximum3600-second observation plus its reviewed
        # bounded native drain fits here; parent death still closes this early.
        lifetime=time.monotonic()+4200
        while not cancelled and time.monotonic()<lifetime and not select.select([parent_fd],[],[],0)[0]:
            if leader.poll()is not None:raise RuntimeError('replacement PG exited unexpectedly')
            time.sleep(.1)
    except Disarmed:pass
    except BaseException as exc:error=type(exc).__name__
    finally:
        if select.select([parent_fd],[],[],0)[0]and original_members(expected):
            orphan_forced,remaining_original=stop_orphaned_original(expected);forced=forced or orphan_forced
        cleanup_forced,remaining,exited_before_stop=cleanup(leader);forced=forced or cleanup_forced
        if exited_before_stop and error is None:error='RuntimeError'
        status='disarmed'if disarmed and not remaining else('verified_stopped'if not remaining and not remaining_original and closed(port)else'cleanup_unconfirmed')
        state.update(status=status,terminal=True,forced=forced,remaining_owned_processes=remaining+remaining_original,error=error,original_cluster_preserved=disarmed,returncode=leader.returncode if leader else None,finished_unix=time.time());write(state_path,state);os.close(parent_fd)
    return 1 if error or forced or state['status']not in ['verified_stopped','disarmed']else 0

if __name__=='__main__':
    try:sys.exit(run())
    except BaseException as error:
        if isinstance(error,SystemExit):raise
        print('Disposable PG fault watchdog failed; no recovery success asserted',file=sys.stderr);sys.exit(1)
