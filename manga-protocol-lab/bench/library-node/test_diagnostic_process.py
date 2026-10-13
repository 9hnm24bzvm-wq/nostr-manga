"""Real controlled CLI supervision: malformed output and child ownership."""
import asyncio
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from pg_diagnostics import collect_sample

class DiagnosticProcess(unittest.IsolatedAsyncioTestCase):
    async def test_malformed_zero_exit_is_explicit_failure(self):
        for output in ('not json','[]','{}'):
            value=await collect_sample([sys.executable,'-c',f'print({output!r})'],'controlled')
            self.assertEqual(value['error'],'invalid diagnostic output')
        failed=await collect_sample([sys.executable,'-c','import json;print(json.dumps({"backend_outbox":{}}));raise SystemExit(2)'],'controlled')
        self.assertEqual(failed['error'],'diagnostic process failed')

    async def test_cancellation_waits_for_cli_to_reap_its_child(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);started=root/'started';done=root/'done';reaped=root/'reaped'
            child=f'import os,time;from pathlib import Path;Path({str(started)!r}).write_text(str(os.getpid()));time.sleep(.15);Path({str(done)!r}).write_text("done")'
            wrapper=f'import subprocess,sys;from pathlib import Path;subprocess.run([sys.executable,"-c",{child!r}],timeout=2,check=True);Path({str(reaped)!r}).write_text("reaped");print("{{\\"backend_outbox\\":{{}}}}")'
            task=asyncio.create_task(collect_sample([sys.executable,'-c',wrapper],'controlled'))
            async def wait_started():
                while not started.exists():await asyncio.sleep(.005)
            await asyncio.wait_for(wait_started(),2)
            pid=int(started.read_text());task.cancel()
            with self.assertRaises(asyncio.CancelledError):await task
            self.assertTrue(done.exists());self.assertTrue(reaped.exists())
            with self.assertRaises(ProcessLookupError):os.kill(pid,0)

if __name__=='__main__':unittest.main()
