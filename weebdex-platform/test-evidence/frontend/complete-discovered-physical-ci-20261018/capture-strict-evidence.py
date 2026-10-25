import json,hashlib,pathlib,shutil
out=pathlib.Path(__file__).resolve().parent;platform=out.parents[2]
for gate in [0,1]:
 generation=platform/'.build'/f'complete-discovered-strict-physical-{gate}-20261018-0335'
 for name in ['summary.json','diagnostics.json','cached-toolchain.json','service-worker-tsc-receipt.json','svelte-check-receipt.json','svelte-kit-sync-receipt.json']:
  source=generation/'evidence'/name
  if source.exists():shutil.copyfile(source,out/f'strict-{gate}-{name}')
records=[]
for file in sorted((out/'cache-audit/strict-physical').glob('*.json')):
 raw=file.read_bytes();row=json.loads(raw);records.append(dict(file=str(file.relative_to(out)),sha256=hashlib.sha256(raw).hexdigest(),pid=row['pid'],roots=len(row['protected_roots']),checkedMutations=row['checked_node_mutations']))
(out/'cache-audit-summary-physical.json').write_text(json.dumps({'recordsManifest':records,'protectedWriteAttempts':sum(json.loads((out/r['file']).read_text())['protected_write_attempts']for r in records)},indent=2)+'\n')
print('strict audit records',len(records))
