// Finite deterministic source/control-flow seams, not a browser rendering test.
// No browser, server, database, network, dependency install or CI execution.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import { readSeedGroupState, seedGroupReady, waitForSeedGroupReady } from '../../scripts/startup-readiness-probe.mjs';

const root = new URL('../../', import.meta.url);
const read = path => fs.readFileSync(new URL(path, root), 'utf8');
const source = read('scripts/browser-acceptance.mjs');
const openSource = source.match(/\tasync function open\(route\) \{[^\n]+/)[0].trim();
const stepSource = source.match(/for \(const route of routes\) await step\(`fresh deep link \$\{route\}`, async \(\) => \{([\s\S]*?)\n\t\t\}\);/)[1];
const good = { pathname: '/group/SeedGroup1', text: 'Fixture Scanlation Team\nInformation\nSynthetic local browser group.',
  heading: 'Fixture Scanlation Team', visible: true, readyState: 'complete' };
const empty = { ...good, text: '', heading: '', visible: false };
const flags = { document_ready: true, body_meaningful: true, fixture_heading_visible: true, fixture_route: true };
function sample(fn, frame) {
  const result = vm.runInNewContext('(' + fn.toString() + ')()', {
    document: { readyState: frame.readyState, body: { innerText: frame.text },
      querySelectorAll: () => [{ textContent: frame.heading, getClientRects: () => frame.visible ? [{}] : [] }] },
    location: { pathname: frame.pathname, search: '?private=not-exported' },
  });
  return JSON.parse(JSON.stringify(result));
}

test('exact group predicate is supported by the genuine fixture and retained route marker', () => {
  assert.match(read('tests/runtime/browser_fixture.py'), /INSERT INTO platform\.catalog_groups.*SeedGroup1.*Fixture Scanlation Team/);
  assert.match(read('frontend/src/routes/(app)/group/[id]/Group.svelte'), /<h1[^>]*>\{\$group\.name\}<\/h1>/);
  assert.match(read('frontend/src/routes/(app)/group/[id]/+layout.ts'), /await API\.Group\.Get\(params\.id\)/);
  assert.match(read('frontend/src/routes/+layout.ts'), /export const ssr = false/);
  assert.deepEqual(sample(readSeedGroupState, good), flags);
  assert.equal(sample(seedGroupReady, good), true);
  for (const change of [{ pathname: '/groups' }, { pathname: '/group/OtherGroup' }, { text: '' },
    { heading: 'Other Group' }, { visible: false }, { readyState: 'loading' }]) {
    assert.equal(sample(seedGroupReady, { ...good, ...change }), false);
  }
  assert.equal(JSON.stringify(sample(readSeedGroupState, good)).includes('private'), false);
});

async function actualGroupStep({ neverReady = false, earlyPageError = false, loseBodyAfterWait = false } = {}) {
  let frame = empty, virtualMs = 0, snapshots = 0, sleeps = 0;
  const report = { page_errors: [], pages: [] };
  const page = {
    async goto(url, options) {
      assert.equal(url, 'http://127.0.0.1:31415/group/SeedGroup1');
      assert.equal(options.waitUntil, 'domcontentloaded');
      if (earlyPageError) report.page_errors.push({ error: 'synthetic page error' });
      return { status: () => 200 };
    },
    async waitForTimeout() { sleeps++; throw Error('Group must not use a fixed sleep'); },
    async waitForFunction(predicate, arg, options) {
      assert.equal(predicate, seedGroupReady); assert.equal(arg, undefined);
      assert.equal(options.polling, 100); assert.ok(options.timeout > 0 && options.timeout <= 6750);
      assert.equal(sample(predicate, frame), false);
      if (neverReady) { const error = Error('synthetic readiness timeout'); error.name = 'TimeoutError'; throw error; }
      // Model genuine content arriving after the old 1200ms snapshot point.
      virtualMs = 1800; frame = good; assert.equal(sample(predicate, frame), true);
    },
    async evaluate(fn) { return sample(fn, frame); },
  };
  const context = vm.createContext({ page, report, route: '/group/SeedGroup1', phase: 'startup',
    origin: 'http://127.0.0.1:31415', seedTitleCapture: false, waitForSeedGroupReady,
    waitForSeedTitleReady: () => { throw Error('Group must not use the title marker'); },
    observeSeedTitleReadiness: () => { throw Error('Group must not produce title diagnostic fields'); },
    insist: (value, message) => { if (!value) throw Error(message); },
    snapshot: async () => { snapshots++; return { text: loseBodyAfterWait ? '' : frame.text, title: frame.heading, horizontal_overflow: false }; },
    screenshot: () => { throw Error('Unexpected screenshot'); }, save: () => {},
  });
  vm.runInContext(openSource, context);
  const action = vm.runInContext('(async () => {' + stepSource + '})', context);
  try { return { value: await action(), report, virtualMs, snapshots, sleeps }; }
  catch (error) { Object.assign(error, { report, virtualMs, snapshots, sleeps }); throw error; }
}

test('real group step waits for late fixture rendering before unchanged snapshot assertions', async () => {
  const result = await actualGroupStep();
  assert.equal(result.virtualMs, 1800); assert.equal(result.snapshots, 1); assert.equal(result.sleeps, 0);
  assert.ok(result.value.rendered_text_length > 20); assert.equal(result.report.pages.length, 1);
  assert.deepEqual(Object.keys(result.report).sort(), ['page_errors', 'pages']);
});

test('real group step never passes when expected content never renders', async () => {
  await assert.rejects(actualGroupStep({ neverReady: true }), error => {
    assert.equal(error.name, 'TimeoutError'); assert.equal(error.snapshots, 0);
    assert.equal(error.report.pages.length, 0); assert.equal(error.sleeps, 0); return true;
  });
});

test('early group page error still fails the unchanged error assertion after readiness', async () => {
  await assert.rejects(actualGroupStep({ earlyPageError: true }), error => {
    assert.match(error.message, /Deep link has JavaScript page errors/);
    assert.equal(error.report.page_errors.length, 1); assert.equal(error.snapshots, 1); return true;
  });
});

test('body content disappearing after readiness still fails the original assertion', async () => {
  await assert.rejects(actualGroupStep({ loseBodyAfterWait: true }), /Page never rendered meaningful content/);
});

test('final readiness resample rejects missing, stale, hidden or unready content', async () => {
  for (const key of Object.keys(flags)) await assert.rejects(waitForSeedGroupReady({
    waitForFunction: async () => {}, evaluate: async () => ({ ...flags, [key]: false }),
  }), /readiness changed before its snapshot/);
});

test('whole group wait is bounded and late rejection is observed', async () => {
  let reject, samples = 0;
  const pending = new Promise((_, fail) => { reject = fail; });
  const started = Date.now();
  await assert.rejects(waitForSeedGroupReady({ waitForFunction: () => pending,
    evaluate: async () => { samples++; return flags; } }, { timeoutMs: 10 }), { name: 'TimeoutError' });
  assert.ok(Date.now() - started < 1000); assert.equal(samples, 0);
  reject(Error('late private renderer error')); await new Promise(done => setTimeout(done, 20));
  assert.equal(samples, 0);
  await assert.rejects(waitForSeedGroupReady({ waitForFunction: async () => {},
    evaluate: () => new Promise(() => {}) }, { timeoutMs: 10 }), { name: 'TimeoutError' });
});

test('new synchronization preserves existing scoped routing, assertions, CSP and sandbox', async () => {
  assert.match(openSource, /phase === "startup" && route === "\/group\/SeedGroup1"/);
  assert.match(source, /insist\(observation\.text\.trim\(\)\.length > 20, "Page never rendered meaningful content"\)/);
  assert.match(source, /insist\(report\.page_errors\.length === before, "Deep link has JavaScript page errors"\)/);
  assert.match(source, /chromiumSandbox: true/);
  assert.match(source, /Served CSP is missing current exact bootstrap hash/);
  assert.match(source, /Script CSP unexpectedly weakened/);
  assert.doesNotMatch(source, /--no-sandbox/);
  for (const [phase, route] of [['startup', '/groups'], ['account', '/group/SeedGroup1']]) {
    let sleep;
    const context = vm.createContext({ phase, origin: 'http://127.0.0.1:31415',
      page: { goto: async () => ({ status: () => 200 }), waitForTimeout: async ms => { sleep = ms; } },
      insist: assert.ok, waitForSeedGroupReady: () => { throw Error('Out-of-scope synchronization'); },
      waitForSeedTitleReady: () => { throw Error('Out-of-scope synchronization'); },
    });
    vm.runInContext(openSource, context); await context.open(route); assert.equal(sleep, 1200);
  }
});
