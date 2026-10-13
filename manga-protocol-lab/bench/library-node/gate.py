"""Provisional experiment gate, not a user-approved production SLO."""
import json,sys
from pathlib import Path
p=Path(sys.argv[1]);r=json.loads((p/'load-summary.json').read_text());checks=dict(no_rejections=not r['errors'],all_accepted=r['accepted']==r['attempted'],no_monitor_errors=not r.get('monitor_errors',[]),bounded_tail=r['scheduled_acceptance']['p95_ms']<1000,short_drain=r['drain_s']<2,drained=r['after']['pending']==0,offered_window_kept=r['accepted_elapsed_s']<r['duration_s']+2)
print(json.dumps(dict(experiment=str(p),checks=checks,passed=all(checks.values()))));sys.exit(0 if all(checks.values()) else 1)
