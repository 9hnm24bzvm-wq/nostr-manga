"""One unchanged async test plus transparent Wire.call request observations."""
import hashlib,importlib.metadata,importlib.util,json,os,sys,socket,unittest
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parent;PROJECT=R/'post-nginx-integration-worktree/manga-protocol-lab';DEPS=R/'finite-fixture-worktree/.build/fixture-python-recovery/owned/site-packages';TEST=PROJECT/'bench/library-node/test_mixed_reads.py'
assert sys.flags.optimize==0
gate=int(os.environ.pop('COMPACT_OWNED_GATE_FD'))
try:admitted=os.read(gate,1)==b'G'
finally:os.close(gate)
if not admitted:os._exit(73)
listener_gate=int(os.environ.pop('MIXED_LISTENER_GATE_FD'));listener_admitted=False
blocked=[];bindings=[];connections=[]
def refuse(event):
 blocked.append(event);raise RuntimeError('Controlled fixture boundary: '+event)
def controlled(event,args):
 if event in {'subprocess.Popen','os.system','sqlite3.connect'}:refuse(event)
 if event=='socket.bind':
  sock,address=args
  if not (sock.family==socket.AF_INET and sock.type&15==socket.SOCK_STREAM and address==('127.0.0.1',0) and not bindings):refuse(event)
  bindings.append(sock)
 if event=='socket.connect':
  sock,address=args
  if not (len(bindings)==1 and isinstance(address,tuple) and len(address)==2 and address[0]=='127.0.0.1' and address[1]==bindings[0].getsockname()[1]):refuse(event)
  connections.append({'host':address[0],'port':address[1]})
 if event=='socket.getaddrinfo' and args[0]!='127.0.0.1':refuse(event)
sys.addaudithook(controlled)
assert os.environ['NODE_RESULTS']==str(D/'output')
import run
original=run.Wire.call;http=[]
async def observed(self,method,path,*args,**kwargs):
 global listener_admitted
 if not listener_admitted:
  assert len(bindings)==1
  host,port=bindings[0].getsockname();assert host=='127.0.0.1' and 1024<port<65536
  pending=D/'first-request-listener.pending';pending.write_text(json.dumps({'host':host,'port':port,'descriptor':bindings[0].fileno()},sort_keys=True)+'\n');os.replace(pending,D/'first-request-listener.json')
  try:listener_admitted=os.read(listener_gate,1)==b'L'
  finally:os.close(listener_gate)
  if not listener_admitted:raise RuntimeError('Owned listener admission refused')
 result=await original(self,method,path,*args,**kwargs)
 http.append({'method':method,**result[1]})
 return result
run.Wire.call=observed
spec=importlib.util.spec_from_file_location('bounded_mixed_reads',TEST);module=importlib.util.module_from_spec(spec);sys.modules[spec.name]=module;spec.loader.exec_module(module)
name='test_large_and_extreme_selective_restore_rejects_corrupt_200'
suite=unittest.TestSuite([module.LibraryReadHttp(name)]);assert suite.countTestCases()==1
result=unittest.TextTestRunner(verbosity=2).run(suite)
case=dict(testsRun=result.testsRun,failures=len(result.failures),errors=len(result.errors),skipped=len(result.skipped),success=result.wasSuccessful())
(D/'case-result.json').write_text(json.dumps(case,sort_keys=True,indent=2)+'\n')
(D/'http-observations.json').write_text(json.dumps(http,sort_keys=True,indent=2)+'\n')
import cbor2,coincurve,aiohttp,PIL
assert Path(cbor2.__file__).resolve().is_relative_to(DEPS) and Path(coincurve.__file__).resolve().is_relative_to(DEPS)
assert importlib.metadata.version('cbor2')=='5.6.5' and importlib.metadata.version('coincurve')=='21.0.0'
assert Path(run.__file__).resolve()==PROJECT/'bench/library-node/run.py'
closure={}
for value in list(sys.modules.values()):
 filename=getattr(value,'__file__',None)
 if not filename:continue
 p=Path(filename).resolve()
 if p.is_file() and p.is_relative_to(PROJECT):closure[str(p.relative_to(PROJECT))]=hashlib.sha256(p.read_bytes()).hexdigest()
(D/'import-closure.json').write_text(json.dumps(closure,sort_keys=True,indent=2)+'\n')
(D/'runtime.json').write_text(json.dumps({'python':sys.version,'versions':{p:importlib.metadata.version(p) for p in ['cbor2','coincurve','aiohttp','Pillow']},'sourceImportCount':len(closure),'controlledAuditHook':True,'blockedEvents':blocked,'boundSocketCount':len(bindings),'listenerAdmitted':listener_admitted,'connections':connections,'fullRequirementsParity':False,'historicalAiohttpPin':'3.13.5','actualAiohttp':'3.14.4','scope':'one unchanged controlled-loopback correctness method; transparent original Wire.call observations; no real application, SQL, WAN, restore or throughput qualification'},sort_keys=True,indent=2)+'\n')
assert case==dict(testsRun=1,failures=0,errors=0,skipped=0,success=True)
assert len(bindings)==1 and connections and len({row['port'] for row in connections})==1 and not blocked
assert len(http)==9 and all(row['status']==200 for row in http)
