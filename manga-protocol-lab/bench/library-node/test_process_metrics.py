import tempfile
import unittest
from pathlib import Path
from process_metrics import ProcessSampler, parse_stat


def stat(name='postgres', user=80, system=20, start=9, rss=10):
    fields = ['0'] * 22
    fields[0], fields[11], fields[12], fields[19], fields[21] = 'S', str(user), str(system), str(start), str(rss)
    return f'5 ({name}) ' + ' '.join(fields)


class ProcessMetrics(unittest.TestCase):
    def test_stat_fields_and_comm_parentheses(self):
        row = parse_stat(stat(name='a (b)'), page_kib=4)
        self.assertEqual(row, {'pid': 5, 'name': 'a (b)', 'start_ticks': 9, 'cpu_ticks': 100, 'rss_kib': 40})

    def test_sample_delta_and_pid_reuse(self):
        with tempfile.TemporaryDirectory() as root:
            p = Path(root) / '5'; p.mkdir()
            f = p / 'stat'; f.write_text(stat())
            now = [10.0]
            sampler = ProcessSampler(root, clock=lambda: now[0], ticks=100, page_kib=4)
            self.assertIsNone(sampler.sample()['processes']['postgres']['cpu_cores_observed'])
            now[0] += 1; f.write_text(stat(user=130))
            self.assertEqual(sampler.sample()['processes']['postgres']['cpu_cores_observed'], .5)
            now[0] += 1; f.write_text(stat(user=900, start=20))
            self.assertEqual(sampler.sample()['processes']['postgres']['cpu_cores_observed'], 0)
            now[0] += 1; f.unlink()
            self.assertEqual(sampler.sample()['processes'], {})

    def test_bad_stat_and_unrelated_process_are_ignored(self):
        with tempfile.TemporaryDirectory() as root:
            p = Path(root) / '5'; p.mkdir(); (p/'stat').write_text('invalid')
            sampler = ProcessSampler(root, ticks=100, page_kib=4)
            self.assertEqual(sampler.sample()['process_stat_reads_skipped'], 1)
            (p/'stat').write_text(stat(name='unrelated'))
            self.assertEqual(sampler.sample()['processes'], {})


if __name__ == '__main__':
    unittest.main()
