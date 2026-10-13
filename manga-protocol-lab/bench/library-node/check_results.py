"""Check finished benchmark artifacts without treating performance as a pass gate."""
import json,sys
from pathlib import Path
p=Path(sys.argv[1]);pg=json.loads((p/'postgres-final.json').read_text());assert pg['fsync']=='on' and pg['synchronous_commit']=='on';assert pg['pending']==0
if (p/'summary.json').exists():
 rows=json.loads((p/'summary.json').read_text());assert len(rows)==4
 for r in rows:
  assert len(r['saves'])==12 and all(x['status'] in (200,202) for x in r['requests']);assert r['selective']['requests']==4
  assert r['restore']['records']=={30000:31712,500000:511202}[r['completed_chapters']]
else:
 r=json.loads((p/'load-summary.json').read_text());assert r['accepted']+len(r['errors'])==r['attempted'];assert r['after']['pending']==0;assert len(r['final_heads'])==r['readers']
print('Artifact consistency checks passed:',p)
