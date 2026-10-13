"""Actual CLI/controlled-PostgreSQL sampler contract, not application outcomes."""
import asyncio
import copy
from datetime import datetime
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

from pg_diagnostics import (CHECKPOINTER_COUNTERS, SETTING_NAMES, WAL_COUNTERS,
                            collect_sample, validate_wal_snapshot)


def example_wal_snapshot():
    """Synthetic validator input only; actual CLI tests use PostgreSQL below."""
    timestamp='2026-10-11T00:00:00+00:00'
    settings=dict(fsync=('on',None),synchronous_commit=('on',None),
                  full_page_writes=('on',None),shared_buffers=('2048','8kB'),
                  max_wal_size=('1024','MB'),wal_buffers=('64','8kB'),
                  wal_compression=('off',None),checkpoint_timeout=('300','s'),
                  checkpoint_completion_target=('0.9',None),wal_sync_method=('fdatasync',None))
    return dict(sampled_at=timestamp,postmaster_started_at=timestamp,server_version_num=180006,
                pg_stat_wal=dict.fromkeys(WAL_COUNTERS,1)|dict(stats_reset=timestamp),
                pg_stat_checkpointer=dict.fromkeys(CHECKPOINTER_COUNTERS,1)|dict(stats_reset=timestamp),
                settings={key:dict(value=setting,setting=setting,unit=unit,source='default')
                          for key,(setting,unit) in settings.items()})


class WalSnapshotShape(unittest.TestCase):
    def test_only_known_bounded_snapshot_fields_are_preserved(self):
        value=example_wal_snapshot()
        expected=copy.deepcopy(value)
        del expected['sampled_at']
        value.update(query='secret query',activity=[],connection='secret connection')
        value['pg_stat_wal']['wal_write_time']='not a PostgreSQL 18 WAL counter'
        value['pg_stat_checkpointer']['unknown']='secret'
        value['settings']['password']='secret'
        value['settings']['fsync']['sourcefile']='secret path'
        self.assertEqual(validate_wal_snapshot(value),expected)

    def test_missing_nonfinite_boolean_negative_and_invalid_identity_are_errors(self):
        for group,keys in (('pg_stat_wal',WAL_COUNTERS),('pg_stat_checkpointer',CHECKPOINTER_COUNTERS)):
            for key in keys:
                for bad in (None,True,-1,'0',float('nan'),float('inf'),2**65):
                    with self.subTest(group=group,key=key,bad=bad):
                        value=example_wal_snapshot();value[group][key]=bad
                        with self.assertRaises(ValueError):validate_wal_snapshot(value)
            for bad in ('','2026-10-11T00:00:00','invalid'):
                value=example_wal_snapshot();value[group]['stats_reset']=bad
                with self.assertRaises(ValueError):validate_wal_snapshot(value)
            value=example_wal_snapshot();del value[group]['stats_reset']
            with self.assertRaises(ValueError):validate_wal_snapshot(value)
        for key in ('sampled_at','postmaster_started_at','server_version_num','pg_stat_wal','pg_stat_checkpointer','settings'):
            value=example_wal_snapshot();del value[key]
            with self.assertRaises(ValueError):validate_wal_snapshot(value)
        for bad in (None,'','2026-10-11T00:00:00','invalid'):
            value=example_wal_snapshot();value['sampled_at']=bad
            with self.assertRaises(ValueError):validate_wal_snapshot(value)

    def test_settings_missing_or_unbounded_are_errors_and_reset_identity_is_raw(self):
        for key,bad in (('value','secret\nquery'),('setting',None),('unit','secret'),('source','secret')):
            value=example_wal_snapshot();value['settings']['max_wal_size'][key]=bad
            with self.assertRaises(ValueError):validate_wal_snapshot(value)
        value=example_wal_snapshot();del value['settings']['fsync']
        with self.assertRaises(ValueError):validate_wal_snapshot(value)
        value=example_wal_snapshot()
        value['pg_stat_wal']['stats_reset']='2026-10-11T00:00:01+00:00'
        self.assertEqual(validate_wal_snapshot(value),{key:item for key,item in value.items() if key!='sampled_at'},
                         'Independent reset epochs must not be merged or zero-filled')
        value['pg_stat_wal']['stats_reset']=None
        self.assertIsNone(validate_wal_snapshot(value)['pg_stat_wal']['stats_reset'],
                          'An actual null reset identity stays unavailable, never fabricated')


