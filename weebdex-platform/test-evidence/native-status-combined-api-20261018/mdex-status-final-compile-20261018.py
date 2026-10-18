from pathlib import Path
import subprocess,os,time,json,hashlib,shutil,signal
R=Path('/workspace/shared/nostr-manga-recovery-20261017');W=R/'mdex-validation-worktree';BASE='c96ead07654d38ead63bab9e6172328b79811ec9'
LOG=R/'mdex-status-final-compile-20261018.log';OUT=R/'mdex-status-final-compile-20261018.json'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=W,text=True).strip()==BASE
assert not subprocess.check_output(['git','status','--porcelain'],cwd=W,text=True).strip()
extra=['weebdex-platform/docs/mangadex-operation-compatibility-snapshot.json','weebdex-platform/tests/ci/probe_public_relation_pages.mjs']+['weebdex-platform/platform-integrations/public-api/'+p for p in ['client.ts','validation.ts','catalogue-edges.ts','normalize.ts']]
def pins():
 files=subprocess.check_output(['git','ls-files'],cwd=W,text=True).splitlines()
 return {p:hashlib.sha256((W/p).read_bytes()).hexdigest() for p in files if (p.startswith('weebdex-platform/backend/') or p.startswith('manga-protocol-lab/') or p in extra) and (W/p).is_file()}
assert not OUT.exists();before=pins();free=shutil.disk_usage(R).free;floor=4*1024**3;assert free>=floor+128*1024**2,('disk preflight',free)
env=os.environ.copy();env.update(CARGO_HOME=str(R/'toolchains/cargo'),RUSTUP_HOME=str(R/'toolchains/rustup'),CARGO_TARGET_DIR=str(R/'mdex-api-cargo-target'),CARGO_INCREMENTAL='0');env['PATH']=env['CARGO_HOME']+'/bin:'+env['PATH'];assert not any(env.get(k) for k in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_PROFILE_TEST_DEBUG','CARGO_PROFILE_DEV_DEBUG'])
cmd=['cargo','test','--locked','--offline','--manifest-path','weebdex-platform/backend/Cargo.toml','-j1','--no-run','--lib']
for name in ['public_account_reads','public_catalogue_compatibility','public_catalogue_reads','public_statistics','public_list_follows','public_nostr_lists','library_http','public_native_reading_status']:cmd+=['--test',name]
start=time.monotonic();minimum=free;limited=False
with LOG.open('wb') as f:
 p=subprocess.Popen(cmd,cwd=W,env=env,stdout=f,stderr=subprocess.STDOUT,start_new_session=True)
 while p.poll() is None:
  minimum=min(minimum,shutil.disk_usage(R).free)
  if minimum<floor:limited=True;os.killpg(p.pid,signal.SIGINT);break
  time.sleep(.5)
 code=p.wait()
result={'source_baseline':BASE,'command':cmd,'returncode':code,'elapsed_seconds':time.monotonic()-start,'minimum_free_bytes':minimum,'floor_interrupted':limited,'source_pins_unchanged':before==pins(),'source_sha256':before,'log_sha256':hashlib.sha256(LOG.read_bytes()).hexdigest()};OUT.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k!='source_sha256'},indent=2));assert code==0 and result['source_pins_unchanged'] and not limited
