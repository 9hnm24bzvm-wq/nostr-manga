#!/usr/bin/env python3
"""Run only the finite transport seams and retain their exact observation adapter."""
import argparse
import collections
from datetime import datetime, timezone
import hashlib
import importlib.metadata
import json
from pathlib import Path
import platform
import subprocess
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
REPOSITORY = ROOT.parent
observed = {}


# This is the same callback used inline for the historical 18-case receipt. It
# observes each successful fixture's retained raw-request list after teardown;
# only counts/methods are exported, never URLs, bodies, headers or credentials.
class EvidenceResult(unittest.TextTestResult):
 def addSuccess(self,test):
  requests=getattr(test,'requests',[])
  observed[test.id()]={'server_observed_http_requests':len(requests),'server_received_request_body_bytes':sum(len(row['body'])for row in requests),'observed_methods':dict(collections.Counter(row['method']for row in requests))}
  super().addSuccess(test)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to((ROOT / 'test-evidence').resolve()) or output.exists():
        parser.error('A new directory below weebdex-platform/test-evidence is required')
    source_files = ['scripts/integrated_load_budget.py', 'tests/ci/test_integrated_load_budget.py',
                    'scripts/ci/qualify-integrated-load-budget.py', 'docs/integrated-load-transport-budget.md',
                    'docs/mixed-load-readiness-20261018.md']
    source_hashes = {'weebdex-platform/' + name: digest(ROOT / name) for name in source_files}
    observed.clear()
    suite = unittest.defaultTestLoader.discover(str(ROOT / 'tests/ci'), pattern='test_integrated_load_budget.py')
    output.mkdir(parents=True)
    started = time.monotonic()
    with (output / 'seams.log').open('w') as stream:
        result = unittest.TextTestRunner(stream=stream, verbosity=2, resultclass=EvidenceResult).run(suite)
    elapsed = time.monotonic() - started
    unchanged = all(digest(REPOSITORY / name) == value for name, value in source_hashes.items())
    passed = result.wasSuccessful() and not result.skipped and unchanged
    receipt = dict(status='passed-finite-transport-seams-only' if passed else 'failed-finite-transport-seams',
        source_base=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPOSITORY, text=True).strip(),
        source_hashes_unchanged=unchanged, utc=datetime.now(timezone.utc).isoformat(), tests=result.testsRun,
        skipped=len(result.skipped), elapsed_s=round(elapsed, 6), python=platform.python_version(),
        aiohttp=importlib.metadata.version('aiohttp'),
        command='PYTHONDONTWRITEBYTECODE=1 timeout 30s python3 weebdex-platform/scripts/ci/qualify-integrated-load-budget.py --output ' + str(output.relative_to(REPOSITORY)),
        source_sha256=source_hashes, evidence_sha256={'seams.log': digest(output / 'seams.log')},
        server_observed_http_requests=sum(v['server_observed_http_requests'] for v in observed.values()),
        server_received_request_body_bytes=sum(v['server_received_request_body_bytes'] for v in observed.values()),
        controlled_cases=observed, observation_adapter='EvidenceResult.addSuccess in the hashed runner',
        application_load_executed=False, browser_executed=False, postgres_executed=False,
        external_network_executed=False, install_executed=False, capacity_qualified=False,
        notes=['Only finite ephemeral numeric-loopback raw HTTP fixtures were started.',
               'Connection refusal spends an attempt without reaching an HTTP fixture.',
               'Consumed encoded-body credit and parser-prefetched bytes are separate; no NIC/OS-buffer cap is claimed.',
               'Device progress stays unmeasured, zero HTTP, and is never replaced by a backend load lane.',
               'Only installed aiohttp version qualified; original pinned-version compatibility remains unverified.',
               'No scenario scheduler, application load, browser/IndexedDB measurement or production capacity result.'])
    (output / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps({key: receipt[key] for key in ['status', 'tests', 'skipped', 'elapsed_s',
        'server_observed_http_requests', 'server_received_request_body_bytes']}))
    raise SystemExit(0 if passed else 1)


if __name__ == '__main__':
    main()
