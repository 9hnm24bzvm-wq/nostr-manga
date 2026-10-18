"""Finite closed nested-schema/log controls. No browser, network or policy writes."""
import copy,importlib.util,json
from pathlib import Path
import unittest
P=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('closed_owner',P/'scripts/ci/run-sandbox-preflight.py');owner=importlib.util.module_from_spec(spec);spec.loader.exec_module(owner);D=owner.diagnostic
PRIVATE='https://synthetic.invalid/private?token=never-export'
def clean():return dict(verified_stopped=True,forced=False,remaining_owned=0,unknown_births=0,tracking_errors=0)
def worker():return dict(schema=2,stage='complete',caught_stage='none',caught_category='none',launch_diagnostic=D.empty_worker(),**dict.fromkeys(owner.FLAGS,True))
class DiagnosticSchema(unittest.TestCase):
 def test_schema2_is_mandatory_and_old_missing_facts_are_not_invented(self):
  w=worker();self.assertTrue(owner.worker_valid(w));self.assertFalse(owner.worker_valid(dict(w,schema=1)))
  del w['launch_diagnostic'];self.assertFalse(owner.worker_valid(w))
 def test_fixed_logger_configuration_cannot_inherit_arbitrary_debug(self):
  env=owner.child_environment(dict(DEBUG=PRIVATE,DEBUG_COLORS=PRIVATE,DEBUG_HIDE_DATE=PRIVATE),Path('/synthetic'))
  self.assertEqual({k:env[k]for k in ('DEBUG','DEBUG_COLORS','DEBUG_HIDE_DATE')},{'DEBUG':'pw:browser','DEBUG_COLORS':'0','DEBUG_HIDE_DATE':'1'})
  self.assertNotIn(PRIVATE,json.dumps(env))
 def test_complete_log_has_only_fixed_correlated_process_witnesses(self):
  value=D.parse_log(b'pw:browser <launched> pid=12\npw:browser [pid=12] <process did exit: exitCode=0, signal=null>\n')
  self.assertTrue(D.valid_log(value));self.assertEqual(value['process']['launches'],1);self.assertEqual(value['process']['exit_code'],0)
  self.assertEqual(value['process']['witness'],'owned-worker-log');self.assertNotIn('pid',json.dumps(value))
 def test_raw_urls_paths_and_pids_never_export(self):
  raw=('pw:browser <launched> pid=314159\npw:browser [pid=314159][err] error while loading shared libraries: '+PRIVATE+'\npw:browser [pid=314159] <process did exit: exitCode=127, signal=null>\n').encode()
  value=D.parse_log(raw);self.assertTrue(value['log_signatures']['loader']);self.assertNotIn(PRIVATE,json.dumps(value));self.assertNotIn('314159',json.dumps(value))
 def test_sigtrap_and_multiple_attempts_do_not_select_arbitrary_scalar(self):
  one=D.parse_log(b'pw:browser <launched> pid=1\npw:browser [pid=1] <process did exit: exitCode=null, signal=SIGTRAP>\n');self.assertEqual(one['process']['exit_signal'],'SIGTRAP');self.assertIsNone(one['process']['exit_code'])
  multi=D.parse_log(b'pw:browser <launched> pid=1\npw:browser <launched> pid=2\npw:browser [pid=1] <process did exit: exitCode=0, signal=null>\npw:browser [pid=2] <process did exit: exitCode=127, signal=null>\n');self.assertEqual(multi['process']['launches'],2);self.assertEqual(multi['process']['exit_events'],2);self.assertIsNone(multi['process']['exit_code']);self.assertEqual(multi['process']['exit_signal'],'multiple')
 def test_missing_partial_invalid_and_oversize_log_witnesses_are_unknown(self):
  for raw,state in [(None,'unavailable'),(b'pw:browser <launched> pid=1','partial'),(b'\xff','malformed'),(b'x'*(D.MAX_LOG+1),'oversized'),(b'pw:browser [pid=99] <process did exit: exitCode=0, signal=null>\n','partial')]:
   value=D.parse_log(raw);self.assertEqual(value['process']['state'],state);self.assertIsNone(value['process']['started']);self.assertTrue(D.valid_log(value))
 def test_nested_stderr_cannot_spoof_process_markers(self):
  value=D.parse_log(b'pw:browser [pid=12][err] pw:browser <launched> pid=9\n');self.assertEqual(value['process']['launches'],0);self.assertFalse(value['process']['started'])
 def test_control_characters_in_child_output_cannot_start_debug_process_records(self):
  for separator in ('\r','\x85','\u2028'):
   raw=('pw:browser [pid=12][err] private'+separator+'pw:browser <launched> pid=9\n').encode()
   value=D.parse_log(raw);self.assertEqual(value['process']['launches'],0);self.assertFalse(value['process']['started'])
 def test_closed_nested_schema_rejects_unknown_fields_types_and_private_values(self):
  base=owner.public_projection(worker(),0,clean(),'complete',1,'a'*64,D.parse_log(b''));owner.validate_public(base)
  mutations=[lambda v:v['launch_diagnostic'].update(raw_error=PRIVATE),lambda v:v['launch_diagnostic']['host'].update(tmp=PRIVATE),lambda v:v['launch_diagnostic']['process'].update(exit_code=True),lambda v:v['launch_diagnostic']['error'].update(code=PRIVATE),lambda v:v['launch_diagnostic']['log_signatures'].update(loader=PRIVATE)]
  for change in mutations:
   value=copy.deepcopy(base);change(value)
   with self.assertRaises(owner.Refused):owner.validate_public(value)
 def test_host_ready_and_log_exit_zero_cannot_replace_a_handle(self):
  w=worker();w['browser_created']=False;w['launch_diagnostic']['host']={k:next(iter(v-{'unavailable'}))for k,v in D.HOST.items()}
  result=owner.public_projection(w,0,clean(),'complete',1,'a'*64,D.parse_log(b'pw:browser <launched> pid=1\npw:browser [pid=1] <process did exit: exitCode=0, signal=null>\n'))
  self.assertEqual(result['status'],'failed');self.assertFalse(result['worker_record_valid']);owner.validate_public(result)
 def test_spawn_envelope_and_conflict_require_observed_bounded_message(self):
  for source,code,reason in [('spawn-envelope','ENOENT','spawn-not-found'),('conflict','unknown','multiple-known-families')]:
   w=dict(worker(),caught_stage='launching-browser',caught_category='launch-failed',**dict.fromkeys(owner.FLAGS,False))
   w['launch_diagnostic']['error']=dict(reason=reason,code=code,code_source=source,message_state='bounded',signatures=dict.fromkeys(D.SIGNATURES,False))
   w['launch_diagnostic']['error']['signatures']['spawn_error']=True
   self.assertTrue(owner.worker_valid(w))
   public=owner.public_projection(w,1,clean(),'worker-failed',1,'a'*64)
   owner.validate_public(public)
   invalid=copy.deepcopy(w);error=invalid['launch_diagnostic']['error'];error['message_state']='unavailable';error['signatures']=dict.fromkeys(D.SIGNATURES,None);error['signatures']['spawn_error']=True
   self.assertFalse(owner.worker_valid(invalid))
   public['launch_diagnostic']['error']=error
   with self.assertRaises(owner.Refused):owner.validate_public(public)
 def test_complete_exit_witness_requires_an_observed_launch(self):
  invalid=D.parse_log(b'');invalid['process'].update(exited=True,exit_events=1)
  self.assertFalse(D.valid_log(invalid))
  public=owner.public_projection(worker(),0,clean(),'complete',1,'a'*64,D.parse_log(b''));public['launch_diagnostic'].update(invalid)
  with self.assertRaises(owner.Refused):owner.validate_public(public)
 def test_malformed_top_level_launch_markers_are_not_clean_zero_observations(self):
  for marker in ('pid=0','pid=-1','pid=01','pid=1 private','pid='+str(10**10)):
   raw=('pw:browser <launched> '+marker+'\n').encode()
   value=D.parse_log(raw);self.assertEqual(value['process']['state'],'malformed');self.assertIsNone(value['process']['started'])
   nested=D.parse_log(b'pw:browser [pid=12][err] '+raw);self.assertEqual(nested['process']['state'],'complete');self.assertEqual(nested['process']['launches'],0)
if __name__=='__main__':unittest.main()
