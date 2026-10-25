#!/usr/bin/env python3
"""Explicit, finite local CI and fresh semantic consumers. Never installs or launches a browser."""
import argparse
import gzip
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

PLATFORM = Path(__file__).resolve().parents[2]
REPO = PLATFORM.parent
CI = PLATFORM / 'tests/ci'
CONSUMERS = ['test_composed_semantic_projection.mjs', 'test_semantic_validation_fail_closed.mjs']
ARTIFACT = 'test_frontend_generated_css.mjs'
PRODUCTS = ('frontend/', 'platform-integrations/', 'scripts/', 'contracts/')
BASE = '862bbc8b6f48bcfdfe2f0a8319e3a2fb80b7f557'
# Original bytes, not reconstructed substitutes. The test suite also checks these hashes.
BASELINES = {
    'WEEBDEX_UPLOAD_REFUSAL_BASELINE_SOURCE': (BASE, 'platform-integrations/public-api/nostr-upload-request.ts', '9418845c867fd7fd0706756197900221e3cd73cd87a48133d5ed3ff6ef0d3056'),
    'WEEBDEX_UPLOAD_OBSERVER_BASELINE_SOURCE': (BASE, 'scripts/public-upload-recovery-observer.mjs', '2536bb20ef35db77f697ed30bb1eb14ad00bf8a2828eaa12d490bf32666b4c71'),
    'WEEBDEX_UPLOAD_GROUP_BASELINE_SOURCE': ('2d654f245d86967dd28e324d171b3fafb0528778', 'scripts/public-upload-recovery-browser.mjs', 'f32c89481ee6cfedddcda1ee1b1a692dbeb89a2e74d209ce06597b99f16146f1'),
    'WEEBDEX_UPLOAD_SECOND_BEGIN_BASELINE_SOURCE': ('f74bbb090aa015f440e34bef7b6e79ddb1774b1e', 'scripts/public-upload-recovery-browser.mjs', '808d27e183168c5b53f2e37317789b79d21601b75eb515c53d68824766605307'),
    'WEEBDEX_KEY_IMPORT_DRIVER_BASELINE_SOURCE': ('d6bfb53b8a9edc38e2c2e1166a0aea350b986b5f', 'scripts/public-upload-key-import.mjs', '2607cbd87ea2969cb030ea068e342045427f68483a3338be3039977c3faff84e'),
    'WEEBDEX_KEY_IMPORT_LEDGER_BASELINE_SOURCE': ('aca99b513a8af6bdae1bc9ec6a3b91e4cce8b099', 'scripts/public-upload-key-import.mjs', 'd0db904c9961e41e66e708ddf9d8672073d90b965dbe81c1a930f8d9808a694b'),
    'WEEBDEX_PUBLIC_UPLOAD_BASELINE_SOURCE': (BASE, 'scripts/public-upload-recovery-browser.mjs', '20346189278b318c55aa08a41c60ff6ce0da639746341cae8de1439bf8d439cb'),
}


RETAINED = PLATFORM / 'tests/fixtures/qualification-baselines'
RETAINED_MANIFEST_SHA256 = '7e9d0a0407aa1438ed21f359b9e53e198135ba4979efe7939fe247f1a3f75a89'
RESTORE_FILES = (
    'local-library/LocalLibraryPanel.svelte', 'local-library/device-list-name.ts',
    'local-library/device-store.ts', 'local-library/library.ts',
    'local-library/protocol/codec.js', 'local-library/protocol/limits.js',
    'public-api/device-id-migration.ts', 'public-api/device-id-state.ts',
    'public-api/device-write-v2.ts', 'public-api/library-staging.ts', 'public-api/validation.ts',
)


