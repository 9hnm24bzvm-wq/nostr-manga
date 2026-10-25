"""Synthetic cleanup control flow only; no process, network or PG runtime."""
import hashlib,importlib.util,os,time,types,unittest
from pathlib import Path
D=Path(__file__).resolve().parent;H=D.parent/'pg18-diagnostics-worktree/weebdex-platform/tests/runtime/auth_fixture_guard.py'
assert hashlib.sha256(H.read_bytes()).hexdigest()=='17ff9c2eb04a4bdefaa3b61e975e77567f873208a83454afcdd6ca19c5cfc33e'
spec=importlib.util.spec_from_file_location('original_helper',H);original=importlib.util.module_from_spec(spec);spec.loader.exec_module(original)
spec=importlib.util.spec_from_file_location('cleanup_controls',D/'cleanup-owned.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
class Child:
 pid=99999999
 def poll(self):return 0
 def wait(self,timeout=0):return 0
class CleanupControls(unittest.TestCase):
 def run_case(self,broken_stop=False,broken_send=False):
  row={'pid':Child.pid,'start_ticks':7,'session':Child.pid,'group':Child.pid,'state':'S','ppid':os.getpid()};ident=original.identity(row);known={(row['pid'],7):ident};live=dict(row);signals=[]
  def members(*a,**k):raise OSError('synthetic census failure')
  def same(_):return dict(live) if live else None
  def send(_,sig):
   signals.append(sig)
   if broken_send:raise OSError('synthetic exact-signal failure')
   live.clear()
  def stop(*a,**k):raise OSError('synthetic primary cleanup failure')
  helper=types.SimpleNamespace(members=members,same=same,send=send,identity=original.identity,stop=stop if broken_stop else original.stop)
  result=module.cleanup(helper,[(Child(),ident)],known,{},time.monotonic()+(.12 if broken_send else 1))
  return result,signals
 def test_census_failure_still_signals_exact_captured_birth(self):
  (forced,remaining,error),signals=self.run_case();self.assertFalse(forced);self.assertEqual(remaining,[]);self.assertIn('process census',error);self.assertTrue(signals)
 def test_primary_failure_runs_exact_birth_fallback(self):
  (forced,remaining,error),signals=self.run_case(broken_stop=True);self.assertFalse(forced);self.assertEqual(remaining,[]);self.assertIn('primary cleanup',error);self.assertTrue(signals)
 def test_failed_signal_cannot_claim_stopped_and_respects_deadline(self):
  started=time.monotonic();(forced,remaining,error),signals=self.run_case(broken_stop=True,broken_send=True);self.assertTrue(remaining);self.assertIn('exact-birth signal',error);self.assertTrue(signals);self.assertLess(time.monotonic()-started,.5)
if __name__=='__main__':unittest.main(verbosity=2)
