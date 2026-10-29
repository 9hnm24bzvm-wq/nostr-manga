#!/usr/bin/env python3
"""Record an explicit reviewed candidate, never qualify tests or alter historical pins."""
from pathlib import Path
import subprocess,json,hashlib,difflib,sys,re,base64
ROOT=Path(__file__).resolve().parents[2]
TREE='a035b00056557f83348db13c6288a680fd35aca8'
OUTPUT='weebdex-platform/docs/fixtures/six-persona-source-association-20261028.json'
HELPER='weebdex-platform/tests/ci/reviewed-six-persona-source-successor.mjs'
if sys.argv[1:]!=['--record-reviewed-source']:raise SystemExit('Requires --record-reviewed-source after source review/freeze')
def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT)
def sha(data):return hashlib.sha256(data).hexdigest()
paths=set(git('diff','--name-only','-z',TREE,'--').decode().split('\0'))|set(git('ls-files','--others','--exclude-standard','-z').decode().split('\0'))
def in_scope(p):
 return p not in (OUTPUT,HELPER) and (p.startswith(('weebdex-platform/platform-integrations/','weebdex-platform/scripts/','weebdex-platform/backend/','weebdex-platform/tests/','manga-protocol-lab/server/src/')) or p in ['manga-protocol-lab/server/Cargo.toml','manga-protocol-lab/server/Cargo.lock']) and '/target/' not in p and Path(p).suffix in ['.rs','.sql','.toml','.lock','.json','.mjs','.cjs','.js','.ts','.svelte','.py','.sh','.md']
rows=[]
for name in sorted(filter(in_scope,paths)):
 file=ROOT/name
 if not file.is_file() or file.is_symlink():raise SystemExit('Nonregular/deleted reviewed source: '+name)
 current=file.read_bytes();probe=subprocess.run(['git','cat-file','-e',TREE+':'+name],cwd=ROOT,stderr=subprocess.DEVNULL)
 previous=git('show',TREE+':'+name) if probe.returncode==0 else None
 if previous==current:continue
 row={'path':name,'current_bytes':len(current),'current_sha256':sha(current),'added':previous is None}
 if previous is not None:
  a,b=previous.splitlines(keepends=True),current.splitlines(keepends=True);positions=[0]
  for line in b:positions.append(positions[-1]+len(line))
  edits=[]
  for op,i,j,k,l in difflib.SequenceMatcher(None,a,b,autojunk=False).get_opcodes():
   if op=='equal':continue
   old=b''.join(a[i:j]);new=b''.join(b[k:l]);edits.append({'start':positions[k],'end':positions[l],'current_sha256':sha(new),'previous_base64':base64.b64encode(old).decode(),'previous_sha256':sha(old)})
  restored=current
  for edit in reversed(edits):restored=restored[:edit['start']]+base64.b64decode(edit['previous_base64'])+restored[edit['end']:]
  if restored!=previous:raise SystemExit('Nonexact reversal: '+name)
  row.update(previous_bytes=len(previous),previous_sha256=sha(previous),byte_reversals=edits)
 rows.append(row)
prior='weebdex-platform/docs/fixtures/card-spinner-source-association-20261028.json'
if git('show',TREE+':'+prior)!=(ROOT/prior).read_bytes():raise SystemExit('Historical source association was altered')
value={'schema':'six-persona-source-association-v1','baseline_tree':TREE,'local_baseline_commit_provenance':'9a70b771bfa8cb1b20f82c926f65956b93c2d5e6','predecessor_association_path':prior,'predecessor_association_sha256':sha((ROOT/prior).read_bytes()),'scope':'Exact reviewed source delta from immutable qualified tree. Embedded byte reversals preserve prior comparison fixtures offline; current behavior is tested separately. No historical qualification pins or exclusions changed.','browser_executed':False,'http_executed':False,'rows':rows}
raw=(json.dumps(value,indent=2,ensure_ascii=False)+'\n').encode();(ROOT/OUTPUT).write_bytes(raw)
helper=ROOT/HELPER;code=helper.read_text();code,n=re.subn(r"export const sixPersonaSourceAssociationSha256='[^']*';","export const sixPersonaSourceAssociationSha256='"+sha(raw)+"';",code)
if n!=1:raise SystemExit('Unknown current association helper pin')
helper.write_text(code);print(json.dumps({'rows':len(rows),'bytes':len(raw),'sha256':sha(raw),'baseline_tree':TREE}))
