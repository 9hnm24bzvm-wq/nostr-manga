import hashlib,json,os,pathlib,shutil,signal,subprocess,sys,time,urllib.request
out=pathlib.Path(__file__).resolve().parent;repo=pathlib.Path.cwd();owned=repo/'weebdex-platform/.build/python-migration-approved-20261018-0556';target=owned/'site-packages';temp=owned/'tmp';owned.mkdir();target.mkdir();temp.mkdir()
start=time.monotonic();deadline=start+180;cap=64*1024**2;floor=4*1024**3+128*1024**2;launch=4*1024**3+256*1024**2
assert shutil.disk_usage(repo).free>=launch,'Insufficient launch reserve'
expected={'cbor2':('5.6.5','38886c41bebcd7dca57739439455bce759f1e4c551b511f618b8e9c1295b431b'),'coincurve':('21.0.0','1b04778b75339c6e46deb9ae3bcfc2250fbe48d1324153e4310fc4996e135715')}
published=[]
for name,(version,digest)in expected.items():
 url=f'https://pypi.org/pypi/{name}/{version}/json'
 with urllib.request.urlopen(url,timeout=min(20,max(1,int(deadline-time.monotonic()))))as response:data=json.load(response)
 matches=[row for row in data['urls']if row['digests']['sha256']==digest];assert len(matches)==1,'Published artifact hash mismatch';row=matches[0];assert row['filename'].endswith('-cp312-cp312-manylinux_2_17_x86_64.manylinux2014_x86_64.whl')and row['packagetype']=='bdist_wheel'and not row['yanked'];assert row['url'].startswith('https://files.pythonhosted.org/')
 published.append({'name':name,'version':version,'metadataUrl':url,'filename':row['filename'],'url':row['url'],'sha256':digest,'bytes':row['size'],'yanked':row['yanked']})
(out/'published-wheel-metadata.json').write_text(json.dumps(published,indent=2)+'\n')
# Exact wheel URLs also carry hashes, eliminating any alternative candidate/source build.
requirements=out/'requirements-hashed.txt';requirements.write_text(''.join(f"{row['name']} @ {row['url']} --hash=sha256:{row['sha256']}\n"for row in published))
command=[sys.executable,'-m','pip','--isolated','install','--disable-pip-version-check','--no-cache-dir','--only-binary=:all:','--no-deps','--no-compile','--require-hashes','--target',str(target),'-r',str(requirements),'--index-url','https://pypi.org/simple']
env=dict(os.environ,TMPDIR=str(temp),PYTHONDONTWRITEBYTECODE='1',PIP_CONFIG_FILE=os.devnull);process=subprocess.Popen(command,env=env,start_new_session=True);samples=[];reason=None
while process.poll()is None:
 allocated=sum(p.lstat().st_blocks*512 for p in owned.rglob('*'));sample={'elapsedSeconds':round(time.monotonic()-start,3),'allocatedBytes':allocated,'freeBytes':shutil.disk_usage(repo).free};samples.append(sample)
 if allocated>cap:reason='64MiB owned setup cap'
 elif sample['freeBytes']<floor:reason='4GiB+128MiB live reserve'
 elif time.monotonic()>deadline:reason='180s setup deadline'
 if reason:
  os.killpg(process.pid,signal.SIGTERM)
  try:process.wait(timeout=3)
  except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.wait()
  break
 time.sleep(.05)
code=process.wait();allocated=sum(p.lstat().st_blocks*512 for p in owned.rglob('*'));assert allocated<=cap
receipt={'status':'passed'if code==0and reason is None else'failed','command':command,'python':sys.version,'exitCode':code,'stopReason':reason,'elapsedSeconds':round(time.monotonic()-start,3),'maximumOwnedAllocatedBytes':max([allocated]+[s['allocatedBytes']for s in samples]),'allocatedBytes':allocated,'capBytes':cap,'deadlineSeconds':180,'minimumFreeBytes':min([shutil.disk_usage(repo).free]+[s['freeBytes']for s in samples]),'target':str(target),'tmp':str(temp),'publishedMetadataReverified':True,'sourceBuildsAllowed':False,'dependenciesInstalled':False,'cacheUsed':False,'globalInstall':False,'samples':samples};(out/'install-resource-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps({k:v for k,v in receipt.items()if k!='samples'}));sys.exit(code or bool(reason))