class DiagnosticShape(unittest.IsolatedAsyncioTestCase):
    async def sample(self, value, **options):
        return await collect_sample([sys.executable,'-c',f'print({json.dumps(value)!r})'],'controlled',**options)

    async def test_activity_field_required_and_array_validated(self):
        for value in ({'backend_outbox':{}},{'activity':{}},{'activity':None}):
            self.assertEqual((await self.sample(value,expected_field='activity'))['error'],'invalid diagnostic output')
        self.assertEqual((await self.sample({'activity':[]},expected_field='activity'))['activity'],[])
        self.assertEqual((await self.sample({'error':'TimeoutExpired'},expected_field='activity'))['error'],'TimeoutExpired')

    async def test_default_backlog_contract_unchanged(self):
        self.assertEqual((await self.sample({'backend_outbox':{}}))['backend_outbox'],{})
        self.assertEqual((await self.sample({'activity':[]}))['error'],'invalid diagnostic output')


@unittest.skipUnless(os.environ.get('MIXED_DIAGNOSTIC_TEST_DB') and os.environ.get('MIXED_DIAGNOSTIC_OWNED_RUNTIME'),
                     'isolated owned PostgreSQL fixture required')
class DiagnosticCli(unittest.TestCase):
    def setUp(self):
        self.database=os.environ['MIXED_DIAGNOSTIC_TEST_DB']
        self.psql=os.environ.get('MIXED_PSQL','vendor/pgsql/bin/psql')
        runtime=Path(os.environ['MIXED_DIAGNOSTIC_OWNED_RUNTIME'])
        self.assertEqual(runtime.parent,Path('/tmp'))
        self.assertTrue(runtime.name.startswith('mixed-diagnostic-'))
        self.assertEqual(Path(self.sql("SHOW data_directory")),runtime/'data',
                         'Never reset statistics or arrange data in an application runtime')
        self.assertEqual(self.sql('SELECT host(inet_server_addr())'), '127.0.0.1')
        self.assertEqual(self.sql('SHOW port'),'55437')
        self.assertEqual(self.sql('SHOW unix_socket_directories'),'')
        self.command=[sys.executable,'bench/library-node/pg_diagnostics.py','--psql',self.psql,'--database',self.database]
        self.children=[]
        self.scratch=tempfile.TemporaryDirectory(prefix='mixed-diagnostic-test-')
        self.addCleanup(self.scratch.cleanup)
        self.addCleanup(self.stop_children)

    def stop_children(self):
        # Only this disposable cluster's explicitly named test sessions are owned.
        self.sql("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE application_name LIKE 'pg-diagnostic-%' AND pid<>pg_backend_pid()")
        for process in self.children:
            if process.poll() is None:
                process.terminate()
            try: process.communicate(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill();process.communicate(timeout=3)

    def sql(self, query):
        return subprocess.run([self.psql,'-X','-q','-A','-t','-v','ON_ERROR_STOP=1','-c',query],text=True,capture_output=True,check=True,timeout=5).stdout.strip()

    def connect(self, name, sql=None):
        command=[self.psql,'-X','-q','-A','-t','-v','ON_ERROR_STOP=1']
        if sql is not None: command.extend(['-c',sql])
        process=subprocess.Popen(command,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,env=dict(os.environ,PGAPPNAME=name))
        self.children.append(process)
        return process

    def send(self, process, query):
        process.stdin.write(query+'\n');process.stdin.flush()

    def wait_sql(self, query, expected='t', seconds=3):
        deadline=time.monotonic()+seconds
        while time.monotonic()<deadline:
            if self.sql(query)==expected:return
            time.sleep(.02)
        self.fail('Controlled PostgreSQL state did not become ready before the deadline')

    def evidence(self, label, value):
        print(json.dumps(dict(evidence=label,result=value),sort_keys=True),flush=True)

    def wrapper(self, body):
        path=Path(self.scratch.name)/'psql-probe'
        path.write_text(f'#!{sys.executable}\nimport os,sys\nfrom pathlib import Path\n'+body)
        path.chmod(0o700)
        return str(path)

    def run_cli(self, *args, env=None, psql=None):
        command=list(self.command)
        if psql is not None: command[command.index('--psql')+1]=psql
        started=time.monotonic()
        result=subprocess.run(command+list(args),env=env,text=True,capture_output=True,timeout=8)
        return result,json.loads(result.stdout),time.monotonic()-started

    def assert_reaped(self, path):
        pid=int(path.read_text())
        with self.assertRaises(ProcessLookupError):os.kill(pid,0)

    def test_read_only_sampler_reports_arranged_backlogs(self):
        result,value,_=self.run_cli()
        self.assertEqual(result.returncode,0)
        self.assertTrue(value['diagnostic_only'])
        self.assertNotIn('activity',value)
        for key in ('pg_stat_wal','pg_stat_checkpointer','settings','postmaster_started_at','server_version_num'):
            self.assertNotIn(key,value,'Default backlog diagnostics must remain unchanged')
        self.assertEqual(value['backend_outbox']['pending_import'],2)
        self.assertEqual(value['backend_outbox']['pending_delete'],1)
        self.assertGreater(value['backend_outbox']['oldest_pending_age_s'],59)
        self.assertEqual(value['library_retirement']['removed_history_rows'],1)
        self.assertEqual(value['library_retirement']['older_than_grace_rows'],1)
        self.assertEqual(value['blob_cleanup']['queued'],0)
        self.assertEqual(value['public_export']['queued'],0)
        # Sampling twice must not consume or alter any queue.
        _,again,_=self.run_cli()
        self.assertEqual(again['backend_outbox']['pending_import'],2)
        self.assertEqual(again['backend_outbox']['pending_delete'],1)
        self.evidence('default_backlog_unchanged',value)

    def test_activity_groups_idle_active_and_lock_wait_without_identities(self):
        for index in range(2):
            idle=self.connect(f'pg-diagnostic-idle-{index}')
            self.send(idle,'SELECT 1;')
        holder=self.connect('pg-diagnostic-holder')
        self.send(holder,'BEGIN; LOCK TABLE diagnostic_wait_fixture IN ACCESS EXCLUSIVE MODE;')
        self.wait_sql("SELECT count(*)=1 FROM pg_stat_activity WHERE application_name='pg-diagnostic-holder' AND state='idle in transaction'")
        self.connect('pg-diagnostic-query-sentinel','SELECT pg_sleep(30) /* diagnostic-query-text-sentinel */')
        self.connect('pg-diagnostic-waiter','SELECT count(*) FROM diagnostic_wait_fixture')
        self.wait_sql("SELECT count(*)=2 FROM pg_stat_activity WHERE application_name LIKE 'pg-diagnostic-idle-%' AND state='idle'")
        self.wait_sql("SELECT count(*)=1 FROM pg_stat_activity WHERE application_name='pg-diagnostic-query-sentinel' AND wait_event='PgSleep'")
        self.wait_sql("SELECT count(*)=1 FROM pg_stat_activity WHERE application_name='pg-diagnostic-waiter' AND wait_event_type='Lock'")
        result,value,_=self.run_cli('--activity-only',env=dict(os.environ,PGPASSWORD='diagnostic-secret-sentinel'))
        self.assertEqual(result.returncode,0)
        self.assertTrue(value['diagnostic_only'])
        self.assertNotIn('backend_outbox',value)
        self.assertIsNotNone(datetime.fromisoformat(value['sampled_at']).tzinfo)
        rows=value['activity']
        self.assertEqual(sum(row['sessions'] for row in rows),5,'The sampler must exclude its own active connection')
        for row in rows:
            self.assertEqual(set(row),{'database','backend_type','state','wait_event_type','wait_event','sessions'})
            self.assertEqual(row['database'],self.database)
            self.assertEqual(row['backend_type'],'client backend')
            self.assertIsInstance(row['sessions'],int)
        self.assertEqual(sum(row['sessions'] for row in rows if row['state']=='idle'),2)
        self.assertTrue(any(row['state']=='active' and row['wait_event_type']=='Timeout' and row['wait_event']=='PgSleep' for row in rows))
        self.assertTrue(any(row['state']=='active' and row['wait_event_type']=='Lock' and row['wait_event']=='relation' for row in rows))
        for forbidden in ('diagnostic-secret-sentinel','diagnostic-query-text-sentinel','pg-diagnostic-query-sentinel','application_name','usename','client_addr','pg_sleep'):
            self.assertNotIn(forbidden,result.stdout+result.stderr)
        self.evidence('controlled_idle_active_lock_activity',value)

    def test_activity_empty_excludes_sampler_and_other_database(self):
        self.connect('pg-diagnostic-other-database','SELECT pg_sleep(30)')
        self.wait_sql("SELECT count(*)=1 FROM pg_stat_activity WHERE application_name='pg-diagnostic-other-database' AND wait_event='PgSleep'")
        # This separate CLI connection samples postgres, which has no fixture sessions.
        command=list(self.command)
        command[command.index('--database')+1]='postgres'
        result=subprocess.run(command+['--activity-only'],text=True,capture_output=True,check=True,timeout=8)
        value=json.loads(result.stdout)
        self.assertEqual(value['activity'],[])
        self.evidence('empty_scoped_activity_sampler_excluded',value)

    def test_wal_checkpointer_and_settings_are_actual_cluster_snapshots(self):
        before=json.loads(self.sql('SELECT row_to_json(w) FROM pg_stat_wal AS w'))
        checkpoint_before=json.loads(self.sql('SELECT row_to_json(c) FROM pg_stat_checkpointer AS c'))
        # Controlled disposable fixture only. The sampling SQL itself never writes.
        self.addCleanup(self.sql,'TRUNCATE diagnostic_wait_fixture')
        self.sql('INSERT INTO diagnostic_wait_fixture SELECT generate_series(1,64); SELECT pg_stat_force_next_flush()')
        self.sql('CHECKPOINT')
        self.wait_sql(f"SELECT num_done>{checkpoint_before['num_done']} FROM pg_stat_checkpointer")
        self.wait_sql(f"SELECT wal_records>{before['wal_records']} FROM pg_stat_wal")
        result,value,_=self.run_cli('--activity-only')
        self.assertEqual(result.returncode,0)
        clean=validate_wal_snapshot(value)
        self.assertEqual(clean['server_version_num'],int(self.sql("SHOW server_version_num")))
        self.assertEqual(datetime.fromisoformat(clean['postmaster_started_at']),
                         datetime.fromisoformat(self.sql('SELECT pg_postmaster_start_time()')))
        self.assertEqual(set(clean['settings']),set(SETTING_NAMES))
        names=','.join("'"+name+"'" for name in SETTING_NAMES)
        actual=json.loads(self.sql("SELECT json_object_agg(name,json_build_object('value',current_setting(name),"
                                   "'setting',setting,'unit',unit,'source',source)) FROM pg_settings WHERE name IN ("+names+")"))
        self.assertEqual(clean['settings'],actual)
        self.assertEqual(clean['settings']['shared_buffers']['value'],'16MB')
        self.assertEqual(clean['settings']['shared_buffers']['unit'],'8kB')
        self.assertEqual(clean['settings']['shared_buffers']['source'],'configuration file')
        self.assertEqual(clean['settings']['max_wal_size']['setting'],os.environ['MIXED_DIAGNOSTIC_MAX_WAL_MB'])
        self.assertEqual(clean['settings']['max_wal_size']['unit'],'MB')
        self.assertEqual(clean['settings']['max_wal_size']['source'],'configuration file')
        for group,keys,previous in (('pg_stat_wal',WAL_COUNTERS,before),
                                    ('pg_stat_checkpointer',CHECKPOINTER_COUNTERS,checkpoint_before)):
            later=json.loads(self.sql(f'SELECT row_to_json(s) FROM {group} AS s'))
            self.assertEqual(set(clean[group]),set(keys)|{'stats_reset'})
            self.assertEqual(clean[group]['stats_reset'],previous['stats_reset'])
            self.assertEqual(clean[group]['stats_reset'],later['stats_reset'])
            for key in keys:
                self.assertTrue(math.isfinite(clean[group][key]))
                self.assertGreaterEqual(clean[group][key],previous[key])
                self.assertLessEqual(clean[group][key],later[key])
        self.assertGreater(clean['pg_stat_wal']['wal_records'],before['wal_records'])
        self.assertGreater(clean['pg_stat_wal']['wal_bytes'],before['wal_bytes'])
        self.assertGreater(clean['pg_stat_checkpointer']['num_done'],checkpoint_before['num_done'])
        self.evidence('actual_pg18_cluster_counters_settings_and_units',value)

    def test_wal_snapshot_retains_independent_real_reset_epochs(self):
        result,before,_=self.run_cli('--activity-only')
        self.assertEqual(result.returncode,0)
        # This class only runs inside test-pg-diagnostics.sh's disposable cluster.
        # Never use these reset functions against application benchmark runtimes.
        self.sql("SELECT pg_stat_reset_shared('wal')")
        result,wal_reset,_=self.run_cli('--activity-only')
        self.assertEqual(result.returncode,0)
        self.assertNotEqual(wal_reset['pg_stat_wal']['stats_reset'],before['pg_stat_wal']['stats_reset'])
        self.assertEqual(wal_reset['pg_stat_checkpointer']['stats_reset'],before['pg_stat_checkpointer']['stats_reset'])
        self.sql("SELECT pg_stat_reset_shared('checkpointer')")
        result,both_reset,_=self.run_cli('--activity-only')
        self.assertEqual(result.returncode,0)
        self.assertEqual(both_reset['pg_stat_wal']['stats_reset'],wal_reset['pg_stat_wal']['stats_reset'])
        self.assertNotEqual(both_reset['pg_stat_checkpointer']['stats_reset'],wal_reset['pg_stat_checkpointer']['stats_reset'])
        self.assertEqual(both_reset['postmaster_started_at'],before['postmaster_started_at'])
        self.assertNotIn('delta',both_reset['pg_stat_wal'])
        self.assertNotIn('delta',both_reset['pg_stat_checkpointer'])
        self.evidence('actual_independent_stats_reset_identity',dict(before=before,wal_reset=wal_reset,both_reset=both_reset))

    def test_cli_rejects_malformed_snapshot_without_fabricating_counters(self):
        for mutate in ("value.pop('pg_stat_wal')", "value['pg_stat_wal']['wal_bytes']=float('nan')",
                       "value['pg_stat_checkpointer']['stats_reset']='invalid'", "value['settings']['fsync']['value']='secret-sentinel'",
                       "value['sampled_at']='2026-10-11T00:00:00'"):
            path=self.wrapper(
                "import json,subprocess\n"
                f"result=subprocess.run([{self.psql!r},*sys.argv[1:]],input=sys.stdin.read(),text=True,capture_output=True,check=True)\n"
                "value=json.loads(result.stdout)\n"+mutate+"\nprint(json.dumps(value))\n")
            result,value,_=self.run_cli('--activity-only',psql=path)
            self.assertEqual(result.returncode,2)
            self.assertEqual(value['error'],'ValueError')
            self.assertNotIn('pg_stat_wal',value)
            self.assertNotIn('pg_stat_checkpointer',value)
            self.assertNotIn('secret-sentinel',result.stdout+result.stderr)
        self.evidence('actual_cli_malformed_statistics_rejected',value)

    def test_failed_connection_is_explicit_without_connection_secrets(self):
        env=dict(os.environ,PGPORT='1',PGPASSWORD='diagnostic-secret-sentinel')
        for mode in ((),('--activity-only',)):
            result,value,elapsed=self.run_cli(*mode,env=env)
            self.assertNotEqual(result.returncode,0)
            self.assertIn('error',value)
            self.assertNotIn('diagnostic-secret-sentinel',result.stdout+result.stderr)
            self.assertLess(elapsed,6)
            self.evidence('failed_connection_redacted',value)

    def test_actual_postgres_rejects_writes_in_both_modes(self):
        error_path=Path(self.scratch.name)/'postgres-error'
        path=self.wrapper(
            "sql=sys.stdin.read()\n"
            "assert 'BEGIN READ ONLY;' in sql\n"
            "assert os.environ['PGOPTIONS']=='-c default_transaction_read_only=on'\n"
            "sql=sql.replace('COMMIT;', 'INSERT INTO diagnostic_wait_fixture VALUES (77); COMMIT;')\n"
            f"error=open({str(error_path)!r},'w');os.dup2(error.fileno(),2)\n"
            f"os.execv({self.psql!r},[{self.psql!r},*sys.argv[1:],'-v','VERBOSITY=sqlstate','-c',sql])\n")
        for mode in ((),('--activity-only',)):
            result,value,_=self.run_cli(*mode,psql=path)
            self.assertEqual(result.returncode,2)
            self.assertEqual(value['error'],'CalledProcessError')
            self.assertIn('25006',error_path.read_text(),'PostgreSQL itself must report read_only_sql_transaction')
            self.assertEqual(self.sql('SELECT count(*) FROM diagnostic_wait_fixture'),'0')
            self.evidence('actual_postgres_write_rejected',value)

    def test_actual_sql_statement_timeout_reaps_psql(self):
        pid_path=Path(self.scratch.name)/'child.pid'
        error_path=Path(self.scratch.name)/'postgres-error'
        path=self.wrapper(
            f"Path({str(pid_path)!r}).write_text(str(os.getpid()))\n"
            "sql=sys.stdin.read().replace('SELECT json_build_object(', 'SELECT pg_sleep(30); SELECT json_build_object(',1)\n"
            f"error=open({str(error_path)!r},'w');os.dup2(error.fileno(),2)\n"
            f"os.execv({self.psql!r},[{self.psql!r},*sys.argv[1:],'-v','VERBOSITY=sqlstate','-c',sql])\n")
        result,value,elapsed=self.run_cli('--activity-only',psql=path,env=dict(os.environ,PGAPPNAME='pg-diagnostic-sql-timeout'))
        self.assertEqual(result.returncode,2)
        self.assertEqual(value['error'],'CalledProcessError')
        self.assertIn('57014',error_path.read_text(),'PostgreSQL itself must report query_canceled')
        self.assertGreater(elapsed,1.8)
        self.assertLess(elapsed,4)
        self.assert_reaped(pid_path)
        self.wait_sql("SELECT count(*)=0 FROM pg_stat_activity WHERE application_name='pg-diagnostic-sql-timeout'")
        self.evidence('actual_sql_2s_timeout_psql_reaped',value)

    def test_actual_lock_timeout_bounded_and_activity_remains_available(self):
        holder=self.connect('pg-diagnostic-outbox-holder')
        self.send(holder,'BEGIN; LOCK TABLE backend_outbox IN ACCESS EXCLUSIVE MODE;')
        self.wait_sql("SELECT count(*)=1 FROM pg_stat_activity WHERE application_name='pg-diagnostic-outbox-holder' AND state='idle in transaction'")
        result,value,elapsed=self.run_cli()
        self.assertEqual(result.returncode,2)
        self.assertEqual(value['error'],'CalledProcessError')
        self.assertGreater(elapsed,.4)
        self.assertLess(elapsed,2)
        self.evidence('actual_lock_500ms_timeout',value)
        activity_result,activity,_=self.run_cli('--activity-only')
        self.assertEqual(activity_result.returncode,0)
        self.assertTrue(any(row['state']=='idle in transaction' for row in activity['activity']))

    def test_actual_cli_5s_child_timeout_reaps_process(self):
        pid_path=Path(self.scratch.name)/'child.pid'
        path=self.wrapper(f"Path({str(pid_path)!r}).write_text(str(os.getpid()))\nimport time\ntime.sleep(30)\n")
        result,value,elapsed=self.run_cli('--activity-only',psql=path)
        self.assertEqual(result.returncode,2)
        self.assertEqual(value['error'],'TimeoutExpired')
        self.assertGreater(elapsed,4.8)
        self.assertLess(elapsed,7)
        self.assert_reaped(pid_path)
        self.evidence('actual_cli_5s_child_timeout_reaped',value)

    def test_actual_cli_async_cancellation_waits_for_psql_reaping(self):
        pid_path=Path(self.scratch.name)/'child.pid'
        path=self.wrapper(
            f"Path({str(pid_path)!r}).write_text(str(os.getpid()))\n"
            "os.environ['PGAPPNAME']='pg-diagnostic-cancellation'\n"
            "sql=sys.stdin.read().replace('SELECT json_build_object(', 'SELECT pg_sleep(.25); SELECT json_build_object(',1)\n"
            f"os.execv({self.psql!r},[{self.psql!r},*sys.argv[1:],'-c',sql])\n")
        command=list(self.command)
        command[command.index('--psql')+1]=path
        async def check():
            ticks=0
            finished=False
            async def heartbeat():
                nonlocal ticks
                while not finished:
                    ticks+=1
                    await asyncio.sleep(.01)
            heartbeat_task=asyncio.create_task(heartbeat())
            task=asyncio.create_task(collect_sample(command+['--activity-only'],self.database,expected_field='activity'))
            async def wait_started():
                while not pid_path.exists():await asyncio.sleep(.005)
            try:
                await asyncio.wait_for(wait_started(),2)
                task.cancel()
                with self.assertRaises(asyncio.CancelledError):await task
                self.assert_reaped(pid_path)
                self.assertGreater(ticks,5,'Subprocess collection must leave the event loop responsive')
                return ticks
            finally:
                finished=True
                await heartbeat_task
                if not task.done():
                    task.cancel()
                    await asyncio.gather(task,return_exceptions=True)
        ticks=asyncio.run(check())
        self.wait_sql("SELECT count(*)=0 FROM pg_stat_activity WHERE application_name='pg-diagnostic-cancellation'")
        self.evidence('actual_cli_cancellation_reaped',dict(event_loop_ticks=ticks,psql_reaped=True))


if __name__=='__main__':unittest.main()
