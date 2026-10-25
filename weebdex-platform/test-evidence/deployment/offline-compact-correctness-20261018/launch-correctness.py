"""Execute the existing correctness file unchanged, with offline audit enforcement."""
import hashlib,importlib.metadata,json,os,runpy,sys
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parent;PROJECT=R/'repository/manga-protocol-lab';DEPS=R/'finite-fixture-worktree/.build/fixture-python-recovery/owned/site-packages';TEST=PROJECT/'bench/library-node/test_compact.py'
assert sys.flags.optimize==0
# Do no dependency import or correctness work until the parent has captured
# and validated this exact child birth. EOF is the safe failed-admission path.
gate=int(os.environ.pop('COMPACT_OWNED_GATE_FD'))
try:admitted=os.read(gate,1)==b'G'
finally:os.close(gate)
if not admitted:os._exit(73)
blocked=[]
def offline(event,args):
 if event in {'socket.connect','socket.connect_ex','socket.bind','socket.listen','subprocess.Popen','os.system','sqlite3.connect'}:
  blocked.append(event);raise RuntimeError('Offline correctness boundary: '+event)
sys.addaudithook(offline)
assert os.environ['NODE_RESULTS']==str(D/'output')
runpy.run_path(str(TEST),run_name='__main__')
import cbor2,coincurve,aiohttp,PIL
assert Path(cbor2.__file__).resolve().is_relative_to(DEPS) and Path(coincurve.__file__).resolve().is_relative_to(DEPS)
assert importlib.metadata.version('cbor2')=='5.6.5' and importlib.metadata.version('coincurve')=='21.0.0'
actual=Path(sys.modules['run'].__file__).resolve();assert actual==PROJECT/'bench/library-node/run.py'
closure={}
for module in list(sys.modules.values()):
 name=getattr(module,'__file__',None)
 if not name:continue
 p=Path(name).resolve()
 if p.is_file() and p.is_relative_to(PROJECT):closure[str(p.relative_to(PROJECT))]=hashlib.sha256(p.read_bytes()).hexdigest()
(D/'import-closure.json').write_text(json.dumps(closure,sort_keys=True,indent=2)+'\n')
(D/'runtime.json').write_text(json.dumps({'python':sys.version,'versions':{p:importlib.metadata.version(p) for p in ['cbor2','coincurve','aiohttp','Pillow']},'sourceImportCount':len(closure),'offlineAuditHook':True,'blockedEvents':blocked,'fullRequirementsParity':False,'historicalAiohttpPin':'3.13.5','actualAiohttp':'3.14.4','scope':'in-memory canonical-codec/compaction equivalence only; no throughput, server, WAN, SQL or scaling qualification'},sort_keys=True,indent=2)+'\n')
