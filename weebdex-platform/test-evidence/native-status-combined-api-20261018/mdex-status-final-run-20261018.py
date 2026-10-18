from pathlib import Path
import os,re,json,hashlib,signal,subprocess,time,sys
R=Path('/workspace/shared/nostr-manga-recovery-20261017');W=R/'mdex-validation-worktree';compile=(R/'mdex-status-final-compile-20261018.log').read_text()
allowed=['lib','public_account_reads','public_catalogue_compatibility','public_catalogue_reads','public_statistics','public_list_follows','public_nostr_lists','library_http','public_native_reading_status']
selected=sys.argv[1:];assert selected and all(s in allowed for s in selected)
free=lambda:os.statvfs(R).f_bavail*os.statvfs(R).f_frsize
for suite in selected:
 assert free()>=4*1024**3+128*1024**2,('disk preflight',free())
 pattern=r'Executable unittests src/lib.rs \(([^)]+)\)' if suite=='lib' else r'Executable tests/'+suite+r'\.rs \(([^)]+)\)'
 matches=re.findall(pattern,compile);assert len(matches)==1,matches
 exe=Path(matches[0]);assert exe.parent==R/'mdex-api-cargo-target/debug/deps' and exe.is_file()
 env=os.environ.copy();env['PLATFORM_TEST_PG_BIN']=str(R/'toolchains/debian-pg/root/usr/lib/postgresql/17/bin');env['LD_LIBRARY_PATH']=str(R/'toolchains/debian-pg/root/usr/lib/x86_64-linux-gnu');env['MANGA_TEST_STRFRY_BINARY']=str(R/'upload-replacement-worktree/manga-protocol-lab/vendor/strfry/strfry')
 prefix=R/('mdex-status-final-'+suite+'-20261018');log=prefix.with_suffix('.log');receipt=prefix.with_suffix('.json');assert not receipt.exists(),receipt
 before={p.name for p in (W/'weebdex-platform/test-runtime').glob('auth-*')};start=time.monotonic();minimum=free();interrupted=False
 command=[str(exe),'--test-threads=1','--nocapture'] if suite=='lib' else ['bash',str(W/'weebdex-platform/scripts/run-auth-tests.sh'),str(exe),'--ignored','--test-threads=1','--nocapture']
 with log.open('wb') as output:
  child=subprocess.Popen(command,cwd=W,env=env,stdout=output,stderr=subprocess.STDOUT)
  while child.poll() is None:
   minimum=min(minimum,free())
   if minimum<4*1024**3 and not interrupted:child.send_signal(signal.SIGINT);interrupted=True
   time.sleep(.25)
  code=child.wait()
 fixtures=[p for p in (W/'weebdex-platform/test-runtime').glob('auth-*') if p.name not in before];cleanup=None
 if suite!='lib':
  assert len(fixtures)==1,fixtures
  cleanup=json.loads((fixtures[0]/'cleanup.json').read_text())
  assert cleanup['postgres_cleanup']=='verified_stopped' and cleanup['closed_port'] and not cleanup['forced'] and not cleanup['remaining_owned_processes'] and not cleanup['cleanup_errors'],cleanup
 summary={'suite':suite,'utc':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),'source_baseline':'c96ead07654d38ead63bab9e6172328b79811ec9','command':command,'returncode':code,'elapsed_seconds':time.monotonic()-start,'minimum_free_bytes':minimum,'free_bytes_after':free(),'interrupted_at_disk_floor':interrupted,'binary_path':str(exe),'binary_sha256':hashlib.sha256(exe.read_bytes()).hexdigest(),'output_sha256':hashlib.sha256(log.read_bytes()).hexdigest(),'result':re.findall(r'test result:.*',log.read_text()),'fixture':str(fixtures[0]) if fixtures else None,'cleanup':{k:cleanup.get(k) for k in ('postgres_cleanup','closed_port','forced','remaining_owned_processes','cleanup_errors','test_returncode')} if cleanup else None}
 receipt.write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary,indent=2),flush=True)
 if code or interrupted:print(log.read_text()[-10000:]);sys.exit(code or 1)
