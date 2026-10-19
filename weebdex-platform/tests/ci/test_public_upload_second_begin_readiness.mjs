// Finite source-extracted harness control. No browser, signer, HTTP, fixture,
// Svelte runtime, dependency install or backend is started by this file.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { createHash } from 'node:crypto';

const source = fs.readFileSync(new URL('../../scripts/public-upload-recovery-browser.mjs', import.meta.url), 'utf8');
const baselineSha = '808d27e183168c5b53f2e37317789b79d21601b75eb515c53d68824766605307';
const session = '11111111-1111-4111-8111-111111111111';
function extract(text, start, end) {
  const index = text.indexOf(start); assert.ok(index >= 0); assert.equal(text.indexOf(start, index + start.length), -1);
  const stop = text.indexOf(end, index + start.length); assert.ok(stop > index);
  return text.slice(index + start.length, stop);
}
function callback(text) {
  const code = extract(text, "await run('new begin preserves retained language as a genuine save', ", ");\n    await run('autosave title and chapter'");
  return new Function('env', `with (env) { return (${code}); }`);
}
function harness(text = source, options = {}) {
  const events = [], stages = [];
  let time = 0, ticks = 0, dispatched = false, terminal = false, failed = false, pageError = false;
  const readFailure = options.readFailure ?? Error('Independent current was not confirmed');
  const env = { sessionId: 'old-cancelled-session', lastVersion: 7, assert, Boolean,
    mark(stage) { stages.push(stage); },
    observer: {
      mutationCount: () => dispatched ? 1 : 0,
      settled() { events.push('native-terminal-read'); return terminal; },
    },
    panel: async () => 'exact-upload-panel',
    button: async (panel, name) => { assert.equal(panel, 'exact-upload-panel'); assert.equal(name, 'Confirm'); return { async click() { events.push('confirm'); dispatched = true; terminal = options.delayTicks === 0; } }; },
    async snapshotStage(label, expected, count, action) {
      assert.equal(label, 'begin-retained-language'); assert.deepEqual(expected, { images: [], draft: { language: 'en' } }); assert.equal(count, 2);
      await action(); events.push('unchanged-snapshot-oracles');
    },
    reader: { async get(path) { assert.equal(path, '/api/v1/upload'); events.push('independent-current'); return { status: terminal && !options.refusedRead ? 200 : 404, body: { id: session } }; } },
    healthy() { if (failed) throw options.failure; if (pageError) assert.fail('Application page error'); },
    Date: { now: () => time },
    page: { async waitForTimeout(ms) {
      assert.equal(ms, 100); time += ms; ticks++;
      if (options.failure && ticks === 1) failed = true;
      if (options.pageError && ticks === 1) pageError = true;
      if (ticks >= (options.delayTicks ?? 3)) { terminal = true; events.push('native-finished'); }
    } },
  };
  // Execute the real bounded waiter and real independent-current status/UUID
  // checks. The controlled observer and reader only model completion ordering.
  const wait = extract(text, 'async function until(', '\n  const unique =');
  env.until = new Function('env', `with (env) { return async function until(${wait.trim()}; }`)(env);
  const current = extract(text, 'const current = ', '\n  async function exact');
  env.current = new Function('env', `with (env) { const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/; return ${current.trim()} }`)(env);
  if (options.readFailure) env.reader.get = async () => { events.push('independent-current'); throw readFailure; };
  return { env, events, stages, readFailure, ticks: () => ticks, run: callback(text)(env) };
}

