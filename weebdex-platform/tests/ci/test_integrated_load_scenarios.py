"""Finite owned loopback + actual mounted components, not real-browser capacity."""
import asyncio
import os
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
from integrated_load_scenarios import run, validate_config, plan, check_runtime, UUID, request_failure_reason, mounted_failure_reason, synthetic_fixture, bounded_selector
from aiohttp import ClientSession
from integrated_load_budget import TransportDeadline, TransportFailure, BudgetRefused
from unittest.mock import patch


class ConfigTests(unittest.TestCase):
    def test_shared_deadline_reports_truthfully_in_both_timeout_orderings(self):
        shared = TransportDeadline(shared_deadline=True)
        # Inner first: the request records deadline before TaskGroup wraps its stop.
        self.assertEqual(request_failure_reason(shared), 'deadline')
        grouped = ExceptionGroup('owned group', [RuntimeError('shared stop')])
        self.assertEqual(mounted_failure_reason(grouped), 'mounted-run-failed')
        # Outer first: cancellation becomes the owning asyncio timeout directly.
        self.assertEqual(mounted_failure_reason(TimeoutError()), 'deadline')

    def test_earlier_request_timeout_and_other_errors_are_not_shared_deadlines(self):
        self.assertEqual(request_failure_reason(TransportDeadline(shared_deadline=False)), 'TransportFailure')
        self.assertEqual(request_failure_reason(TransportFailure('owned transport failed')), 'TransportFailure')
        self.assertEqual(request_failure_reason(TimeoutError()), 'TimeoutError')
        self.assertEqual(request_failure_reason(BudgetRefused('request-limit')), 'request-limit')
        self.assertEqual(mounted_failure_reason(RuntimeError()), 'mounted-run-failed')

    def test_plan_does_not_claim_rate_conversion_or_network_progress(self):
        result = plan(validate_config({}))
        self.assertIsNone(result['expected_http_per_action'])
        self.assertFalse(result['capacity_qualified'])
        self.assertFalse(result['public_sync']['enabled'])
        self.assertIsNone(result['public_sync']['cadence'])

    def test_missing_runtime_fails_before_fixture_launch(self):
        with patch.dict(os.environ, {}, clear=True), self.assertRaisesRegex(ValueError, 'no install'):
            check_runtime()

    def test_no_target_urls_sync_percent_or_unbounded_work(self):
        for value in [{'origin': 'https://example.com'}, {'public_sync': True}, {'rate': 61},
                      {'duration_s': 181}, {'chapters': 10000}, {'rate': True},
                      {'max_response_body_bytes': -1}, {'selector_pages': 4}, {'selector_pages': True}, {'actions': []}, {'actions': [{}]}]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                validate_config(value)


    def test_bounded_fixture_cursor_binds_scope_and_rejects_unsupported_queries(self):
        chapters = [dict(id=UUID(100 + index), attributes=dict(chapter=str(index + 1))) for index in range(101)]
        query = dict(operation=['groups'], limit=['50'], current=[UUID(100)], **{'translatedLanguage[]': ['en'], 'direction': ['newest']})
        first = bounded_selector(chapters, query)
        second = bounded_selector(chapters, dict(query, after=[first['nextCursor']]))
        third = bounded_selector(chapters, dict(query, after=[second['nextCursor']]))
        self.assertEqual([len(page['data']) for page in [first, second, third]], [50, 50, 1])
        self.assertEqual([row['id'] for page in [first, second, third] for row in page['data']], [UUID(i) for i in range(200, 99, -1)])
        self.assertEqual({page['scope'] for page in [first, second, third]}, {first['scope']})
        self.assertIsNone(third['nextCursor'])
        for changes in [dict(current=[UUID(101)]), dict(limit=['49']), dict(direction=['oldest']),
                        {'translatedLanguage[]': ['ja']}, dict(manga=[UUID(2)]), dict(operation=['around']),
                        dict(after=['invented']), dict(current=[UUID(100), UUID(100)])]:
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                bounded_selector(chapters, {**query, 'after': [first['nextCursor']], **changes})
        around = bounded_selector(chapters, {key: value for key, value in {**query, 'operation': ['around']}.items() if key != 'direction'})
        self.assertEqual([row['id'] for row in around['data']], [UUID(100)])
        self.assertIsNone(around['previous'])
        self.assertEqual(around['next']['id'], UUID(101))
        self.assertIsNone(around['nextCursor'])


