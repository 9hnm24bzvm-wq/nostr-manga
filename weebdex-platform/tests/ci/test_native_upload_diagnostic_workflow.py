import copy
import ast
import itertools
import hashlib
import yaml
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('native_owner', ROOT / 'weebdex-platform/scripts/ci/native-upload-diagnostic-owner.py')
owner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(owner)


def evaluate_job_gate(expression, values):
    """Evaluate the parsed actual expression using a closed test-only AST."""
    for index, field in enumerate(sorted(values, key=len, reverse=True)):
        expression = expression.replace(field, 'value_' + str(index))
    expression = expression.replace('&&', ' and ').replace('||', ' or ').replace('!', ' not ')
    expression = expression.replace('false', 'False').replace('true', 'True')
    tree = ast.parse(expression.strip(), mode='eval')
    context = {'value_' + str(index): values[field] for index, field in enumerate(sorted(values, key=len, reverse=True))}
    allowed = (ast.Expression, ast.BoolOp, ast.And, ast.Or, ast.UnaryOp, ast.Not,
               ast.Compare, ast.Eq, ast.Name, ast.Load, ast.Constant)
    if any(not isinstance(node, allowed) or isinstance(node, ast.Name) and node.id not in context
           for node in ast.walk(tree)):
        raise AssertionError('Unexpected job-gate syntax')
    result = eval(compile(tree, '<actual-workflow-gate>', 'eval'), {'__builtins__': {}}, context)
    if type(result) is not bool:
        raise AssertionError('Job gate must be boolean')
    return result


