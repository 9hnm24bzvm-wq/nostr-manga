import pathlib,json,hashlib,subprocess,os,importlib.util,sys
root=pathlib.Path.cwd(); receipts=pathlib.Path(os.environ['RECEIPTS']); prior=pathlib.Path(os.environ['PRIOR'])
old=json.loads((prior/'before.json').read_text())
node=(receipts/'node.args').read_text().splitlines();py=(receipts/'python.args').read_text().splitlines()
keys=set(old['pins'])|{'weebdex-platform/tests/ci/public-upload-native-failure-fixture.mjs','weebdex-platform/tests/ci/test_public_upload_native_failure_projection.py'}
sha=lambda b:hashlib.sha256(b).hexdigest()
def capture():
 pins={}
 for name in sorted(keys):
  p=root/name;assert p.is_file() and not p.is_symlink();b=p.read_bytes();assert b==subprocess.check_output(['git','show','HEAD:'+name]);pins[name]=sha(b)
 ext={}
 for name in old['externalInputs']:
  b=(receipts/name).read_bytes();ext[name]={'bytes':len(b),'sha256':sha(b)}
 assert ext==old['externalInputs']
 return {'schema':1,'source':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'tree':subprocess.check_output(['git','rev-parse','HEAD^{tree}'],text=True).strip(),'pins':pins,'externalInputs':ext,'nodeFiles':node,'pythonModules':py}
before=capture();(receipts/'before.json').write_text(json.dumps(before,sort_keys=True,indent=2)+'\n')
env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1','NODE_OPTIONS':'--max-old-space-size=256','PYTHONPATH':'weebdex-platform/tests/ci','WEEBDEX_PUBLIC_UPLOAD_BASELINE_SOURCE':str(receipts/'baseline-5c9-browser.mjs'),'WEEBDEX_KEY_IMPORT_DRIVER_BASELINE_SOURCE':str(receipts/'baseline-d6-driver.mjs'),'WEEBDEX_KEY_IMPORT_LEDGER_BASELINE_SOURCE':str(receipts/'baseline-aca-driver.mjs'),'WEEBDEX_UPLOAD_OBSERVER_BASELINE_SOURCE':str(receipts/'baseline-959-observer.mjs'),'WEEBDEX_BITS_DIALOG_SOURCE_DIR':str(receipts/'official-bits-ui-2.16.5'),'WEEBDEX_UPLOAD_GROUP_BASELINE_SOURCE':str(receipts/'baseline-17d-browser.mjs')}
commands={'node':['timeout','45s','node','--experimental-vm-modules','--test','--test-concurrency=2',*node],'python':['timeout','45s',sys.executable,'-B','-m','unittest','-v',*py],'actor':['timeout','10s','node','--test','--test-name-pattern=current actor-follow runtime source','weebdex-platform/tests/ci/test_browser_actor_follow_journeys.mjs']}
(receipts/'commands.json').write_text(json.dumps(commands,indent=2)+'\n')
for name,argv in commands.items():
 with (receipts/(name+'.log')).open('wb') as log:r=subprocess.run(argv,env=env,stdout=log,stderr=subprocess.STDOUT)
 (receipts/(name+'.exit')).write_text(str(r.returncode)+'\n');print(name,r.returncode,flush=True)
 if r.returncode:raise SystemExit(r.returncode)
after=capture();assert after==before;(receipts/'after.json').write_text(json.dumps(after,sort_keys=True,indent=2)+'\n')
spec=importlib.util.spec_from_file_location('prep',root/'weebdex-platform/scripts/ci/prepare-hosted-browser.py');prep=importlib.util.module_from_spec(spec);spec.loader.exec_module(prep);prep.verify_frontend();snapshot=prep.source_snapshot()
(receipts/'build-source-snapshot.json').write_text(json.dumps({'pins':snapshot,'sha256':sha(json.dumps(snapshot,sort_keys=True).encode()),'originalFrontendVerified':True},indent=2,sort_keys=True)+'\n')
assert not subprocess.check_output(['git','status','--porcelain'])
print('pins',len(keys),'build',len(snapshot),'sha256',sha(json.dumps(snapshot,sort_keys=True).encode()),flush=True)