class HistoricalFixtureTests(unittest.IsolatedAsyncioTestCase):
    async def test_historical_eager_fixture_still_pages_100_plus_1_not_current_reader(self):
        async with synthetic_fixture(validate_config(dict(chapters=101))) as (origin, observed):
            async with ClientSession() as client:
                pages = []
                for offset in [0, 100]:
                    async with client.get(origin + '/api/v1/chapter', params=dict(manga=UUID(1), limit='100', offset=str(offset))) as response:
                        self.assertEqual(response.status, 200)
                        pages.append(await response.json())
            self.assertEqual([len(page['data']) for page in pages], [100, 1])
            self.assertEqual([page['total'] for page in pages], [101, 101])
            self.assertEqual([row['id'] for page in pages for row in page['data']], [UUID(i) for i in range(100, 201)])
            self.assertEqual(len(observed), 2)


class MountedTests(unittest.IsolatedAsyncioTestCase):
    async def checked(self, config=None, **kwargs):
        # A missing exact runtime is a failed prerequisite, never a successful skip.
        self.assertTrue(os.environ.get('WEEBDEX_LOAD_NODE_MODULES'), 'Explicit pinned runtime is required')
        result = await run(config or {}, **kwargs)
        self.assertEqual(result['cleanup'], dict(child_reaped=True, active_http=0, read_credit=0, listener_closed=True, generated_directory_removed=True))
        self.assertFalse(result['capacity_qualified'])
        return result

    def assert_current_fanout(self, result, *, mounts, capabilities, around, groups, images, total, analytics=1):
        paths = result['fixture']['paths']
        expected = {'/api/protocol/capabilities': capabilities, '/api/v1/manga': mounts,
                    '/api/v1/nostr/native-forum/contexts': mounts,
                    '/api/v1/chapter/' + UUID(100): mounts,
                    '/api/v1/at-home/server/' + UUID(100): mounts,
                    '/api/v1/manga/' + UUID(1) + '/reader': around + groups}
        if analytics:
            expected['/api/v1/analytics/chapter/' + UUID(100) + '/views'] = analytics
        actual_nonmedia = {path: count for path, count in paths.items() if not path.startswith('/data/')}
        self.assertEqual(actual_nonmedia, expected)
        self.assertEqual(sum(count for path, count in paths.items() if path.startswith('/data/')), images)
        self.assertEqual(result['transport']['http_attempts'], total)
        self.assertEqual(result['fixture']['http_requests'], total)
        selectors = result['fixture']['selectors']
        self.assertEqual([row['operation'] for row in selectors].count('around'), around)
        self.assertEqual([row['operation'] for row in selectors].count('groups'), groups)
        self.assertTrue(all(row['limit'] == 50 for row in selectors))
        self.assertTrue(all(row['returned'] == 1 and row['after'] is None and row['next_cursor'] is None
                            for row in selectors if row['operation'] == 'around'))
        self.assertEqual(result['child']['node_diagnostics'], dict(
            accepted={'node-type-stripping': 1, 'node-vm-modules': 1}, provider_scheduled=2,
            provider_completed=2, failures=0, setup_closed=True, listener_cleanup=True))

    async def test_actual_mounted_fanout_and_debounced_local_transaction(self):
        r = await self.checked()
        self.assertTrue(r['passed'], r['failures'])
        self.assert_current_fanout(r, mounts=1, capabilities=2, around=2, groups=0, images=2, total=11)
        self.assertEqual(r['transport']['response_body_bytes_consumed'], r['fixture']['response_body_bytes_prepared'])
        self.assertEqual(r['transport']['upload_body_bytes_reserved'], r['fixture']['request_body_bytes'])
        self.assertEqual(r['child']['progress_saves_completed'], 1)
        self.assertEqual(r['child']['page_positions_selected'], 2)
        self.assertEqual(r['child']['actions'][0]['saved_page_after_unmount'], 3)
        self.assertEqual(r['child']['default_progress_http_writes'], 0)
        self.assertEqual(r['fixture']['methods']['POST'], 2) # One detail forum read, one actual analytics write.
        self.assertEqual(r['child']['image_bodies_verified'], 2)
        self.assertFalse(r['child']['image_decode_measured'])
        self.assertEqual(r['timings']['media']['complete_body_ms']['observations'], 2)
        self.assertIsNotNone(r['timings']['reader']['response_headers_ms']['p95'])

    async def test_actual_101_selector_pagination_and_exit_flush(self):
        r = await self.checked(dict(chapters=101, pages=1, selector_pages=3, actions=[dict(chapter=100, preload=0, page_turns=[], dwell_ms=0)]))
        self.assertTrue(r['passed'], r['failures'])
        self.assert_current_fanout(r, mounts=1, capabilities=2, around=1, groups=3, images=1, total=11, analytics=0)
        pages = r['child']['actions'][0]['selector_pages']
        self.assertEqual([len(page['ids']) for page in pages], [50, 50, 1])
        self.assertEqual([page['more'] for page in pages], [True, True, False])
        ids = [id for page in pages for id in page['ids']]
        self.assertEqual(ids, [UUID(i) for i in range(200, 99, -1)])
        self.assertEqual(len(set(ids)), 101)
        requests = [row for row in r['fixture']['selectors'] if row['operation'] == 'groups']
        self.assertEqual([row['returned'] for row in requests], [50, 50, 1])
        self.assertEqual(len({row['scope'] for row in requests}), 1)
        self.assertIsNone(requests[0]['after'])
        self.assertEqual([row['after'] for row in requests[1:]], [row['next_cursor'] for row in requests[:-1]])
        self.assertIsNone(requests[-1]['next_cursor'])
        self.assertIsNone(r['child']['actions'][0]['saved_page_before_unmount'])
        self.assertEqual(r['child']['progress_saves_completed'], 1)
        self.assertEqual(r['child']['actions'][0]['saved_page_after_unmount'], 1)

    async def test_same_title_remount_rechecks_selector_prefetch_and_analytics_marker(self):
        action = dict(chapter=100, preload=2, page_turns=[], dwell_ms=1150)
        r = await self.checked(dict(actions=[action, action]))
        self.assertTrue(r['passed'], r['failures'])
        self.assertEqual(r['child']['logical_actions_completed'], 2)
        self.assert_current_fanout(r, mounts=2, capabilities=3, around=4, groups=0, images=6, total=22)
        self.assertEqual(r['fixture']['paths']['/api/v1/analytics/chapter/' + UUID(100) + '/views'], 1)
        self.assertEqual(r['child']['image_bodies_verified'], 6)

    async def test_request_cap_cancels_every_lane_and_reaps_child(self):
        r = await self.checked(dict(max_http_attempts=2))
        self.assertFalse(r['passed'])
        self.assertEqual(r['transport']['http_attempts'], 2)
        self.assertIn('request-limit', r['failures'])

    async def test_shared_deadline_includes_mount_and_owned_http(self):
        r = await self.checked(dict(duration_s=1))
        self.assertFalse(r['passed'])
        self.assertIn('deadline', r['failures'])
        self.assertLess(r['elapsed_s'], 3)

    async def test_non_2xx_cannot_be_hidden_by_forum_adapter_fallback(self):
        async def fault(request, row, response):
            if request.path.endswith('/native-forum/contexts'):
                return response(dict(error='owned failure'), 503)
        r = await self.checked(fault=fault)
        self.assertFalse(r['passed'])
        self.assertEqual(r['fixture']['paths']['/api/v1/nostr/native-forum/contexts'], 1)
        self.assertEqual(r['fixture']['prepared_statuses']['503'], 1)
        self.assertEqual(r['failures'].count('unexpected-http-503'), 1)

    async def test_changed_image_body_stops_globally_without_retry(self):
        async def fault(request, row, response):
            if request.path.startswith('/data/'):
                return response(raw=b'wrong bytes', mime='image/png')
        r = await self.checked(fault=fault)
        self.assertFalse(r['passed'])
        self.assertIn('child-integrity-or-authority-stop', r['failures'])
        self.assertEqual(sum(n for p, n in r['fixture']['paths'].items() if p.startswith('/data/')), 1)

    async def test_response_cap_failure_is_not_a_successful_logical_action(self):
        r = await self.checked(dict(max_response_body_bytes=90))
        self.assertFalse(r['passed'])
        self.assertLessEqual(r['transport']['response_body_bytes_consumed'], 90)
        self.assertIn('response-byte-limit', r['failures'])


if __name__ == '__main__':
    unittest.main()
