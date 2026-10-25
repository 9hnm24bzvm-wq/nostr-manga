"""Approved four-package, no-script extraction; no server or global changes."""
import hashlib,json,os,subprocess,sys,tarfile,urllib.request,urllib.parse,ssl
from pathlib import Path,PurePosixPath
D=Path(__file__).resolve().parent
fd=int(os.environ.pop('OWNER_GATE_FD'))
try:admitted=os.read(fd,1)==b'G'
finally:os.close(fd)
if not admitted:os._exit(73)
os.umask(0o077)
assert hashlib.sha256((D/'proposal.json').read_bytes()).hexdigest()=='eeaa90c2d4c6987c32ac40d39ace8246ed84e14d38d7f1358e1a9495e9580353'
plan=json.loads((D/'proposal.json').read_text());assert len(plan['packages'])==4
root=D/'root';root.mkdir(mode=0o700,exist_ok=False)
archives=D/'archives';archives.mkdir(mode=0o700,exist_ok=False)
report={'status':'running','downloaded':[],'archives':[],'tools':{},'noMaintainerScripts':True,'noGlobalInstall':True,'noServerStarted':True}

def save(): (D/'setup-result.json').write_text(json.dumps(report,sort_keys=True,indent=2)+'\n')
def command(argv,**kwargs):
 value=subprocess.run(argv,stdin=subprocess.DEVNULL,capture_output=True,text=True,timeout=30,**kwargs)
 assert value.returncode==0, 'bounded tool refused: '+Path(argv[0]).name
 return value.stdout.strip()
class NoRedirect(urllib.request.HTTPRedirectHandler):
 def redirect_request(self,*args,**kwargs):raise RuntimeError('unapproved package redirect')
# Use only the already configured normal HTTPS proxy. No fallback may change
# the destination; TLS hostname/certificate checks stay on.
https_proxy=urllib.request.getproxies().get('https');assert https_proxy,'normal HTTPS proxy is unavailable'
proxy=urllib.parse.urlsplit(https_proxy)
assert proxy.scheme=='http' and proxy.hostname=='127.0.0.1' and 1024<proxy.port<65536 and proxy.username is None and proxy.password is None and not proxy.query and not proxy.fragment and proxy.path in ('','/'),'normal HTTPS proxy changed'
assert hashlib.sha256(https_proxy.encode()).hexdigest()==os.environ.pop('PG18_EXPECTED_HTTPS_PROXY_SHA256'),'normal owner/child HTTPS proxy mismatch'
opener=urllib.request.build_opener(urllib.request.ProxyHandler({'https':https_proxy}),NoRedirect(),urllib.request.HTTPSHandler(context=ssl.create_default_context()))
def safe_path(value):
 p=PurePosixPath(value);assert not p.is_absolute() and '..' not in p.parts,'unsafe archive path'
 return p
