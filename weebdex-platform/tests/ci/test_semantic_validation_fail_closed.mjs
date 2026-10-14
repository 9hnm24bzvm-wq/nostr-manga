#!/usr/bin/env node
// Negative controls for checker evidence and read-only cache configuration.
// All probe files live under this checkout .build; production caches are untouched.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const work = path.resolve(process.argv[2] ?? path.join(root, '.build/composed-semantic-check-hardened2'));
const temporary = fs.mkdtempSync(path.join(root, '.build/semantic-negative-controls-'));
const validator = path.join(root, 'tests/ci/test_composed_semantic_projection.mjs');
const guard = path.join(root, 'scripts/semantic-readonly-cache-guard.cjs');
const results = [];
function fixture(name, mutate) {
  const destination = path.join(temporary, name); fs.mkdirSync(path.join(destination, 'evidence'), { recursive: true });
  for (const entry of fs.readdirSync(path.join(work, 'evidence'))) fs.symlinkSync(path.join(work, 'evidence', entry), path.join(destination, 'evidence', entry));
  for (const name of ['project', 'production-composed']) fs.symlinkSync(path.join(work, name), path.join(destination, name));
  const edit = (name, callback) => { const file = path.join(destination, 'evidence', name), value = JSON.parse(fs.readFileSync(file)); fs.unlinkSync(file); fs.writeFileSync(file, JSON.stringify(callback(value), null, 2) + '\n'); };
  mutate(edit);
  const checked = spawnSync(process.execPath, [validator, destination], { cwd: root, encoding: 'utf8', timeout: 30_000 });
  assert.ok(checked.status !== 0 && !checked.error, `Incomplete artifact fixture unexpectedly passed: ${name}`);
  const expectedFailure = { 'empty-canonical-inventory': 'Actual canonical inventory must contain 679 records', 'missing-map-manifest-entry': 'Actual manifest must contain 591 mapped application sources', 'empty-cache-inventory': 'Actual cache inventory must not be empty', 'no-cache-process-audits': 'At least one configured cache-guard process audit is required', 'empty-audit-protected-roots': 'Each audit must cover both exact dependency roots', 'empty-declared-cache-roots': 'deepStrictEqual' }[name];
  assert.ok(checked.stderr.includes(expectedFailure), `Negative control failed for an unrelated reason: ${name}\n${checked.stderr}`);
  results.push({ probe: name, status: 'rejected', exit_code: checked.status, first_error: checked.stderr.split('\n').find(line => line.includes('AssertionError')) ?? checked.stderr.split('\n')[0] });
}
fixture('empty-canonical-inventory', edit => edit('canonical-source-hashes.json', () => ({})));
fixture('missing-map-manifest-entry', edit => edit('projection-manifest.json', entries => entries.filter(entry => entry.projection !== 'src/app.d.ts')));
fixture('empty-cache-inventory', edit => { edit('cache-source-inventory.json', () => ({})); edit('isolation-and-fidelity-verification.json', value => ({ ...value, snapshot_entries: 0 })); });
fixture('no-cache-process-audits', edit => edit('isolation-and-fidelity-verification.json', value => ({ ...value, node_process_audits: [] })));
fixture('empty-audit-protected-roots', edit => edit('isolation-and-fidelity-verification.json', value => ({ ...value, node_process_audits: value.node_process_audits.map(audit => ({ ...audit, protected_roots: [] })) })));
fixture('empty-declared-cache-roots', edit => edit('isolation-and-fidelity-verification.json', value => ({ ...value, protected_cache_roots: [] })));
const protectedDirectory = path.join(temporary, 'protected-probe-cache'), auditDirectory = path.join(temporary, 'guard-audits');
fs.mkdirSync(protectedDirectory); fs.mkdirSync(auditDirectory);
const probeFile = path.join(protectedDirectory, 'must-not-exist');
const cleanEnvironment = { ...process.env }; delete cleanEnvironment.NODE_OPTIONS; delete cleanEnvironment.WEEBDEX_READONLY_CACHE_ROOTS; delete cleanEnvironment.WEEBDEX_READONLY_AUDIT_DIR;
function guardProbe(name, environment) {
  const checked = spawnSync(process.execPath, ['--require', guard, '-e', `require('node:fs').writeFileSync(${JSON.stringify(probeFile)}, 'forbidden')`], { cwd: root, env: { ...cleanEnvironment, ...environment }, encoding: 'utf8', timeout: 30_000 });
  assert.ok(checked.status !== 0 && !checked.error, `Unsafe cache guard configuration unexpectedly passed: ${name}`);
  const expectedFailure = { 'guard-missing-protected-roots': 'requires configured protected roots', 'guard-empty-protected-roots': 'requires a nonempty array', 'guard-missing-audit-directory': 'requires an existing absolute audit directory', 'guard-valid-root-blocks-write': 'Read-only dependency cache mutation denied' }[name];
  assert.ok(checked.stderr.includes(expectedFailure), `Guard control failed for an unrelated reason: ${name}`);
  assert.equal(fs.existsSync(probeFile), false, name);
  results.push({ probe: name, status: 'rejected', exit_code: checked.status });
}
guardProbe('guard-missing-protected-roots', { WEEBDEX_READONLY_AUDIT_DIR: auditDirectory });
guardProbe('guard-empty-protected-roots', { WEEBDEX_READONLY_CACHE_ROOTS: '[]', WEEBDEX_READONLY_AUDIT_DIR: auditDirectory });
guardProbe('guard-missing-audit-directory', { WEEBDEX_READONLY_CACHE_ROOTS: JSON.stringify([protectedDirectory]) });
guardProbe('guard-valid-root-blocks-write', { WEEBDEX_READONLY_CACHE_ROOTS: JSON.stringify([protectedDirectory]), WEEBDEX_READONLY_AUDIT_DIR: auditDirectory });
const audits = fs.readdirSync(auditDirectory).map(name => JSON.parse(fs.readFileSync(path.join(auditDirectory, name))));
assert.equal(audits.length, 1, 'Configured negative write must produce one guard receipt');
assert.equal(audits[0].protected_write_attempts, 1);
assert.deepEqual(audits[0].protected_roots, [protectedDirectory]);
const report = { status: 'passed', rejected_negative_controls: results.length, real_dependency_caches_written: false, results, guard_denied_write_receipt: audits[0] };
fs.writeFileSync(path.join(temporary, 'negative-controls.json'), JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify({ ...report, evidence: path.relative(root, path.join(temporary, 'negative-controls.json')) }));