for (const delayTicks of [0, 1, 3]) test(`actual second-begin action waits for native completion before independent current, delay=${delayTicks}`, async () => {
  const h = harness(source, { delayTicks });
  assert.deepEqual(await h.run(), { session_id: session, version: 0 });
  assert.equal(h.ticks(), delayTicks); assert.equal(h.env.sessionId, session); assert.equal(h.env.lastVersion, 0);
  assert.equal(h.events.filter(event => event === 'confirm').length, 1);
  assert.equal(h.events.filter(event => event === 'independent-current').length, 1);
  if (delayTicks) assert.ok(h.events.indexOf('native-finished') < h.events.indexOf('independent-current'));
  assert.deepEqual(h.stages, ['begin-control', 'begin-dispatch', 'browser-terminal', 'current-read']);
  assert.equal(h.events.at(-1), 'unchanged-snapshot-oracles');
});
test('a permanently pending begin hits the existing finite readiness deadline before any independent read', async () => {
  const h = harness(source, { delayTicks: Infinity });
  await assert.rejects(h.run(), error => error.code === 'UPLOAD_UI_READINESS_TIMEOUT');
  assert.equal(h.ticks(), 150); assert.equal(h.events.includes('independent-current'), false);
  assert.equal(h.env.sessionId, 'old-cancelled-session'); assert.equal(h.env.lastVersion, 7);
});
test('native failure preserves the first observer assertion and prevents independent current', async () => {
  const failure = new assert.AssertionError({ message: 'signed_native_failed' }), h = harness(source, { failure });
  await assert.rejects(h.run(), error => error === failure);
  assert.equal(h.events.includes('independent-current'), false); assert.equal(h.events.includes('unchanged-snapshot-oracles'), false);
});
test('a page error during native readiness still refuses before independent current', async () => {
  const h = harness(source, { pageError: true }); await assert.rejects(h.run(), /Application page error/);
  assert.equal(h.events.includes('independent-current'), false);
});
test('finished begin cannot replace the actual independent discovery status gate', async () => {
  const h = harness(source, { delayTicks: 1, refusedRead: true }); await assert.rejects(h.run(), error => error.code === 'ERR_ASSERTION');
  assert.equal(h.events.includes('independent-current'), true); assert.equal(h.events.includes('unchanged-snapshot-oracles'), false);
  assert.equal(h.env.sessionId, 'old-cancelled-session'); assert.equal(h.env.lastVersion, 7);
});
test('an actual independent read failure retains its identity and does not reset the prior session/version', async () => {
  const readFailure = Error('Controlled current read failed'), h = harness(source, { delayTicks: 1, readFailure });
  await assert.rejects(h.run(), error => error === readFailure); assert.equal(h.events.includes('unchanged-snapshot-oracles'), false);
  assert.equal(h.env.sessionId, 'old-cancelled-session'); assert.equal(h.env.lastVersion, 7);
});
test('exact frozen917 counterfactual reads current before the delayed native begin is finished', async () => {
  const path = process.env.WEEBDEX_UPLOAD_SECOND_BEGIN_BASELINE_SOURCE;
  assert.ok(path, 'Provide the exact frozen917 runner source for the counterfactual');
  const original = fs.readFileSync(path, 'utf8'); assert.equal(createHash('sha256').update(original).digest('hex'), baselineSha);
  const old = "'Second begin started'); sessionId = await current();";
  const fixed = "'Second begin started'); mark('browser-terminal'); await until(() => observer.settled(), Boolean, 'Second begin browser terminal'); sessionId = await current();";
  assert.equal(original.split(old).length, 2);
  // The composed runner also contains the separately qualified QA reader.
  // Admit only its exact full bytes, then keep this check's action identical
  // to the historical action plus the bounded native-terminal wait.
  assert.equal(createHash('sha256').update(source).digest('hex'), '2c1edd5124418e1fb7c1d8a50e93f16aa2bc10bcbb42e7512f7dc2a47392975d');
  const action = text => extract(text, "await run('new begin preserves retained language as a genuine save', ", ");\n    await run('autosave title and chapter'");
  assert.equal(action(source), action(original.replace(old, fixed)), 'Only the bounded second-begin terminal wait may differ inside this action');
  const h = harness(original, { delayTicks: 3 });
  await assert.rejects(h.run(), error => error.code === 'ERR_ASSERTION');
  assert.equal(h.ticks(), 0); assert.equal(h.events.includes('independent-current'), true); assert.equal(h.events.includes('native-finished'), false);
  assert.equal(h.env.sessionId, 'old-cancelled-session'); assert.equal(h.env.lastVersion, 7);
});