try:
 for item in plan['packages']:
  dest=archives/Path(item['Filename']).name;remaining=int(item['Size']);digest=hashlib.sha256()
  assert item['URL'].startswith(('https://apt.postgresql.org/pub/repos/apt/pool/main/p/postgresql-18/','https://deb.debian.org/debian/pool/main/libu/liburing/'))
  assert not urllib.request.proxy_bypass(urllib.parse.urlsplit(item['URL']).hostname),'normal proxy route would be bypassed'
  with opener.open(item['URL'],timeout=25) as response,dest.open('xb') as output:
   assert response.geturl()==item['URL']
   if response.headers.get('Content-Length') is not None:assert int(response.headers['Content-Length'])==int(item['Size'])
   while True:
    chunk=response.read(min(65536,remaining+1))
    if not chunk:break
    remaining-=len(chunk);assert remaining>=0,'package exceeds reviewed exact size';digest.update(chunk);output.write(chunk)
  assert remaining==0 and digest.hexdigest()==item['SHA256'],'package hash/size changed'
  report['downloaded'].append({'package':item['Package'],'version':item['Version'],'bytes':int(item['Size']),'sha256':digest.hexdigest()});save()
  # List data only; dpkg-deb never executes package control scripts.
  inspect=subprocess.Popen(['/usr/bin/dpkg-deb','--fsys-tarfile',str(dest)],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  count=total=0;declared={}
  try:
   with tarfile.open(fileobj=inspect.stdout,mode='r|*') as members:
    for member in members:
     name=safe_path(member.name);key=str(name);assert key not in declared,'duplicate archive path';declared[key]=member
     assert member.isdir() or member.isfile() or member.issym() or member.islnk(),'special archive member'
     assert not member.mode&0o6000,'unexpected privileged archive mode'
     if member.issym() or member.islnk():
      link=PurePosixPath(member.linkname);assert not link.is_absolute(),'absolute archive link'
      resolved=(root/str(name.parent if member.issym() else PurePosixPath('.'))/str(link)).resolve()
      assert resolved.is_relative_to(root),'archive link escapes isolated root'
     count+=1;total+=member.size;assert count<20000 and total<64*1024**2,'archive budget'
   inspect.stdout.close();errors=inspect.stderr.read(4096);assert inspect.wait(timeout=5)==0,'archive inspection failed'
  finally:
   if inspect.poll() is None:inspect.terminate();inspect.wait(timeout=5)
  for key,member in declared.items():
   name=PurePosixPath(key)
   for ancestor in name.parents:
    prior=declared.get(str(ancestor));assert prior is None or prior.isdir(),'member ancestor is not a directory'
    existing=root/str(ancestor);assert not existing.is_symlink() and (not existing.exists() or existing.is_dir()),'existing member ancestor is not a directory'
   existing=root/key
   assert not existing.is_symlink() and (not existing.exists() or member.isdir() and existing.is_dir()),'package member conflicts with existing extraction'
  command(['/usr/bin/dpkg-deb','-x',str(dest),str(root)])
  report['archives'].append({'package':item['Package'],'members':count,'declaredPayloadBytes':total});save()
 bin=root/'usr/lib/postgresql/18/bin';env={k:v for k,v in os.environ.items() if not k.startswith('PG') and k not in {'LD_LIBRARY_PATH','LD_PRELOAD'}};env.update(LD_LIBRARY_PATH=str(root/'usr/lib/x86_64-linux-gnu'),LC_ALL='C')
 for name in ['postgres','initdb','psql','pg_dump','pg_restore','pg_ctl','createdb']:
  p=bin/name;assert p.is_file() and not p.is_symlink()
  needed=command(['/usr/bin/readelf','-d',str(p)],env=env)
  libraries=command(['/usr/bin/ldd',str(p)],env=env);assert 'not found' not in libraries,'missing runtime dependency'
  version=command([str(p),'--version'],env=env);assert '18.6' in version,'wrong approved PostgreSQL version'
  report['tools'][name]={'version':version,'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'elfNeeded':[v.strip() for v in needed.splitlines() if '(NEEDED)' in v],'resolvedLibraries':libraries.splitlines()}
 inventory=[]
 for p in sorted(root.rglob('*')):
  st=p.lstat()
  if p.is_symlink():assert p.resolve().is_relative_to(root);inventory.append({'path':str(p.relative_to(root)),'symlink':os.readlink(p)})
  elif p.is_file():inventory.append({'path':str(p.relative_to(root)),'bytes':st.st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()})
  else:assert p.is_dir(),'unexpected extracted type'
 (D/'installed-inventory.json').write_text(json.dumps(inventory,sort_keys=True,indent=2)+'\n')
 report.update(status='passed',installedInventorySha256=hashlib.sha256((D/'installed-inventory.json').read_bytes()).hexdigest(),installedEntries=len(inventory),totalPackageBytes=sum(v['bytes'] for v in report['downloaded']))
except Exception as error:report.update(status='failed',errorClass=type(error).__name__,error=str(error));raise
finally:save()
