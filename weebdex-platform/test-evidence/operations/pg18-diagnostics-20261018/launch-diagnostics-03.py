"""Retaining verification adapter for the unchanged17-method PG18 fixture."""
import ctypes,hashlib,importlib.util,json,os,resource,signal,stat,subprocess,sys,tempfile,time,types,unittest
from pathlib import Path
D=Path(__file__).resolve().parent;G=D.parent/'pg18-diagnostics-worktree';P=G/'manga-protocol-lab';E=D/'runtime-evidence-03';ROOT=D/'root';BIN=ROOT/'usr/lib/postgresql/18/bin';RUNTIME=Path(os.environ['DIAGNOSTIC_RUNTIME'])
fd=int(os.environ.pop('OWNER_GATE_FD'))
try:ready=os.read(fd,1)==b'G'
finally:os.close(fd)
if not ready:os._exit(73)
assert RUNTIME.parent==Path('/tmp') and RUNTIME.name.startswith('mixed-diagnostic-') and RUNTIME.is_dir() and not RUNTIME.is_symlink()
st=RUNTIME.stat();assert st.st_uid==os.getuid() and stat.S_IMODE(st.st_mode)==0o700
assert not any(RUNTIME.iterdir()),'runtime must be new and empty'
resource.setrlimit(resource.RLIMIT_CORE,(0,0))
H=G/'weebdex-platform/tests/runtime/auth_fixture_guard.py';assert hashlib.sha256(H.read_bytes()).hexdigest()=='17ff9c2eb04a4bdefaa3b61e975e77567f873208a83454afcdd6ca19c5cfc33e'
spec=importlib.util.spec_from_file_location('owner_helper',H);h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
os.umask(0o077);listen_gate=int(os.environ.pop('LISTENER_GATE_FD'))
env={k:v for k,v in os.environ.items() if not k.startswith('PG') and k not in {'LD_LIBRARY_PATH','LD_PRELOAD'}}
env.update(LD_LIBRARY_PATH=str(ROOT/'usr/lib/x86_64-linux-gnu'),LC_ALL='C',PGHOST='127.0.0.1',PGPORT='55437',PGUSER='diagnostic_fixture',PGDATABASE='mixed_diagnostic',PGCONNECT_TIMEOUT='5',PYTHONDONTWRITEBYTECODE='1')
report={'schema':1,'status':'running','retainedDatadir':True,'retainedScratch':True,'tests':None,'commands':[]};pg=None;pg_ident=None;pg_log=None

def save(): (E/'fixture-result.json').write_text(json.dumps(report,sort_keys=True,indent=2)+'\n')
def command(argv,*,seconds=30,body=None):
 value=subprocess.run(argv,input=body,capture_output=True,text=True,timeout=seconds,env=env)
 index=len(report['commands']);out=E/f'command-{index:02d}-{Path(argv[0]).name}.stdout';err=E/f'command-{index:02d}-{Path(argv[0]).name}.stderr'
 out.write_text(value.stdout);err.write_text(value.stderr);report['commands'].append({'tool':Path(argv[0]).name,'returnCode':value.returncode,'stdoutSha256':hashlib.sha256(out.read_bytes()).hexdigest(),'stderrSha256':hashlib.sha256(err.read_bytes()).hexdigest()});save()
 assert value.returncode==0,'fixture tool failed: '+Path(argv[0]).name
 return value.stdout.strip()
