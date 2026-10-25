import importlib.util,pathlib,sys,time
E=pathlib.Path(__file__).resolve().parent;spec=importlib.util.spec_from_file_location('owner',E/'owned-guard.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
assert m.run([sys.executable,'-c','print("normal owned guard")'],E,E/'guard-normal.json',20)==0
start=time.monotonic()
def fail_later(_):
 if time.monotonic()-start>.6:raise RuntimeError('controlled census-adjacent failure')
 return 0
m.allocated=fail_later
child='import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); time.sleep(20)'
parent='import subprocess,sys,time; subprocess.Popen([sys.executable,"-c",'+repr(child)+']); time.sleep(20)'
assert m.run([sys.executable,'-c',parent],E,E/'guard-exception-resistant-child.json',20,[E],0)==1
v=__import__('json').loads((E/'guard-exception-resistant-child.json').read_text());assert v['cleanup_verified']and not v['surviving_owned_processes']and v['guard_internal_failure']and any(x['signal']=='SIGKILL'for x in v['cleanup_signals'])and len(v['owned_process_births'])>=2
print('guard controls 2/2 passed')
