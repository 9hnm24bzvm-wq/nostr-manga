from pathlib import Path
import gzip,hashlib,json,re
D=Path(__file__).resolve().parent
manifest=json.loads((D/'evidence-manifest.json').read_text())
for name,digest in manifest.items():assert hashlib.sha256((D/name).read_bytes()).hexdigest()==digest,name
receipt=json.loads((D/'final-http-02.json').read_text());log=gzip.decompress((D/'final-http-02.log.gz').read_bytes())
assert hashlib.sha256(log).hexdigest()==receipt['log_sha256'] and receipt['pass'] and receipt['exit_code']==0
assert receipt['source_file_count']==1244 and receipt['source_changed']==[] and receipt['executable_unchanged']
assert receipt['tests']=={'passed':1,'failed':0,'ignored':0,'filtered_out':1,'seconds':1.63}
a=json.loads(re.search(rb'ACCOUNT_INTENT_VERIFIED=(\{[^\n]+\})',log)[1]);e=json.loads(re.search(rb'CHAPTER_REMOVAL_EFFECT_VERIFIED=(\{[^\n]+\})',log)[1])
assert len(a['writes'])==6
assert [r['sameAccountStatus'] for r in a['writes']]==[200,200,202,200,200,201]
assert all(r['actualActor']=='PublicUploadUser1' and r['rebindStatus']==409 and r['rollback'] for r in a['writes'])
assert a['discoveryRebindStatus']==409 and a['retainedSessionOwnerStatus']==404 and a['foreignSessionsUnchanged']
assert e=={'freshExplicitApproval':204,'grantGainLegacyCompatibility':204,'grantGainSignedReview':202,'ordinaryApproval':403,'refusalNativeStateUnchanged':True,'revokedApproval':403}
cleanup=json.loads((D/'cleanup-02.json').read_text())
assert cleanup['postgres_cleanup']=='verified_stopped' and cleanup['closed_port'] and not cleanup['forced']
assert not any(cleanup[k] for k in ['remaining_owned_processes','cleanup_errors','unconfirmed_births','unconfirmed_child_pids'])
assert b'120 passed; 0 failed; 2 ignored' in gzip.decompress((D/'final-units-01.log.gz').read_bytes())
assert 'tests 6' in gzip.decompress((D/'final-component-dom-02.log.gz').read_bytes()).decode()
for gate,warnings in [('on',12),('off',15)]:
 s=json.loads((D/f'strict-{gate}-summary.json').read_text());assert s['status']=='passed' and s['canonical_frontend_unchanged'] and s['canonical_frontend_files']==679 and s['errors']==0 and s['warnings']==warnings
before=json.loads((D/'final-compile-01-source-pins.json').read_text());after=json.loads((D/'final-source-after.json').read_text());assert sorted(p for p in before if before[p]!=after[p])==['weebdex-platform/backend/tests/public_chapter_writes.rs']
baseline=json.loads((D.parent/'account-intent-baseline-20261017/baseline-http-result.json').read_text());assert baseline['capturedAccountInvariantPassed'] is False and baseline['confirmedDriftCases']==7 and len(baseline['observations'])==7
assert all(x['capturedAccount']=='PublicUploadUser1' and x['actualActor']=='PublicUploadUser2' for x in baseline['observations'])
assert b'tests 224' in gzip.decompress((D/'final-finite-01.log.gz').read_bytes())
print(json.dumps({'actualHttpTestCases':1,'capturedAccountSequences':7,'allSixRefusedWritesRollback':True,'explicitChapterEffectsVerified':True,'physicalCleanupVerified':True,'rustUnits':120,'actualCompiledDomCases':6,'genuineBrowser':False}))
