import pathlib,json,hashlib,os,subprocess,sys
root=pathlib.Path.cwd();out=root/'weebdex-platform/test-evidence/frontend/final-discovered-ci-20261018';phase=sys.argv[1]
roots=[pathlib.Path('/workspace/shared/nostr-manga-recovery-20261017/replacement-frontend-build-worktree/weebdex-platform/.build/frontend-restore3/node_modules'),pathlib.Path('/workspace/shared/nostr-manga-recovery-20261017/account-library-worktree/weebdex-platform/.build/browser-dom-dependencies/node_modules'),pathlib.Path('/workspace/shared/nostr-manga-recovery-20261017/account-library-worktree/weebdex-platform/.build/local-library-noble-2.0.1/node_modules'),pathlib.Path('/workspace/shared/nostr-manga-recovery-20261017/account-library-worktree/weebdex-platform/.build/local-signing-base-2.0.0/node_modules'),pathlib.Path('/workspace/shared/nostr-manga-recovery-20261017/frontend-build-worktree/weebdex-platform/.build/local-library-noble-2.0.1/node_modules'),pathlib.Path('/opt/codex/runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright-core')]
result={}
for directory in roots:
 manifest={}
 for parent,dirs,files in os.walk(directory,followlinks=False):
  for name in list(dirs)+files:
   p=pathlib.Path(parent)/name;rel=str(p.relative_to(directory))
   if p.is_symlink():manifest[rel]={'symlink':os.readlink(p)}
   elif p.is_file():manifest[rel]={'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}
 result[str(directory.resolve())]={'entries':len(manifest),'manifestSha256':hashlib.sha256(json.dumps(manifest,sort_keys=True,separators=(',',':')).encode()).hexdigest()}
(out/f'dependency-pins-{phase}.json').write_text(json.dumps(result,indent=2)+'\n')
files=subprocess.check_output(['git','ls-files','-z']).decode().split('\0');prefixes=('weebdex-platform/frontend/','weebdex-platform/platform-integrations/','weebdex-platform/scripts/','weebdex-platform/tests/ci/','weebdex-platform/contracts/')
manifest={p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in files if p.startswith(prefixes) and (root/p).is_file()}
(out/f'frontend-source-pins-{phase}.json').write_text(json.dumps(manifest,indent=2)+'\n');print('dependency roots',len(result),'sourcefiles',len(manifest))
if phase=='after':
 for name in ['dependency-pins','frontend-source-pins']:assert json.loads((out/f'{name}-before.json').read_text())==json.loads((out/f'{name}-after.json').read_text()),name+' changed'
