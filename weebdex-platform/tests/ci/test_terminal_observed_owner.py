"""Finite real fork/reap controls; no services, network, or application fixture."""
import ctypes, importlib.util, json, os, signal, sys, tempfile, time, unittest
from pathlib import Path
from unittest.mock import patch
source=Path(os.environ.get('TERMINAL_OWNER_SOURCE',str(Path(__file__).resolve().parents[2]/'scripts/ci/run-sandbox-preflight.py')))
spec=importlib.util.spec_from_file_location('terminal_owner_under_test',source)
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
class TerminalObservedTests(unittest.TestCase):
    def setUp(self):
        assert ctypes.CDLL(None,use_errno=True).prctl(36,1,0,0,0)==0
        self.tmp=tempfile.TemporaryDirectory(prefix='terminal-observed-control-')
        self.owner=m.Owner(Path(self.tmp.name),work_seconds=5,cleanup_seconds=1,total_seconds=6)
        self.children={};self.fds=[];self.cleaned=False
    def tearDown(self):
        for pid,fd in self.children.items():
            try:signal.pidfd_send_signal(fd,signal.SIGKILL,None,0)
            except ProcessLookupError:pass
        for pid,fd in self.children.items():
            try:os.waitpid(pid,0)
            except ChildProcessError:pass
            os.close(fd)
        if not self.cleaned:self.owner.cleanup()
        for fd in self.fds:
            try:os.close(fd)
            except OSError:pass
        self.tmp.cleanup()
    def pipe(self):
        r,w=os.pipe();self.fds.extend([r,w]);return r,w
    def fork(self,fn):
        gate,release=self.pipe();pid=os.fork()
        if pid==0:
            try:
                os.read(gate,1);fn()
            finally:os._exit(0)
        self.children[pid]=os.pidfd_open(pid,0)
        # Give the unchanged outer qualification owner a live child to capture.
        time.sleep(.15);os.write(release,b'G');return pid
    def wait_state(self,pid,state):
        deadline=time.monotonic()+2
        while time.monotonic()<deadline:
            row=m.process(pid)
            if row is not None and row['state']==state:return row
            time.sleep(.002)
        self.fail('Expected real process state '+state)
    def reaped_zombie(self):
        pid=self.fork(lambda:None);row=self.wait_state(pid,'Z');os.waitpid(pid,0)
        self.assertIsNone(m.process(pid));return row
    def finish(self):
        result=self.owner.cleanup();self.cleaned=True;return result
    def test_real_zombie_then_reaped_is_recorded_terminal(self):
        row=self.reaped_zombie();self.owner.register(row);key=(row['pid'],row['birth'])
        self.assertIn(key,self.owner.ledger);self.assertIsNone(self.owner.ledger[key])
        self.assertEqual(self.owner.terminal_observed[key]['confirmation'],'absent')
        result=self.finish();self.assertTrue(result['verified_stopped']);self.assertFalse(result['forced'])
        journal=json.loads((Path(self.tmp.name)/'owner.json').read_text())
        self.assertEqual(journal['terminal_observed'][0]['birth'],row['birth'])
    def test_real_live_then_reaped_stays_unknown(self):
        gate,release=self.pipe();pid=self.fork(lambda:os.read(gate,1));row=m.process(pid)
        self.assertNotEqual(row['state'],'Z');os.write(release,b'G');os.waitpid(pid,0)
        self.owner.register(row);self.assertIn((pid,row['birth']),self.owner.unknown)
        self.assertFalse(self.finish()['verified_stopped'])
    def test_initial_x_is_not_widened_to_terminal(self):
        row=self.reaped_zombie();row['state']='X';self.owner.register(row)
        self.assertIn((row['pid'],row['birth']),self.owner.unknown)
        self.assertFalse(self.finish()['verified_stopped'])
    def test_different_birth_after_esrch_stays_unknown(self):
        row=self.reaped_zombie()
        with patch.object(m,'process',return_value={**row,'birth':row['birth']+1,'state':'S'}):
            self.owner.register(row)
        self.assertIn((row['pid'],row['birth']),self.owner.unknown)
        self.assertFalse(self.finish()['verified_stopped'])
    def test_unreadable_confirmation_fails_closed(self):
        row=self.reaped_zombie()
        with patch.object(self.owner,'_rows',return_value=[row]),patch.object(m,'process',side_effect=PermissionError('controlled denial')):
            self.assertIsNone(self.owner.scan())
        self.assertGreater(self.owner.errors,0);self.assertFalse(self.finish()['verified_stopped'])
    def test_same_birth_after_esrch_stays_unknown(self):
        row=self.reaped_zombie()
        with patch.object(m,'process',return_value=row):self.owner.register(row)
        self.assertIn((row['pid'],row['birth']),self.owner.unknown)
        self.assertFalse(self.finish()['verified_stopped'])
    def test_reappearing_terminal_identity_fails_closed(self):
        row=self.reaped_zombie();self.owner.register(row)
        with patch.object(m.os,'pidfd_open',side_effect=AssertionError('must not adopt reappearance')):
            self.owner.register({**row,'state':'S'})
        self.assertIn((row['pid'],row['birth']),self.owner.unknown)
        self.assertFalse(self.finish()['verified_stopped'])
    def test_terminal_records_share_birth_limit(self):
        first=self.reaped_zombie();second=self.reaped_zombie()
        with patch.object(m,'MAX_BIRTHS',1):
            self.owner.register(first)
            with self.assertRaises(m.Refused):self.owner.register(second)
        self.assertEqual(len(self.owner.ledger),1);self.assertFalse(self.finish()['verified_stopped'])
    def test_captured_pidfd_then_absence_is_closed_and_terminal(self):
        gate,release=self.pipe();pid=self.fork(lambda:os.read(gate,1));fd=os.pidfd_open(pid,0)
        os.write(release,b'G');row=self.wait_state(pid,'Z');os.waitpid(pid,0)
        with patch.object(m.os,'pidfd_open',return_value=fd):self.owner.register(row)
        with self.assertRaises(OSError):os.fstat(fd)
        self.assertTrue(self.finish()['verified_stopped'])
    def test_opened_reused_handle_is_closed_without_signal(self):
        row=self.reaped_zombie();gate,release=self.pipe();pid=self.fork(lambda:os.read(gate,1))
        fd=os.pidfd_open(pid,0)
        with patch.object(m.os,'pidfd_open',return_value=fd),patch.object(m,'process',return_value={**row,'birth':row['birth']+1,'state':'S'}),patch.object(m.signal,'pidfd_send_signal',side_effect=AssertionError('no reused handle signal')):
            self.owner.register(row)
        with self.assertRaises(OSError):os.fstat(fd)
        self.assertIsNotNone(m.process(pid));os.write(release,b'G');os.waitpid(pid,0)
        self.assertFalse(self.finish()['verified_stopped'])
    def test_opened_handle_closes_on_unreadable_second_read(self):
        row=self.reaped_zombie();gate,release=self.pipe();pid=self.fork(lambda:os.read(gate,1));fd=os.pidfd_open(pid,0)
        with patch.object(m.os,'pidfd_open',return_value=fd),patch.object(m,'process',side_effect=PermissionError('controlled denial')):
            with self.assertRaises(PermissionError):self.owner.register(row)
        with self.assertRaises(OSError):os.fstat(fd)
        os.write(release,b'G');os.waitpid(pid,0)
    def test_adopted_detached_descendant_still_prevents_clean_pass(self):
        ids_r,ids_w=self.pipe();middle_gate,middle_release=self.pipe();child_gate,child_release=self.pipe()
        def middle():
            grand=os.fork()
            if grand==0:
                os.setsid();os.write(ids_w,(str(os.getpid())+'\n').encode());os.read(child_gate,1);os._exit(0)
            os.read(middle_gate,1)
        pid=self.fork(middle);grand=int(os.read(ids_r,64));self.children[grand]=os.pidfd_open(grand,0)
        time.sleep(.15);os.write(middle_release,b'G');row=self.wait_state(pid,'Z');os.waitpid(pid,0)
        self.owner.register(row);live=self.owner.scan()
        self.assertIn(grand,[r['pid'] for r in live]);self.assertEqual(m.process(grand)['ppid'],os.getpid())
        result=self.finish();self.assertTrue(result['verified_stopped']);self.assertTrue(result['forced'])
        os.waitpid(grand,0);self.assertIsNone(m.process(grand))
    def test_zombie_group_leader_with_live_thread_is_not_terminal(self):
        ready_r,ready_w=self.pipe();gate,release=self.pipe()
        def threaded():
            import threading
            def hold():
                os.write(ready_w,b'R');os.read(gate,1)
            threading.Thread(target=hold).start()
            ctypes.CDLL(None).pthread_exit(None)
        pid=self.fork(threaded);os.read(ready_r,1);row=self.wait_state(pid,'Z')
        self.assertGreaterEqual(len(list(Path('/proc/'+str(pid)+'/task').iterdir())),2)
        self.owner.register(row);key=(pid,row['birth'])
        self.assertIsNotNone(self.owner.ledger[key]);self.assertNotIn(key,self.owner.terminal_observed)
        os.write(release,b'G');os.waitpid(pid,0);self.assertTrue(self.finish()['verified_stopped'])
if __name__=='__main__':unittest.main(verbosity=2)
