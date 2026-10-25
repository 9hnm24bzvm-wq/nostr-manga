// Explicit delayed-UI stand-ins. No DOM, browser, key, network or account starts.
import assert from 'node:assert/strict';
import { connectPublicUploadKey, createKeyImportStep, uniqueUploadControl } from '../../scripts/public-upload-key-import.mjs';
export function importFixture({ delayed = true, enrollmentOpen = false, backupSessionControl = false, reviewFails = false, duplicate, never, failAt, failure = Error('synthetic private sentinel'), attemptRows, snapshotSink, failSaveAt = [] } = {}) {
  const attempts = attemptRows ?? [], snapshots = snapshotSink ?? [], calls = [], saveErrors = [], secretHex = 'synthetic-private-input-never-exported';
  let saveCount = 0;
  let route = '/title/synthetic/upload', stage = 'upload', next, polls = delayed ? 2 : 0, selected = false, filled = false, acknowledged = false, reviewFailed = false;
  const later = (value, delay = delayed ? 2 : 0) => { next = value; polls = delay; if (!polls) { stage = next; next = undefined; } };
  const advance = () => { if (polls > 0 && --polls === 0 && next) { stage = next; next = undefined; if (stage === 'connected') selected = true; } };
  const timeout = () => Object.assign(Error('Synthetic bounded readiness deadline'), { code: 'UPLOAD_UI_READINESS_TIMEOUT' });
  const until = async (read, valid) => { for (let i = 0; i < 12; i++) { const value = await read(); if (valid(value)) return value; await Promise.resolve(); } throw timeout(); };
  const visible = name => {
    advance();
    if (never === name || polls > 0) return false;
    return ({ 'Open Settings': stage === 'upload', 'Public identity': stage === 'settings', 'Create or paste a key': stage === 'identity',
      'Paste an existing key': stage === 'choose', 'Private key (nsec or hex)': stage === 'import', 'Use this key': stage === 'import',
      'I have saved my private key somewhere safe.': stage === 'backup', 'Review public identity': stage === 'backup',
      'Use this key for this session': stage === 'review' || stage === 'backup' && backupSessionControl, 'Join this instance': stage === 'review' && enrollmentOpen,
      'Instance approval needed': stage === 'review' && !enrollmentOpen,
      'Key connected for this session': stage === 'connected', 'Done': stage === 'connected', 'Session private key connected.': selected && stage === 'identity',
      'Lock private key': selected && stage === 'identity', 'Upload Chapter': selected && stage === 'upload' })[name] === true;
  };
  const control = name => ({
    async count() { calls.push(['count', name]); if (duplicate === name) return 2; return visible(name) ? 1 : 0; },
    async isEnabled() { return visible(name); },
    async getAttribute(key) { assert.equal(key, 'type'); return 'password'; },
    async fill(value) { assert.equal(visible(name), true); assert.equal(value, secretHex); filled = true; calls.push(['fill']); },
    async check() { await until(() => this.count(), n => n === 1); acknowledged = true; calls.push(['ack']); },
    async click() {
      assert.equal(visible(name), true, 'Control must really be mounted before click'); calls.push(['click', name]);
      if (name === 'Open Settings') { route = '/settings/preferences'; later('settings'); }
      else if (name === 'Public identity') { route = '/settings/identity'; later('identity'); }
      else if (name === 'Create or paste a key') later('choose');
      else if (name === 'Paste an existing key') later('import');
      else if (name === 'Use this key') { assert.equal(filled, true); later('backup'); }
      else if (name === 'Review public identity') { assert.equal(acknowledged, true); reviewFailed = reviewFails; later(reviewFails ? 'backup' : 'review', delayed ? 4 : 0); }
      else if (name === 'Use this key for this session') { assert.equal(acknowledged, true); if (!delayed) selected = true; later('connected'); }
      else if (name === 'Join this instance') throw Error('Fixture self-enrollment is outside the expected import path');
      else if (name === 'Done') later('identity');
    },
  });
  const scope = { getByRole(role, options) {
      if (role === 'alert') return { async count() { advance(); return reviewFailed && polls === 0 ? 1 : 0; } };
      assert.equal(options.exact, true); return control(options.name);
    },
    getByLabel(name, options) { assert.equal(options.exact, true); return control(name); },
    getByText(pattern) { assert.equal(pattern.source, '^Session private key connected\\.'); return control('Session private key connected.'); } };
  const page = { ...scope, locator(selector) { assert.ok(['[data-protocol-identity-panel]', '[data-identity-setup]'].includes(selector)); return scope; },
    url: () => 'http://127.0.0.1:50123' + route, async evaluate() { return 123456; },
    async goBack() { calls.push(['back']); route = route === '/settings/identity' ? '/settings/preferences' : '/title/synthetic/upload'; stage = route === '/settings/preferences' ? 'settings' : 'upload'; }, };
  const unique = locator => uniqueUploadControl(locator, until);
  const button = async (parent, name) => { const target = await unique(parent.getByRole('button', { name, exact: true })); await until(() => target.isEnabled(), Boolean); return target; };
  const assertUploadRoute = () => assert.equal(route, '/title/synthetic/upload');
  const tracked = createKeyImportStep(attempts, () => {
    snapshots.push(structuredClone(attempts)); saveCount++;
    if (failSaveAt.includes(saveCount)) { const error = Error('Synthetic persistence failure'); saveErrors.push(error); throw error; }
  });
  const importStep = (name, action) => tracked(name, async () => { if (failAt === name) throw failure; return action(); });
  const args = { page, unique, button, until, assertUploadRoute, secretHex, importStep };
  return { args, attempts, snapshots, saveErrors, failure, calls, run: () => connectPublicUploadKey(args), snapshot: () => ({ selected, acknowledged, route }) };
}
export async function serializedImportTrace() {
  const rows = [];
  for (let attempt = 1; attempt <= 2; attempt++) {
    const fixture = importFixture({ attemptRows: rows }); await fixture.run();
  }
  return rows;
}
export async function serializedImportSnapshots(options = {}) {
  const fixture = importFixture(options); let failed = false;
  await fixture.run().catch(() => { failed = true; });
  return { snapshots: fixture.snapshots, attempts: fixture.attempts, failed };
}