def retained_baselines(read=None):
    """Read immutable historical originals without Git; no current-source fallback."""
    read = read or Path.read_bytes
    encoded = read(RETAINED / 'manifest.json')
    if digest(encoded) != RETAINED_MANIFEST_SHA256:
        raise ValueError('Retained baseline manifest SHA-256 changed')
    manifest = json.loads(encoded)
    expected = set(BASELINES) | {'native-user-site-client'} | {
        'device-restore/platform-integrations/' + name for name in RESTORE_FILES}
    if (set(manifest) != {'schema', 'entries'} or
            manifest['schema'] != 'retained-qualification-counterfactuals-v1' or
            set(manifest['entries']) != expected):
        raise ValueError('Retained baseline manifest identities changed')
    result = {}
    for name, entry in manifest['entries'].items():
        if set(entry) != {'commit', 'path', 'git_blob', 'bytes', 'sha256', 'file'}:
            raise ValueError('Retained baseline provenance fields changed: ' + name)
        relative = Path(entry['file'])
        if relative.is_absolute() or '..' in relative.parts or relative.suffix != '.txt':
            raise ValueError('Unsafe retained source path: ' + name)
        source = RETAINED / relative
        if source.is_symlink() or source.resolve().parent != source.parent.resolve():
            raise ValueError('Unexpected retained source symlink: ' + name)
        data = read(source)
        blob = hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest()
        if len(data) != entry['bytes'] or digest(data) != entry['sha256'] or blob != entry['git_blob']:
            raise ValueError('Retained source bytes, SHA-256 or Git blob changed: ' + name)
        if name in BASELINES:
            commit, path, sha = BASELINES[name]
            if (entry['commit'], entry['path'], entry['sha256']) != (commit, 'weebdex-platform/' + path, sha):
                raise ValueError('Historical source identity differs: ' + name)
        result[name] = (entry, data)
    return result


def materialize_baselines(work):
    """Validate every original before creating a new owned fixture-only tree."""
    retained = retained_baselines()
    directory = work / 'baselines'
    directory.mkdir(parents=True, exist_ok=False)
    env, records = {}, {}
    for name, (entry, data) in retained.items():
        if name in BASELINES:
            target = directory / name / Path(entry['path']).name
            env[name] = str(target)
        elif name.startswith('device-restore/'):
            target = directory / 'device' / name.removeprefix('device-restore/')
        else:
            continue  # Native-user test verifies its retained fixture directly.
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        records[name] = dict(entry)
    env['WEEBDEX_DEVICE_RESTORE_BASELINE_ROOT'] = str(directory / 'device')
    records['WEEBDEX_DEVICE_RESTORE_BASELINE_ROOT'] = {
        'commit': BASE, 'tree': 'weebdex-platform/platform-integrations',
        'scope': 'exact 11-file closure used by the existing finite restore fixture',
        'files': ['platform-integrations/' + name for name in RESTORE_FILES],
    }
    write(work / 'baseline-provenance.json', {
        'retained_manifest_sha256': RETAINED_MANIFEST_SHA256, 'records': records})
    if not all(env.get(name) and Path(env[name]).is_file() for name in BASELINES):
        raise ValueError('Every required historical control must receive its exact source')
    return env


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':')).encode()


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n')


def git(*args):
    return subprocess.check_output(['git', *args], cwd=REPO)


def inventory():
    names = git('ls-files', '-z', '--', 'weebdex-platform').decode().split('\0')
    return {name: digest((REPO / name).read_bytes()) for name in names if name and
            name.removeprefix('weebdex-platform/').startswith(PRODUCTS + ('tests/ci/', 'tests/fixtures/', 'docs/fixtures/'))}


def discover():
    files = sorted(set(CI.glob('test_*.mjs')) | set(CI.glob('*.test.mjs')) | set(CI.glob('test_*.py')))
    return [{'path': str(path.relative_to(REPO)), 'sha256': digest(path.read_bytes()),
             'phase': 'strict-consumer' if path.name in CONSUMERS else 'artifact' if path.name == ARTIFACT
             else 'python' if path.suffix == '.py' else 'node'} for path in files]


def link(target, source):
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists() or target.is_symlink():
        if target.resolve(strict=True) != source.resolve(strict=True):
            raise ValueError('Refusing to replace an existing dependency path: ' + str(target))
    else:
        target.symlink_to(source.resolve(strict=True), target_is_directory=True)


