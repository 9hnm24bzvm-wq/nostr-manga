from pathlib import Path
import gzip,hashlib,json,re
p=Path(__file__).parent
report=json.loads((p/'baseline-http-result.json').read_text())
raw=gzip.decompress((p/'baseline-http.log.gz').read_bytes())
assert hashlib.sha256(raw).hexdigest()==report['logSha256']
observations=json.loads(re.search(rb'ACCOUNT_INTENT_LEGACY_REPRO=(\[.*\])',raw)[1])
assert observations==report['observations'] and len(observations)==7
violations=[row for row in observations if row['capturedAccount']!=row['actualActor']]
assert not violations, f'Captured-account invariant failed: {len(violations)} of 7 actual signed HTTP sequences executed as another eligible native account'
