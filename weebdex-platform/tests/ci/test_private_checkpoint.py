"""Finite filesystem/command-boundary tests. No database, listener or pg tool runs.
Generated small synthetic directories are retained for inspection, not auto-deleted.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import re
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[2]/'scripts/private-checkpoint.py'
spec = importlib.util.spec_from_file_location('private_checkpoint', SCRIPT)
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class PrivateCheckpointTests(unittest.TestCase):
    def setUp(self):
        self.base = Path(tempfile.mkdtemp(prefix='synthetic-private-checkpoint-'))
        self.source = self.base/'source'; self.source.mkdir()
        self.roots = {key:self.source/key for key in ['native','originals','projection']}
        for key, root in self.roots.items():
            root.mkdir(); (root/'nested').mkdir()
            (root/'nested'/'original.bin').write_bytes((key+'\0opaque\n').encode())
        self.config = self.base/'platform.json'
        self.release = self.base/'release-manifest.json'
        self.release.write_text('{"syntheticRelease":"test-only"}\n')
        self.write_config(self.config, self.roots)
        self.args = argparse.Namespace(config=str(self.config), node_config=None,
            release_manifest=str(self.release), bundle=str(self.base/'private-backup'),
            database_env='SYNTHETIC_PRIVATE_CHECKPOINT_DSN', execute=True, quiesced=True,
            timeout=30, max_files=100, max_bytes=1024*1024, min_free_bytes=0,
            confirm_empty_database='synthetic_target')
        self.env = patch.dict(os.environ, {self.args.database_env:
            'postgresql://synthetic:synthetic%20password@db.example.invalid/synthetic_target?sslmode=verify-full'})
        self.env.start(); self.addCleanup(self.env.stop)
        self.calls = []

    def write_config(self, path, roots):
        path.write_text(json.dumps({'production':True,'bind':'127.0.0.1:8080',
            'ui_dir':str(self.base/'public-ui'), 'uploads_dir':str(roots['native']),
            'protocol_publication':{'storage':{'path':str(roots['originals'])},
                'operator_key_file':str(self.base/'separate-operator.key'),
                'native_node':{'bind':'127.0.0.1:8081','service_key_file':str(self.base/'separate-service.key'),
                    'backend':{'data_dir':str(roots['projection'])}}}}))

    def fake_tool(self, name, argv, env, budget, capture=False):
        self.calls.append((name, list(argv)))
        self.assertFalse(any('synthetic password' in value or 'postgresql://' in value for value in argv))
        self.assertEqual(env['PGDATABASE'], 'synthetic_target')
        self.assertEqual(env['PGPASSWORD'], 'synthetic password')
        self.assertEqual(env['PGSSLMODE'], 'verify-full')
        self.assertIn('--no-password', argv)
        if name == 'pg_dump':
            Path(argv[argv.index('--file')+1]).write_bytes(b'SYNTHETIC-NOT-A-REAL-PG-DUMP')
        elif name == 'psql':
            self.assertTrue(capture); return 'synthetic_target\t0'
        elif name == 'pg_restore':
            self.assertEqual(argv[argv.index('--dbname')+1], 'synthetic_target')
            self.assertIn('--single-transaction', argv)
            self.assertIn('--exit-on-error', argv)
            self.assertNotIn('--clean', argv); self.assertNotIn('--create', argv)
        else:
            self.fail('Unexpected tool')
        return ''

    def backup(self):
        with patch.object(m, 'stopped'):
            return m.backup(self.args, self.fake_tool)

    def restored_args(self):
        parent = self.base/'new-storage'; parent.mkdir()
        roots = {key:parent/key for key in self.roots}
        config = self.base/'restore-platform.json'; self.write_config(config, roots)
        return argparse.Namespace(**{**vars(self.args),'config':str(config)}), roots

    def test_default_cli_plan_runs_no_tools_and_creates_no_bundle(self):
        value = subprocess.run([sys.executable, str(SCRIPT), 'backup', '--config',str(self.config),
            '--release-manifest',str(self.release),'--bundle',self.args.bundle],
            env={**os.environ,'PATH':'/nonexistent'}, capture_output=True, text=True, timeout=5)
        self.assertEqual(value.returncode,0,value.stdout)
        self.assertTrue(json.loads(value.stdout)['dryRun'])
        self.assertNotIn('synthetic password',value.stdout)
        self.assertFalse(Path(self.args.bundle).exists())

    def test_execute_requires_explicit_quiescence_before_tool_or_output(self):
        self.args.quiesced = False
        with self.assertRaises(m.Refused): self.backup()
        self.assertEqual(self.calls,[]); self.assertFalse(Path(self.args.bundle).exists())

    def test_fake_round_trip_preserves_all_three_exact_storage_roots(self):
        self.assertTrue(self.backup()['complete'])
        bundle = Path(self.args.bundle)
        self.assertEqual(bundle.stat().st_mode & 0o777,0o700)
        manifest = m.verify(bundle,m.Budget(max_files=9,max_bytes=1024*1024))
        self.assertEqual(len(manifest['entries']),9)
        self.assertNotIn('synthetic password',(bundle/'manifest.json').read_text())
        args, roots = self.restored_args()
        with patch.object(m,'stopped'):
            result = m.restore(args,self.fake_tool)
        self.assertTrue(result['complete']); self.assertFalse(result['signingKeysRestored'])
        for key, root in roots.items():
            self.assertEqual((root/'nested/original.bin').read_bytes(),(self.roots[key]/'nested/original.bin').read_bytes())
            self.assertEqual((root/'nested/original.bin').stat().st_mode & 0o777,0o600)
        self.assertEqual([name for name,_ in self.calls],['pg_dump','psql','pg_restore'])

    def test_restore_plan_verifies_but_never_runs_tool_or_makes_target(self):
        self.backup(); self.calls.clear()
        args, roots = self.restored_args(); args.execute=False
        self.assertTrue(m.restore(args,self.fake_tool)['dryRun'])
        self.assertEqual(self.calls,[]); self.assertTrue(all(not p.exists() for p in roots.values()))

    def test_nonempty_database_refused_before_creating_storage(self):
        self.backup(); args, roots = self.restored_args()
        with patch.object(m,'stopped'), self.assertRaises(m.Refused):
            m.restore(args,lambda *unused:'synthetic_target\t1')
        self.assertTrue(all(not p.exists() for p in roots.values()))

    def test_database_name_mismatch_refused(self):
        self.backup(); args,_ = self.restored_args()
        with patch.object(m,'stopped'), self.assertRaises(m.Refused):
            m.restore(args,lambda *unused:'other_database\t0')

    def test_existing_storage_refused_before_database_command(self):
        self.backup(); args, roots = self.restored_args(); roots['native'].mkdir()
        self.calls.clear()
        with self.assertRaises(m.Refused): m.restore(args,self.fake_tool)
        self.assertEqual(self.calls,[])

    def test_incomplete_output_is_never_accepted(self):
        bundle=Path(self.args.bundle); bundle.mkdir(mode=0o700)
        (bundle/'database.dump').write_bytes(b'partial')
        with self.assertRaises(m.Refused): m.verify(bundle,m.Budget())

    def test_corrupted_original_or_dump_refused(self):
        self.backup(); bundle=Path(self.args.bundle)
        (bundle/'storage/originals/nested/original.bin').write_bytes(b'corrupt')
        with self.assertRaises(m.Refused): m.verify(bundle,m.Budget())
        (bundle/'database.dump').write_bytes(b'corrupt')
        with self.assertRaises(m.Refused): m.verify(bundle,m.Budget())

    def test_traversal_is_refused_even_with_recomputed_completion_hash(self):
        self.backup(); bundle=Path(self.args.bundle)
        path=bundle/'manifest.json'; manifest=json.loads(path.read_text())
        manifest['entries'][0]['path']='../escape'; path.write_text(json.dumps(manifest))
        (bundle/'COMPLETE.json').write_text(json.dumps({'manifestSha256':hashlib.sha256(path.read_bytes()).hexdigest()}))
        with self.assertRaises(m.Refused): m.verify(bundle,m.Budget())

    def test_unaccounted_member_refused(self):
        self.backup(); bundle=Path(self.args.bundle)
        (bundle/'storage/native/unrecorded').write_bytes(b'x')
        with self.assertRaises(m.Refused): m.verify(bundle,m.Budget())

    def test_symlink_and_hardlink_sources_refused(self):
        (self.roots['native']/'link').symlink_to(self.release)
        with self.assertRaises(m.Refused): self.backup()
        # A separate source keeps the symlink fixture intact for inspection.
        other=self.base/'hardlink-root'; other.mkdir()
        os.link(self.release,other/'linked-file')
        with self.assertRaises(m.Refused): m.scan({'test':other},m.Budget())

    def test_key_storage_overlap_refused(self):
        config=json.loads(self.config.read_text())
        config['protocol_publication']['operator_key_file']=str(self.roots['originals']/'private.key')
        self.config.write_text(json.dumps(config))
        with self.assertRaises(m.Refused): self.backup()

    def test_platform_config_inside_storage_refused_before_tools(self):
        inside=self.roots['native']/'platform-private.json'
        inside.write_bytes(self.config.read_bytes()); self.args.config=str(inside)
        with self.assertRaisesRegex(m.Refused,'Configuration files'):
            self.backup()
        self.assertEqual(self.calls,[])
        self.assertFalse(Path(self.args.bundle).exists())

    def test_original_config_inside_storage_refused_before_tools(self):
        config=json.loads(self.config.read_text())
        node=config['protocol_publication']['native_node']
        node['storage']={'path':str(self.roots['originals'])}
        inside=self.roots['projection']/'original-private.json'
        inside.write_text(json.dumps(node)); self.args.node_config=str(inside)
        with self.assertRaisesRegex(m.Refused,'Configuration files'):
            self.backup()
        self.assertEqual(self.calls,[])
        self.assertFalse(Path(self.args.bundle).exists())

    def test_same_size_original_change_during_dump_never_completes_checkpoint(self):
        changed=self.roots['originals']/'nested/original.bin'
        before=changed.read_bytes()
        def mutate_after_dump(name,argv,env,budget,capture=False):
            answer=self.fake_tool(name,argv,env,budget,capture)
            if name=='pg_dump': changed.write_bytes(bytes([before[0]^1])+before[1:])
            return answer
        with patch.object(m,'stopped'),self.assertRaises(m.Refused):
            m.backup(self.args,mutate_after_dump)
        self.assertEqual(changed.stat().st_size,len(before))
        self.assertFalse((Path(self.args.bundle)/'COMPLETE.json').exists())

    def test_production_tls_downgrade_refused_without_tool(self):
        os.environ[self.args.database_env]='host=db.example.invalid dbname=synthetic_target sslmode=require'
        with self.assertRaises(m.Refused): self.backup()
        self.assertEqual(self.calls,[])

    def test_official_libpq_parses_escaped_keyword_credentials_without_connecting(self):
        os.environ[self.args.database_env]="host=db.example.invalid dbname=synthetic_target password='synthetic\\ password' sslmode=verify-full"
        env=m.pg_environment({'production':True},self.args.database_env)
        self.assertEqual(env['PGPASSWORD'],'synthetic password')
        self.assertEqual(env['PGDATABASE'],'synthetic_target')

    def test_source_release_mismatch_refused(self):
        self.backup(); args,_=self.restored_args()
        self.release.write_text('{"syntheticRelease":"different"}')
        with self.assertRaises(m.Refused): m.restore(args,self.fake_tool)

    def test_file_byte_deadline_and_reserve_bounds(self):
        for budget in [m.Budget(max_files=1),m.Budget(max_bytes=1)]:
            with self.assertRaises(m.Refused): m.scan(self.roots,budget)
        budget=m.Budget(); budget.end=0
        with self.assertRaises(m.Refused): budget.check()
        budget=m.Budget(min_free=2**62,filesystem=self.base)
        with self.assertRaises(m.Refused): budget.check(writing=True)

    def test_failed_dump_leaves_incomplete_output_and_no_false_success(self):
        def failed(*unused): raise m.Refused('Synthetic tool failure')
        with patch.object(m,'stopped'),self.assertRaises(m.Refused): m.backup(self.args,failed)
        self.assertTrue(Path(self.args.bundle).is_dir())
        self.assertFalse((Path(self.args.bundle)/'COMPLETE.json').exists())


class RecoveryRuntimeBoundaries(unittest.TestCase):
    """Wrapper stand-ins and source contracts, never application-start acceptance."""
    def setUp(self):
        self.base=Path(tempfile.mkdtemp(prefix='synthetic-recovery-boundary-'))
        self.wrapper=SCRIPT.parent/'run-platform.sh'
        self.capture=self.base/'arguments.txt'
        self.binary=self.base/'synthetic-boundary.sh'
        self.binary.write_text('#!/bin/sh\nprintf \'%s\\n\' "$#" "$1" > "$SYNTHETIC_CAPTURE"\n')
        self.binary.chmod(0o700)
        self.env={'PATH':'/usr/bin:/bin','LANG':'C','PLATFORM_CONFIG':str(self.base/'synthetic-config.json'),
                  'PLATFORM_BIN':str(self.binary),'SYNTHETIC_CAPTURE':str(self.capture)}

    def invoke(self,*args,env=None):
        return subprocess.run(['bash',str(self.wrapper),*args],env=env or self.env,
                              capture_output=True,text=True,timeout=5)

    def test_wrapper_forwards_only_exact_reviewed_action_to_standin(self):
        for action in ['check-config','verify','migrate','serve']:
            value=self.invoke(action,env={**self.env,'PLATFORM_DATABASE_URL':'dbname=synthetic_boundary'})
            self.assertEqual(value.returncode,0,value.stderr)
            self.assertEqual(self.capture.read_text().splitlines(),['1',action])
        value=self.invoke(env={**self.env,'MANGA_DATABASE_URL':'dbname=synthetic_boundary'})
        self.assertEqual(value.returncode,0,value.stderr)
        self.assertEqual(self.capture.read_text().splitlines(),['1','serve'])

    def test_wrapper_refuses_unconfigured_database_unknown_or_extra_action(self):
        self.assertNotEqual(self.invoke('verify').returncode,0)
        self.assertEqual(self.invoke('unexpected').returncode,64)
        self.assertEqual(self.invoke('verify','unexpected').returncode,64)
        env={k:v for k,v in self.env.items() if k!='PLATFORM_CONFIG'}
        self.assertNotEqual(self.invoke('check-config',env=env).returncode,0)
        self.assertFalse(self.capture.exists())

    def test_graceful_signal_drain_and_service_deadline_source_contract(self):
        root=SCRIPT.parents[1]
        source=(root/'backend/src/main.rs').read_text()
        self.assertLess(source.index('if action == "verify"'),source.index('configure_publication'))
        self.assertIn('with_graceful_shutdown',source)
        self.assertIn('SignalKind::terminate()',source)
        self.assertIn('tokio::signal::ctrl_c()',source)
        self.assertLess(source.index('state.runtime.begin_draining();'),source.index('let _ = shutdown_sender.send(())'))
        self.assertIn('state.requests.available_permits() != state.config.maximum_requests',source)
        unit=(root/'deployment/weebdex.service.example').read_text()
        self.assertIn('KillSignal=SIGTERM',unit)
        seconds=int(re.search(r'^TimeoutStopSec=(\d+)$',unit,re.M)[1])
        for name in ['platform.publisher.example.json','platform.read-only.example.json']:
            config=json.loads((root/'deployment'/name).read_text())
            self.assertGreater(seconds,config['shutdown_grace_seconds'])


if __name__ == '__main__': unittest.main()