def setup(config, work):
    required = ['frontend', 'dom_node_modules', 'noble_stage', 'scure_stage', 'bits_dialog_sources', 'playwright_core', 'svelte_runtime_sources']
    paths = {}
    for key in required:
        path = Path(config[key])
        if not path.is_absolute() or not path.is_dir():
            raise ValueError('Explicit existing absolute prerequisite required: ' + key)
        paths[key] = path.resolve(strict=True)
    expected = [(paths['frontend'] / 'node_modules/svelte', '5.55.1'),
                (paths['frontend'] / 'node_modules/typescript', '5.9.3'),
                (paths['dom_node_modules'] / 'jsdom', '27.0.1'),
                (paths['dom_node_modules'] / 'fake-indexeddb', '6.2.4'),
                (paths['playwright_core'], '1.62.1')]
    for package, version in expected:
        if json.loads((package / 'package.json').read_text())['version'] != version:
            raise ValueError('Pinned dependency version differs: ' + str(package))
    for name, expected_hash in {'dialog.svelte.ts': 'a68947871a8e0b8b2471a77158359d62841a216e9c19170634131d15ec2299b9',
                                'dialog-trigger.svelte': 'eecde0ffeea2523e75cd318a65bf4c66577e8d9481163717a892b112c36dc8f3'}.items():
        if digest((paths['bits_dialog_sources'] / name).read_bytes()) != expected_hash:
            raise ValueError('Official pinned Bits UI source differs: ' + name)
    pins = json.loads((CI / 'svelte-review-runtime-pins.json').read_text())
    for name, sha in pins['files'].items():
        if digest((paths['svelte_runtime_sources'] / name).read_bytes()) != sha:
            raise ValueError('Pinned Svelte Git-runtime source differs: ' + name)
    link(work / 'inputs/frontend', paths['frontend'])
    link(work / 'inputs/local-library-noble-2.0.1', paths['noble_stage'])
    for name, key in [('local-library-noble-2.0.1', 'noble_stage'), ('local-signing-base-2.0.0', 'scure_stage')]:
        link(PLATFORM / '.build' / name, paths[key])
    # A few existing tests load these original-client fixtures by their real path.
    modules = REPO / 'manga-protocol-lab/browser-client/node_modules'
    if modules.is_symlink():
        raise ValueError('Original-client dependency container must be owned, not a shared symlink')
    modules.mkdir(exist_ok=True)
    for name in ['jsdom', 'fake-indexeddb']:
        link(modules / name, paths['dom_node_modules'] / name)
    for name in ['@noble/curves', '@noble/hashes']:
        link(modules / name, paths['noble_stage'] / 'node_modules' / name)
    env = dict(os.environ)
    # Do not inherit stale source substitutions, skips, external targets or guards.
    for key in list(env):
        if key.startswith('WEEBDEX_') or key in ('NODE_OPTIONS', 'PUBLIC_MANGADEX_CATALOGUE', 'PUBLIC_API_ORIGIN', 'PUBLIC_CLIENT_ORIGIN'):
            del env[key]
    env.update(WEEBDEX_FRONTEND_ROOT=str(work / 'inputs/frontend'),
               WEEBDEX_BROWSER_DOM_NODE_MODULES=str(paths['dom_node_modules']),
               WEEBDEX_MIGRATION_TEST_NODE_MODULES=str(paths['dom_node_modules']),
               WEEBDEX_MIGRATION_TEST_NOBLE=str(paths['noble_stage'] / 'node_modules/@noble/hashes'),
               WEEBDEX_SVELTE_REVIEW_RUNTIME_ROOT=str(paths['svelte_runtime_sources']),
               WEEBDEX_SELECT_SOURCE_NODE_MODULES=str(paths['frontend'] / 'node_modules'),
               WEEBDEX_SEMANTIC_NODE_MODULES=str(paths['frontend'] / 'node_modules'),
               WEEBDEX_LOAD_NODE_MODULES=str(paths['dom_node_modules']),
               WEEBDEX_BITS_DIALOG_SOURCE_DIR=str(paths['bits_dialog_sources']),
               WEEBDEX_PLAYWRIGHT_CORE_ROOT=str(paths['playwright_core']),
               WEEBDEX_NATIVE_SETTINGS_SCHEMA=str(PLATFORM / 'contracts/native-settings-template-v1.json'),
               PYTHON=sys.executable)
    env.update(materialize_baselines(work))
    roots = [paths['frontend'] / 'node_modules', paths['dom_node_modules'], paths['noble_stage'] / 'node_modules',
             paths['scure_stage'] / 'node_modules', paths['playwright_core'], paths['svelte_runtime_sources'], modules]
    return env, roots


