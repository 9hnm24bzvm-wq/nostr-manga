"""Read-only historical consumer reproduction with its real producer receipt."""
import json,pathlib,subprocess,types
root=pathlib.Path(__file__).resolve().parents[3]
path='weebdex-platform/scripts/ci/native-feed-diagnostics.py'
source=subprocess.check_output(['git','show','9feba101f8bb1bda487d1df56a5952151d9f5022:'+path],cwd=root,text=True)
contract=types.ModuleType('historical_native_feed')
exec(compile(source,path,'exec'),contract.__dict__)
fixture=json.loads((root/'weebdex-platform/test-evidence/native-account-profile-20261018/public_native_profile/cleanup.json').read_text())
owned={'process_exit_code':0,'deadline_exceeded':False,'terminal_receipt':True,'cleanup_verified':True,'surviving_owned_process_ids':[],'cleanup_signals':[],'prior_or_unrelated_processes_touched':False}
observed={'scenario':'compact-feed505-plus10000-metadata','baseline_outcome':'query-canceled','baseline_ms':5001,'batched_first_page_ms':210,**{key:True for key in contract.FLAGS}}
value=contract.project(owned,observed,fixture)
print(json.dumps({'historicalSource':'9feba101f8bb1bda487d1df56a5952151d9f5022','actualProducerFields':len(fixture),'closedProjection':value},sort_keys=True))
assert value['status']=='passed', 'Historical consumer rejects the genuine15-field producer receipt'