def sql(body):return command([str(BIN/'psql'),'-X','-q','-A','-t','-v','ON_ERROR_STOP=1'],body=body,seconds=10)
try:
 command([str(BIN/'initdb'),'-D',str(RUNTIME/'data'),'-L',str(ROOT/'usr/share/postgresql/18'),'--username=diagnostic_fixture','--auth-local=reject','--auth-host=trust','--no-locale','--encoding=UTF8','--no-clean'])
 with (RUNTIME/'data/postgresql.conf').open('a') as config:
  config.write("\nlisten_addresses='127.0.0.1'\nport=55437\nunix_socket_directories=''\nshared_buffers='16MB'\nmax_wal_size='512MB'\n")
 gate_read,gate_write=os.pipe();pg_env=dict(env,POSTMASTER_GATE_FD=str(gate_read));pg_log=(RUNTIME/'postgres.log').open('w')
 pg=subprocess.Popen([sys.executable,'-B',str(D/'exec-after-admission.py'),str(BIN/'postgres'),'-D',str(RUNTIME/'data')],stdin=subprocess.DEVNULL,stdout=pg_log,stderr=subprocess.STDOUT,env=pg_env,start_new_session=True,pass_fds=(gate_read,));os.close(gate_read)
 try:
  birth=h.row(pg.pid);assert birth and birth['ppid']==os.getpid() and birth['group']==birth['session']==pg.pid
  pg_ident=h.identity(birth);report['postmasterBirth']=pg_ident;save();os.write(gate_write,b'G')
 finally:os.close(gate_write)
 # No connection or database command occurs before exact listener ownership.
 try:admitted=os.read(listen_gate,1)==b'L'
 finally:os.close(listen_gate)
 assert admitted and pg.poll() is None,'listener ownership admission refused'
 env['PGDATABASE']='postgres'
 assert sql('SHOW data_directory;')==str(RUNTIME/'data')
 version=int(sql('SHOW server_version_num;'));assert version==180006
 assert sql('SHOW port;')=='55437' and sql('SHOW unix_socket_directories;')==''
 assert sql('SELECT host(inet_server_addr());')=='127.0.0.1'
 command([str(BIN/'createdb'),'mixed_diagnostic']);env['PGDATABASE']='mixed_diagnostic'
 migrations=sorted((P/'server/migrations').glob('*.sql'));assert migrations
 sql('BEGIN;\n'+'\n'.join(p.read_text() for p in migrations)+'\nCOMMIT;')
 wrapper=(P/'bench/library-node/test-pg-diagnostics.sh').read_text();seed=wrapper.split("<<'SQL'\n",1)[1].split('\nSQL',1)[0];sql(seed)
 os.environ.update(env,MIXED_DIAGNOSTIC_TEST_DB='mixed_diagnostic',MIXED_PSQL=str(BIN/'psql'),MIXED_DIAGNOSTIC_OWNED_RUNTIME=str(RUNTIME),MIXED_DIAGNOSTIC_MAX_WAL_MB='512')
 path=P/'bench/library-node/test_pg_diagnostics.py';spec=importlib.util.spec_from_file_location('bounded_pg_diagnostics',path);module=importlib.util.module_from_spec(spec);sys.modules[spec.name]=module;spec.loader.exec_module(module)
 scratch=RUNTIME/'retained-scratch';scratch.mkdir(mode=0o700)
 class RetainedTemporaryDirectory:
  def __init__(self,*,prefix):self.name=tempfile.mkdtemp(prefix=prefix,dir=scratch)
  def cleanup(self):pass
 # Only wrapper-file retention differs; no SQL, timeout or test assertion changes.
 module.tempfile=types.SimpleNamespace(TemporaryDirectory=RetainedTemporaryDirectory)
 suite=unittest.defaultTestLoader.loadTestsFromModule(module);assert suite.countTestCases()==17
 result=unittest.TextTestRunner(verbosity=2).run(suite)
 tests={'testsRun':result.testsRun,'failures':len(result.failures),'errors':len(result.errors),'skipped':len(result.skipped),'successful':result.wasSuccessful()};report.update(tests=tests,serverVersionNum=version,migrations=len(migrations),status='passed' if tests=={'testsRun':17,'failures':0,'errors':0,'skipped':0,'successful':True} else 'failed')
 assert report['status']=='passed','diagnostic assertion or runtime failure'
except BaseException as error:report.update(status='failed',errorClass=type(error).__name__,error=str(error));raise
finally:
 if pg is not None:
  if pg_ident is None:
   try:pg.wait(timeout=5)
   except subprocess.TimeoutExpired:report['postmasterStopError']='unverified birth did not abort'
  else:
   report['postmasterForced']=False
   for sig,seconds in [(signal.SIGINT,15),(signal.SIGTERM,5),(signal.SIGKILL,5)]:
    if pg.poll() is not None:break
    if sig!=signal.SIGINT:report['postmasterForced']=True
    h.send(pg_ident,sig)
    try:pg.wait(timeout=seconds)
    except subprocess.TimeoutExpired:pass
   report['postmasterExit']=pg.poll()
 if pg_log:pg_log.close()
 report['postmasterPidFileRemoved']=not(RUNTIME/'data/postmaster.pid').exists()
 if pg is not None and (report.get('postmasterExit')!=0 or report.get('postmasterForced') is not False or not report['postmasterPidFileRemoved']):report['status']='failed'
 save()
 if report['status']!='passed':sys.exit(1)