def physical_roots(initial):
    roots = {str(path.resolve(strict=True)) for path in initial}
    pending = list(roots)
    while pending:
        root = pending.pop()
        for parent, dirs, files in os.walk(root, followlinks=False):
            for name in dirs + files:
                entry = Path(parent) / name
                if not entry.is_symlink():
                    continue
                target = entry.resolve(strict=True)
                if any(target == Path(candidate) or Path(candidate) in target.parents for candidate in roots):
                    continue
                text = str(target)
                if '/node_modules/' not in text:
                    raise ValueError('Dependency symlink outside a package root: ' + str(entry))
                actual = text.split('/node_modules/', 1)[0] + '/node_modules'
                if actual not in roots:
                    roots.add(actual)
                    pending.append(actual)
    return sorted(roots)


def snapshot(roots, out, phase):
    snapshots, summary = {}, {}
    for value in roots:
        root, manifest = Path(value), {}
        for parent, dirs, files in os.walk(root, followlinks=False):
            for name in sorted(dirs + files):
                path = Path(parent) / name
                stat = path.lstat()
                row = {'mode': stat.st_mode & 0o7777, 'mtimeNs': stat.st_mtime_ns}
                if path.is_symlink():
                    row.update(kind='symlink', target=os.readlink(path))
                elif path.is_file():
                    row.update(kind='file', bytes=stat.st_size, sha256=digest(path.read_bytes()))
                elif path.is_dir():
                    row.update(kind='directory')
                else:
                    raise ValueError('Unexpected dependency entry: ' + str(path))
                manifest[str(path.relative_to(root))] = row
        snapshots[value] = manifest
        summary[value] = {'entries': len(manifest), 'regularFiles': sum(row['kind'] == 'file' for row in manifest.values()),
                          'canonicalManifestSha256': digest(canonical(manifest))}
    encoded = canonical(snapshots)
    (out / ('physical-dependencies-' + phase + '.json.gz')).write_bytes(gzip.compress(encoded, mtime=0))
    write(out / ('physical-dependencies-' + phase + '-summary.json'), summary)
    if phase != 'before' and gzip.decompress((out / 'physical-dependencies-before.json.gz').read_bytes()) != encoded:
        raise ValueError('Physical dependency bytes, mode, mtime or symlink changed')


