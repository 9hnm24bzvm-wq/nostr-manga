#!/usr/bin/env node
// Execute the actual journey helper and held-callback call-site seam. Locator
// lifetime and timeout outcomes are driver stand-ins; no browser or Svelte DOM
// starts, and no real elapsed five-second timeout or browser acceptance is claimed.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';

const source = fs.readFileSync(new URL('../../scripts/browser-ui11-16-journeys.mjs', import.meta.url), 'utf8');
const helpers = [...source.matchAll(/  const noManagementPanel = async \(\) => \{[\s\S]*?\n  \};/g)];
assert.equal(helpers.length, 1, 'Execute the one actual management-detachment helper');
const heldSeams = [...source.matchAll(/const newer = page\.url\(\); await noManagementPanel\(\);[\s\S]*?await gate\.release\(\);/g)];
assert.equal(heldSeams.length, 1, 'Execute the actual newer-route check before obsolete callback release');
const leakMessage = 'Library management leaked outside the dedicated settings route';
const insist = (condition, message) => assert.ok(condition, message);
const helperFor = page => vm.runInNewContext('(() => {\n' + helpers[0][0] + '\nreturn noManagementPanel; })()', { page, insist });
const boundedDetach = options => {
  assert.equal(options.state, 'detached');
  assert.equal(options.timeout, 5000, 'Disposal must have its own finite five-second limit');
  assert.deepEqual(Object.keys(options).sort(), ['state', 'timeout']);
};
const pageFor = locator => ({ locator(selector) { assert.equal(selector, '[data-local-library-panel]'); return locator; } });

test('actual helper joins initially mounted management disposal before checking its absence', async () => {
  const events = [];
  let mounted = true, finishDetach;
  const detached = new Promise(resolve => { finishDetach = resolve; });
  const noManagementPanel = helperFor(pageFor({
    waitFor(options) { boundedDetach(options); events.push('wait-detached'); return detached; },
    async count() { events.push('count'); assert.equal(mounted, false); return 0; },
  }));
  const pending = noManagementPanel();
  assert.deepEqual(events, ['wait-detached'], 'A URL change alone must not trigger the absence assertion');
  mounted = false;
  events.push('detached');
  finishDetach();
  await pending;
  assert.deepEqual(events, ['wait-detached', 'detached', 'count']);
});

test('permanently mounted management fails the bounded wait before count or actual stale-gate release', async () => {
  const timeout = new Error('locator.waitFor: Timeout 5000ms exceeded; management remains attached');
  const events = [];
  const page = {
    url() { events.push('newer-url'); return 'http://fixture.invalid/search'; },
    locator(selector) {
      assert.equal(selector, '[data-local-library-panel]', 'No later dialog assertion may run after disposal times out');
      return {
        async waitFor(options) { boundedDetach(options); events.push('wait-detached'); throw timeout; },
        async count() { events.push('count'); assert.fail('A failed detached wait must not fall back to count'); },
      };
    },
  };
  const gate = {
    async held() { events.push('gate-held'); return true; },
    async release() { events.push('gate-release'); },
  };
  const noManagementPanel = helperFor(page);
  const seam = heldSeams[0][0];
  assert(seam.indexOf('await noManagementPanel();') < seam.indexOf('await gate.held()'));
  assert(seam.indexOf('await gate.held()') < seam.indexOf('await gate.release();'));
  const operation = vm.runInNewContext('(async () => {\n' + seam + '\n})()', { page, gate, insist, noManagementPanel });
  await assert.rejects(operation, cause => cause === timeout);
  assert.deepEqual(events, ['newer-url', 'wait-detached']);
});

test('a resolved detached wait with a nonzero count still fails the original leak assertion', async () => {
  for (const remaining of [1, 2]) {
    const events = [];
    const noManagementPanel = helperFor(pageFor({
      async waitFor(options) { boundedDetach(options); events.push('wait-detached'); },
      async count() { events.push('count'); return remaining; },
    }));
    await assert.rejects(noManagementPanel(), cause => cause.message === leakMessage);
    assert.deepEqual(events, ['wait-detached', 'count']);
  }
});

test('arbitrary locator wait rejection is preserved without retry or an absence fallback', async () => {
  const failure = new Error('Page closed during outgoing-route disposal');
  let waits = 0, counts = 0, locators = 0;
  const noManagementPanel = helperFor({ locator(selector) {
    assert.equal(selector, '[data-local-library-panel]');
    locators++;
    return {
      async waitFor(options) { boundedDetach(options); waits++; throw failure; },
      async count() { counts++; return 0; },
    };
  } });
  await assert.rejects(noManagementPanel(), cause => cause === failure);
  assert.deepEqual({ waits, counts, locators }, { waits: 1, counts: 0, locators: 1 });
});
