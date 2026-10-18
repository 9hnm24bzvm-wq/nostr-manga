import hashlib,importlib.util,json,sys,time,unittest
from pathlib import Path
D=Path(__file__).resolve().parent;ROOT=D.parent/'nginx-runtime-fix-worktree/manga-protocol-lab';TEST=ROOT/'tests/test_serving_supervisor.py';BINARY=(D.parent/'nginx-qualification-20261018T1329/official-package/usr/sbin/nginx').resolve()
assert BINARY.is_file() and BINARY.is_relative_to(D.parent/'nginx-qualification-20261018T1329/official-package')
sys.path.insert(0,str(ROOT/'tests'))
spec=importlib.util.spec_from_file_location('restored_serving_tests',TEST);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
# Actual package version override; the historical vendor directory is not recreated.
m.NGINX=BINARY
original_unlink=Path.unlink
unlink_account={'events':0,'largestPayloadBytes':0,'maximumAllocatedBeforeUnlinkBytes':0}
def accounted_unlink(path,*args,**kwargs):
 if path.suffix=='.png' and path.is_relative_to(D/'runtime') and path.is_file():
  value=path.stat();unlink_account['events']+=1;unlink_account['largestPayloadBytes']=max(unlink_account['largestPayloadBytes'],value.st_size);unlink_account['maximumAllocatedBeforeUnlinkBytes']=max(unlink_account['maximumAllocatedBeforeUnlinkBytes'],value.st_blocks*512)
  temp=D/'unlinked-block-accounting.next';temp.write_text(json.dumps(unlink_account));temp.replace(D/'unlinked-block-accounting.json')
 return original_unlink(path,*args,**kwargs)
Path.unlink=accounted_unlink
ports=[]
original_setup=m.ServingCLI.setUp
def record_setup(self):
 original_setup(self)
 argv=self.args();assert argv[argv.index('--bind')+1]=='127.0.0.1:'+str(self.port) and self.port>1024
 ports.append({'test':self.id(),'bind':'127.0.0.1','port':self.port})
 (D/'test-loopback-ports.json').write_text(json.dumps(ports))
m.ServingCLI.setUp=record_setup
seam=[]
original_cleanup=m.ServingCLI.cleanup
def check_private_paths(self):
 try:
  config=self.runtime/'nginx.conf'
  if config.is_file():
   text=config.read_text()
   for directive,name in [('client_body_temp_path','client_temp'),('proxy_temp_path','proxy_temp'),('fastcgi_temp_path','fastcgi_temp'),('uwsgi_temp_path','uwsgi_temp'),('scgi_temp_path','scgi_temp')]:
    path=self.runtime/name
    assert directive+' '+str(path)+';' in text
    assert path.is_dir() and not path.is_symlink() and path.resolve()==path and path.stat().st_uid==__import__('os').geteuid() and path.stat().st_mode&0o022==0
   log=self.runtime/'nginx.log'
   assert log.is_file() and not log.is_symlink() and log.stat().st_uid==__import__('os').geteuid() and self.runtime.stat().st_mode&0o077==0
   seam.append({'case':self.id(),'validatedPrivateTempDirectories':5,'startupLogOwnedUnderPrivateRuntime':True})
 finally:original_cleanup(self)
m.ServingCLI.cleanup=check_private_paths
durations=[]
def retain_failure_log(test):
 if hasattr(test,'root') and test.root.exists():
  for i,p in enumerate(test.root.rglob('nginx.log')):
   (D/('failed-nginx-'+str(len(durations))+'-'+str(i)+'.log')).write_bytes(p.read_bytes())
class Result(unittest.TextTestResult):
 def addFailure(self,test,err):retain_failure_log(test);super().addFailure(test,err)
 def addError(self,test,err):retain_failure_log(test);super().addError(test,err)
 def startTest(self,test):self.start=time.monotonic();super().startTest(test)
 def stopTest(self,test):durations.append({'name':test.id(),'seconds':round(time.monotonic()-self.start,3)});super().stopTest(test)
suite=unittest.defaultTestLoader.loadTestsFromTestCase(m.ServingCLI);assert suite.countTestCases()==8
result=unittest.TextTestRunner(verbosity=2,resultclass=Result,failfast=True).run(suite)
value={'run':result.testsRun,'planned':8,'failures':len(result.failures),'errors':len(result.errors),'skipped':len(result.skipped),'success':result.wasSuccessful(),'cases':durations,'testSha256':hashlib.sha256(TEST.read_bytes()).hexdigest(),'unlinkedBlockAccounting':unlink_account,'privatePathRegressionProofs':seam,'loopbackPortsRecorded':len(ports),'instrumentation':'read-only setUp argv/port recording and pre-cleanup failure-log retention','actualNginxBinarySha256':hashlib.sha256(BINARY.read_bytes()).hexdigest(),'override':'module.NGINX explicitly set to isolated official 1.30.6 binary; historical source constant unchanged'}
(D/'eight-result.json').write_text(json.dumps(value,sort_keys=True,indent=2)+'\n');sys.exit(0 if value['success'] and value['run']==8 else 1)