def bounded(command, cwd, env, out, name, deadline=600, evidence_roots=()):
    spec = importlib.util.spec_from_file_location('combined_process_owner', CI / 'qualification_process_owner.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.run(command, cwd, env, out, name, deadline, evidence_roots)


def audit(env, roots, out, phase):
    directory = out / 'cache-audit' / phase
    directory.mkdir(parents=True)
    return dict(env, NODE_OPTIONS='--max-old-space-size=2048 --require=' + str(PLATFORM / 'scripts/semantic-readonly-cache-guard.cjs'),
                WEEBDEX_READONLY_CACHE_ROOTS=json.dumps(roots), WEEBDEX_READONLY_AUDIT_DIR=str(directory))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path, help='New directory; existing evidence is never overwritten')
    parser.add_argument('--phase', choices=['finite', 'strict', 'artifact', 'all'], default='all')
    parser.add_argument('--export-manifest', type=Path, help='Explicit externally pinned source export; finite phase only')
    parser.add_argument('--export-manifest-sha256', help='Independent expected manifest SHA-256, never taken from the manifest')
    args = parser.parse_args()
    exported = None
    if args.export_manifest is not None or args.export_manifest_sha256 is not None:
        if args.export_manifest is None or args.export_manifest_sha256 is None:
            parser.error('Both export manifest and independent SHA-256 are required')
        if args.phase != 'finite':
            parser.error('Source-export mode supports only --phase finite; strict/artifact/all still require Git')
        if args.output.resolve().is_relative_to(REPO.resolve()):
            parser.error('Export-mode evidence output must be outside the source tree')
        sys.dont_write_bytecode = True
        spec = importlib.util.spec_from_file_location('qualification_source_export', CI / 'qualification_source_export.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        exported = module.ExportSource(REPO, args.export_manifest, args.export_manifest_sha256)
        exported.verify()  # Refuse incomplete, altered or ambiguous sources before any setup.
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    work = PLATFORM / '.build' / ('combined-' + out.name)
    work.mkdir(parents=True, exist_ok=False)
    state = {'status': 'running', 'scope': 'platform CI finite tests; strict semantic projections; explicit fresh CSS artifact',
             'phase': args.phase, 'work': str(work),
             'results': {}, 'browser_executed': False, 'capacity_qualified': False}
    if exported is None:
        state['sourceCommit'] = git('rev-parse', 'HEAD').decode().strip()
    else:
        state['sourceProvenance'] = exported.provenance()
    write(out / 'status.json', state)
    try:
        config = json.loads(args.config.read_text())
        env, initial = setup(config, work)
        if exported is not None:
            env['PYTHONDONTWRITEBYTECODE'] = '1'
            exported.bind_dependency_controls(config['frontend'])
            write(out / 'export-dependency-controls-before.json', exported.dependency_controls_before)
            initial.append(Path(config['bits_dialog_sources']).resolve(strict=True))
            exported.bind_runtime([work, PLATFORM / '.build/local-library-noble-2.0.1',
                PLATFORM / '.build/local-signing-base-2.0.0', REPO / 'manga-protocol-lab/browser-client/node_modules'])
            write(out / 'export-runtime-pins-before.json', exported.runtime_before)
        roots = physical_roots(initial)
        write(out / 'physical-dependency-root-discovery.json', {'roots': roots, 'uncoveredSymlinks': []})
        before = exported.verify() if exported is not None else inventory()
        write(out / 'execution-source-pins-before.json', before)
        products = {name: sha for name, sha in before.items() if name.removeprefix('weebdex-platform/').startswith(PRODUCTS)}
        write(out / 'strict-product-source-pins.json', products)
        discovery = discover()
        write(out / 'discovery.json', discovery)
        snapshot(roots, out, 'before')
        if args.phase in ('finite', 'all'):
            finite_env = audit(env, roots, out, 'finite')
            if exported is not None:
                # Only the fully verified export branch grants this scoped
                # provenance context. setup() discarded all inherited values.
                finite_env['WEEBDEX_FINITE_EXPORT_MANIFEST'] = str(exported.manifest_path)
                finite_env['WEEBDEX_FINITE_EXPORT_MANIFEST_SHA256'] = exported.expected_sha256
            nodes = [row['path'] for row in discovery if row['phase'] == 'node']
            state['results']['node'] = bounded(['node', '--experimental-vm-modules', '--test', '--test-concurrency=1', '--test-reporter=tap', *nodes], REPO, finite_env, out, 'node')['status']
            script = "import json,sys,unittest; suite=unittest.defaultTestLoader.discover(sys.argv[1],pattern='test_*.py'); result=unittest.TextTestRunner(verbosity=2).run(suite); report={'testsRun':result.testsRun,'failures':len(result.failures),'errors':len(result.errors),'skipped':[{'test':str(t),'reason':r} for t,r in result.skipped]}; open(sys.argv[2],'w').write(json.dumps(report,indent=2)+'\\n'); sys.exit(not result.wasSuccessful())"
            state['results']['python'] = bounded([sys.executable, '-c', script, str(CI), str(out / 'python-results.json')], REPO, finite_env, out, 'python')['status']
            # Expose every existing framework skip rather than describing it as a pass.
            state['node_skip_lines'] = [line.strip() for line in (out / 'node.log').read_text().splitlines() if '# SKIP' in line]
            state['python_skips'] = json.loads((out / 'python-results.json').read_text())['skipped'] if (out / 'python-results.json').exists() else None
        if args.phase in ('strict', 'all'):
            strict_env = audit(env, roots, out, 'strict-physical')
            generations = [work / ('strict-' + str(gate)) for gate in (0, 1)]
            driver = work / 'strict-driver.py'
            driver.write_text("import os,subprocess,sys\ncode=0\nfor gate in (0,1):\n env=dict(os.environ,PUBLIC_MANGADEX_CATALOGUE=str(gate),WEEBDEX_SEMANTIC_OUTPUT=sys.argv[1+gate])\n print('STRICT_GATE '+str(gate),flush=True)\n result=subprocess.run(['node','scripts/check-composed-frontend.mjs'],env=env)\n code = code or result.returncode\nsys.exit(code)\n")
            strict = bounded([sys.executable, str(driver), *map(str, generations)], PLATFORM, strict_env, out, 'strict', evidence_roots=[work])
            state['results']['strict'] = strict['status']
            for gate, generation in enumerate(generations):
                for name in ['summary.json', 'diagnostics.json', 'cached-toolchain.json', 'service-worker-tsc-receipt.json', 'svelte-check-receipt.json', 'svelte-kit-sync-receipt.json']:
                    source = generation / 'evidence' / name
                    if source.exists():
                        shutil.copyfile(source, out / ('strict-' + str(gate) + '-' + name))
            records = []
            for path in sorted((out / 'cache-audit/strict-physical').glob('*.json')):
                value = json.loads(path.read_text())
                records.append({'file': str(path.relative_to(out)), 'sha256': digest(path.read_bytes()), 'pid': value['pid'],
                                'roots': len(value['protected_roots']), 'checkedMutations': value['checked_node_mutations']})
            write(out / 'cache-audit-summary-physical.json', {'recordsManifest': records})
            snapshot(roots, out, 'after')
            if strict['status'] == 'passed':
                consumer_env = audit(env, roots, out, 'consumers')
                for gate, generation in enumerate(generations):
                    for consumer in CONSUMERS:
                        name = 'consumer-' + str(gate) + '-' + consumer.removesuffix('.mjs')
                        state['results'][name] = bounded(['node', str(CI / consumer), str(generation), str(out)], PLATFORM, consumer_env, out, name)['status']
            else:
                state['results']['semantic-consumers'] = 'not-run: fresh strict baseline did not pass'
        if args.phase in ('artifact', 'all'):
            for key in ['artifact_root', 'artifact_manifest']:
                if key not in config or not Path(config[key]).is_absolute():
                    raise ValueError('Explicit fresh artifact prerequisite required: ' + key)
            artifact_env = audit(dict(env, WEEBDEX_FRONTEND_ARTIFACT_ROOT=config['artifact_root'],
                                      WEEBDEX_FRONTEND_ARTIFACT_MANIFEST=config['artifact_manifest']), roots, out, 'artifact')
            state['results']['artifact'] = bounded(['node', '--test', str(CI / ARTIFACT)], REPO, artifact_env, out, 'artifact')['status']
        snapshot(roots, out, 'after-final')
        after = exported.verify() if exported is not None else inventory()
        if exported is not None:
            write(out / 'export-runtime-pins-after.json', exported.runtime_inventory())
            write(out / 'export-dependency-controls-after.json', exported.dependency_control_inventory())
        write(out / 'execution-source-pins-after.json', after)
        if before != after:
            raise ValueError('Qualification source inputs changed during execution')
        state['status'] = 'passed' if state['results'] and all(value == 'passed' for value in state['results'].values()) else 'failed'
        if state['status'] == 'passed' and (state.get('node_skip_lines') or state.get('python_skips')):
            state['status'] = 'passed-with-explicit-skips'
        state['not_selected_phases'] = [phase for phase in ['finite', 'strict', 'artifact'] if args.phase not in (phase, 'all')]
    except BaseException as error:
        state['status'] = 'failed'
        state['error'] = str(error)
        raise
    finally:
        write(out / 'status.json', state)
        print(json.dumps(state, indent=2))
    return 0 if state['status'].startswith('passed') else 1


if __name__ == '__main__':
    sys.exit(main())