class NativeDiagnosticWorkflow(unittest.TestCase):
    def setUp(self):
        self.sha = 'a' * 40
        self.environment = {'GITHUB_EVENT_NAME': 'push', 'GITHUB_REPOSITORY': '9hnm24bzvm-wq/nostr-manga', 'GITHUB_REF': 'refs/heads/ci/browser-failure-diagnostics', 'GITHUB_SHA': self.sha}
        self.event = {'before': owner.PARENT, 'after': self.sha, 'head_commit': {'message': owner.MARKER}, 'repository': {'private': False}, 'deleted': False, 'forced': False}
        self.paths = set(owner.ALLOWED)
        self.parent = owner.PARENT
        self.dirty = ''

    def git(self, *args):
        return {'rev-parse': self.sha, 'rev-list': self.sha + ' ' + self.parent, 'diff-tree': '\n'.join(sorted(self.paths)), 'status': self.dirty}[args[0]]

    def test_exact_one_time_scope_admitted(self):
        self.assertEqual(owner.PARENT, '1b35d7a6a1e5247931b23183f9c8368483d9d511')
        self.assertEqual(owner.MARKER, 'diagnostic(native-upload): approved bounded pipe run 20261022')
        self.assertEqual(len(owner.ALLOWED), 12)
        self.assertTrue(owner.preflight(self.event, self.environment, self.git))

    def test_unrelated_missing_and_extra_paths_refused(self):
        for paths in [self.paths | {'production.ts'}, self.paths | {'unapproved-workflow.yml'}, self.paths - {'weebdex-platform/scripts/ci/native-upload-diagnostic-owner.py'}, set()]:
            with self.subTest(paths=len(paths)):
                original = self.paths; self.paths = paths
                with self.assertRaises(AssertionError): owner.preflight(self.event, self.environment, self.git)
                self.paths = original

    def test_remote_identity_force_and_dispatch_refused(self):
        for field, value in [('before', 'b' * 40), ('after', 'b' * 40), ('deleted', True), ('forced', True)]:
            event = copy.deepcopy(self.event); event[field] = value
            with self.assertRaises(AssertionError): owner.preflight(event, self.environment, self.git)
        for field, value in [('GITHUB_EVENT_NAME', 'workflow_dispatch'), ('GITHUB_REPOSITORY', 'other/repo'), ('GITHUB_REF', 'refs/heads/main')]:
            env = {**self.environment, field: value}
            with self.assertRaises(AssertionError): owner.preflight(self.event, env, self.git)
        for message in [owner.MARKER + '\n', owner.MARKER + ' extra', 'ordinary change']:
            event = copy.deepcopy(self.event); event['head_commit']['message'] = message
            with self.assertRaises(AssertionError): owner.preflight(event, self.environment, self.git)

    def test_parent_and_dirty_checkout_refused(self):
        self.parent = 'b' * 40
        with self.assertRaises(AssertionError): owner.preflight(self.event, self.environment, self.git)
        self.parent = owner.PARENT; self.dirty = ' M production.ts'
        with self.assertRaises(AssertionError): owner.preflight(self.event, self.environment, self.git)

    def test_full_yaml_parse_and_exact_published_failure_counterfactual(self):
        source = (ROOT / '.github/workflows/hosted-browser-e2e.yml').read_text()
        self.assertEqual(hashlib.sha256(source.encode()).hexdigest(), '29c61963198b921a2e22419cce67d38a4ebd9c868012701bfff2b0bb87dff994')
        document = yaml.safe_load(source)
        triggers = document.get('on', document.get(True))
        self.assertEqual(triggers['push']['branches'], ['ci/hosted-browser-e2e', 'ci/browser-failure-diagnostics'])
        batch = triggers['workflow_dispatch']['inputs']['batch']
        self.assertEqual(batch['default'], 'public-upload')
        self.assertEqual(batch['type'], 'choice')
        self.assertEqual(batch['description'], 'Bounded batch, or all 40 phases with separate native and public profiles')
        self.assertEqual(batch['options'], ['all', 'startup', 'ordinary-a', 'ordinary-b', 'ui10', 'signed-a', 'signed-b', 'signed-c', 'signed-d', 'ui-feed', 'ui-reader', 'ui-home', 'actor-follow', 'public-upload'])
        self.assertEqual(set(document['jobs']), {'browser', 'native-upload-diagnostic'})
        for job in document['jobs'].values():
            self.assertIsInstance(job['if'], str)
            self.assertIn(owner.MARKER, job['if'])
            self.assertNotIn('\n', job['if'])
        self.assertEqual(document['jobs']['browser']['timeout-minutes'], 140)
        self.assertEqual(document['jobs']['native-upload-diagnostic']['timeout-minutes'], 15)
        self.assertIn("&& !(github.event_name == 'push'", document['jobs']['browser']['if'])
        self.assertTrue(document['jobs']['native-upload-diagnostic']['if'].startswith("github.event_name == 'push' &&"))
        self.assertEqual(source.count('Run six bounded pipe-ownership comparisons with normal Chromium sandbox'), 1)
        published = (ROOT / 'weebdex-platform/tests/ci/fixtures/hosted-browser-workflow-9a46af6a61e4.yml.txt').read_bytes()
        self.assertEqual(hashlib.sha256(published).hexdigest(), '9a46af6a61e41b6ca8d82149648ba784c1d58add1b4da4b6550cbdf770336d8b')
        with self.assertRaises(yaml.YAMLError):
            yaml.safe_load(published)

    def test_single_existing_workflow_mutually_exclusive_jobs(self):
        source = (ROOT / '.github/workflows/hosted-browser-e2e.yml').read_text()
        group = "(github.event.head_commit.message == 'diagnostic(native-upload): approved bounded pipe run 20261022' || github.event.head_commit.message == 'diagnostic(native-upload): reviewed browser counterfactual 20261022' || github.event.head_commit.message == 'diagnostic(native-upload): reviewed introspection gate 20261022')"
        self.assertEqual(source.count(group), 1)
        self.assertEqual(source.count('  native-upload-diagnostic:\n'), 1)
        original, diagnostic = source.split('  native-upload-diagnostic:\n')
        self.assertIn('timeout-minutes: 140', original)
        self.assertIn('timeout-minutes: 15', diagnostic)
        self.assertNotIn('workflow_dispatch', diagnostic)
        self.assertIn("github.event_name == 'push'", diagnostic)
        self.assertIn("github.event.head_commit.message == '" + owner.MARKER + "'", diagnostic)
        self.assertEqual(sorted(p.name for p in (ROOT / '.github/workflows').glob('*.yml')), ['hosted-browser-e2e.yml', 'hosted-sandbox-preflight.yml', 'native-upload-browser-counterfactual.yml'])
        preflight = (ROOT / '.github/workflows/hosted-sandbox-preflight.yml').read_text()
        self.assertIn('branches: [ci/browser-sandbox-preflight]', preflight)

    def test_actual_parsed_gates_are_mutually_exclusive_for_every_supported_context(self):
        jobs = yaml.safe_load((ROOT / '.github/workflows/hosted-browser-e2e.yml').read_text())['jobs']
        counter = yaml.safe_load((ROOT / '.github/workflows/native-upload-browser-counterfactual.yml').read_text())['jobs']['counterfactual']
        repository = '9hnm24bzvm-wq/nostr-manga'
        diagnostic_ref = 'refs/heads/ci/browser-failure-diagnostics'
        application_ref = 'refs/heads/ci/hosted-browser-e2e'
        approved = 'diagnostic(native-upload): reviewed introspection gate 20261022'
        retired = 'diagnostic(native-upload): reviewed browser counterfactual 20261022'
        markers = [owner.MARKER, approved, retired]
        for event, private, repo, ref, message, same_head in itertools.product(
                ['push', 'workflow_dispatch', 'pull_request'], [False, True], [repository, 'other/repo'],
                [diagnostic_ref, application_ref, 'refs/heads/ci/browser-sandbox-preflight', 'refs/heads/main'],
                markers + [owner.MARKER + ' extra', approved + ' extra', 'ordinary change'], [False, True]):
            values = {'github.event_name': event, 'github.event.repository.private': private,
                      'github.repository': repo, 'github.ref': ref, 'github.event.head_commit.message': message,
                      'github.sha': 'a' * 40, 'github.event.head_commit.id': ('a' if same_head else 'b') * 40}
            with self.subTest(values=values):
                actual = [evaluate_job_gate(jobs['browser']['if'], values), evaluate_job_gate(jobs['native-upload-diagnostic']['if'], values), evaluate_job_gate(counter['if'], values)]
                allowed = not private and repo == repository
                diagnostic_push = event == 'push' and ref == diagnostic_ref
                expected = [allowed and ref in [diagnostic_ref, application_ref] and not (diagnostic_push and message in markers), allowed and diagnostic_push and message == owner.MARKER, allowed and diagnostic_push and same_head and message == approved]
                self.assertEqual(actual, expected)
                self.assertLessEqual(sum(actual), 1)

    def test_legacy_owner_does_not_admit_current_public_parent(self):
        event = copy.deepcopy(self.event)
        event['before'] = '92c15ddfbcab8dcb88d8a39fc0bc139c6cae20fc'
        self.parent = event['before']
        with self.assertRaises(AssertionError):
            owner.preflight(event, self.environment, self.git)

    def test_report_dependencies_caps_and_secret_boundary(self):
        source = (ROOT / '.github/workflows/hosted-browser-e2e.yml').read_text().split('  native-upload-diagnostic:\n')[1]
        self.assertLess(source.index('owner.py preflight'), source.index('npm ci'))
        self.assertIn('npm ci --ignore-scripts --registry=https://registry.npmjs.org --no-audit --no-fund --omit=optional --prefix .ci/browser', source)
        self.assertIn('node-version: 24.19.0', source)
        self.assertIn('persist-credentials: false', source)
        self.assertIn('fetch-depth: 2', source)
        self.assertEqual(source.count('uses: actions/upload-artifact@'), 1)
        self.assertIn('retention-days: 1', source)
        self.assertIn('path: weebdex-platform/ci-native-upload-evidence/report.json', source)
        self.assertNotIn('secrets.', source)
        self.assertNotIn('--no-sandbox', source)
        self.assertNotIn('sysctl', source)
        self.assertNotIn('sudo', source)
        self.assertIn('timeout-minutes: 5', source)

    def test_owned_process_normal_and_deadline_cleanup(self):
        for code, limit, expected in [('pass', 2, 0), ('import time; time.sleep(2)', .1, 1), ("import subprocess,sys; subprocess.Popen([sys.executable,'-c','import time; time.sleep(2)'],start_new_session=True)", 2, 1)]:
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory)
                result = owner.run_owned([sys.executable, '-c', code], path, path / 'private.log', path / 'owner.json', {'PATH': os.environ['PATH']}, deadline=limit)
                self.assertEqual(result, expected)
                receipt = json.loads((path / 'owner.json').read_text())
                self.assertTrue(receipt['terminal'])
                self.assertTrue(receipt['cleanup_verified'])
                self.assertEqual(receipt['remaining_owned_process_count'], 0)
                self.assertEqual(receipt['forced'], bool(expected))


if __name__ == '__main__': unittest.main()
