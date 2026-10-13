"""Summarize opt-in sampled server stages; never silently mix setup with traffic."""
import collections,json,statistics,sys
from pathlib import Path
p=Path(sys.argv[1]);summary=json.loads((p/'load-summary.json').read_text());offset=summary['before'].get('profile_log_line_offset');includes_setup=offset is None
if includes_setup and '--include-setup' not in sys.argv:raise SystemExit('No recorded setup boundary; pass --include-setup and retain that qualification')
values=collections.defaultdict(list)
for line in (p/'server.log').read_text().splitlines()[offset or 0:]:
 if line.startswith('LIBRARY_PERF '):
  item=json.loads(line.removeprefix('LIBRARY_PERF '))
  for stage,ms in item['stages_ms']:values[item['operation']+'.'+stage].append(ms)
stages={k:dict(n=len(v),median_ms=statistics.median(v),p95_ms=sorted(v)[min(len(v)-1,int(len(v)*.95))],max_ms=max(v)) for k,v in values.items()}
result=dict(scope='Sampled diagnostic timings with profiling overhead; not a capacity certificate',includes_setup=includes_setup,stages=stages)
(p/'stage-profile.json').write_text(json.dumps(result,indent=2))
for name,stats in sorted(stages.items(),key=lambda kv:kv[1]['median_ms'],reverse=True):print(name,json.dumps(stats))
