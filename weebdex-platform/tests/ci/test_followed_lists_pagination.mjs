// Actual component script and pagination expressions with finite account reads.
// No DOM, browser, signing transport or service is claimed by this seam.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import {stripTypeScriptTypes} from 'node:module';
import test from 'node:test';

const source = fs.readFileSync(new URL('../../platform-integrations/public-api/FollowedLists.svelte', import.meta.url), 'utf8');
const script = source.match(/<script[^>]*>([\s\S]*?)<\/script>/)[1].replace(/^\s*import[^;]+;\s*$/gm, '');
const nav = source.match(/\{#if ([^}]+)\}<nav[^>]*aria-label="Followed list pages"/)[1];
const previous = source.match(/<Button[^>]*disabled=\{([^}]+)\}[^>]*onclick=\{\(\)=>load\(Math\.max\(0,offset-20\)\)\}>Previous/)[1];
const next = source.match(/<Button[^>]*disabled=\{([^}]+)\}[^>]*onclick=\{\(\)=>load\(offset\+20\)\}>Next/)[1];
const refresh = source.match(/onclick=\{(\(\)=>\{reader\?\.dispose\(\);reader=undefined;void load\(\);\})\}/)[1];
function fixture() {
  let bound = 'First account', total = 45, failure = false, gate;
  const calls = [], captures = [];
  const loadFollowedLists = async (_nativeAccount,check) => {
    const account = bound; captures.push(account);
    return {dispose(){},username: account, async page(offset) {
      check(); calls.push({account, offset}); if (gate) await gate;
      check(); if (bound !== account) throw Error('Account binding changed');
      if (failure) throw Error('Private page unavailable');
      return {offset, total, unavailable: 0, data: Array.from({length: Math.max(0, Math.min(20, total - offset))}, (_, index) => ({id: String(index + offset), name: 'Actual list', members: []}))};
    }};
  };
  const ui = vm.runInNewContext('(function(){' + stripTypeScriptTypes(script, {mode: 'transform'}) + ';return{load,refresh:' + refresh + ',state:()=>({rows,pending,error,username,offset,total,navVisible:' + nav + ',previousDisabled:' + previous + ',nextDisabled:' + next + '})};})()', {
    Error, $state: value => value, $effect() {}, untrack: fn => fn(), onDestroy() {},
    $Session: {identity:{id:'Reader1234'}}, nativeAccountIssue:()=>undefined, PUBLIC_MANGADEX_CATALOGUE: '1', loadFollowedLists
  });
  return {ui, calls, captures, shrink(value) { total = value; }, fail(value = true) { failure = value; }, rebind() { bound = 'Second account'; }, hold(value) { gate = value; }};
}

test('shrinking below one page retains Previous for the existing later-page offset', async () => {
  const f = fixture(); await f.ui.load(20); f.shrink(5); await f.ui.load(20);
  const state = f.ui.state(); assert.equal(state.rows.length, 0); assert.equal(state.offset, 20); assert.equal(state.total, 5);
  assert.equal(state.navVisible, true); assert.equal(state.previousDisabled, false); assert.equal(state.nextDisabled, true);
  assert.match(source, /There are no available followed lists on this page\./);
});

test('Previous after shrink returns genuine first-page rows using the same captured account', async () => {
  const f = fixture(); await f.ui.load(20); f.shrink(5); await f.ui.load(20); await f.ui.load(0);
  assert.equal(f.ui.state().rows.length, 5); assert.equal(f.ui.state().offset, 0); assert.equal(f.ui.state().navVisible, false);
  assert.deepEqual(f.captures, ['First account']);
});

test('failed later-page reads fence both page controls and cannot recapture a rebound account', async () => {
  const f = fixture(); await f.ui.load(20); f.fail(); await f.ui.load(40);
  assert.equal(f.ui.state().navVisible, true); assert.equal(f.ui.state().previousDisabled, true); assert.equal(f.ui.state().nextDisabled, true);
  f.fail(false); f.rebind(); await f.ui.load(0); // Injected callback must also keep its original reader.
  assert.equal(f.ui.state().rows.length, 0); assert.match(f.ui.state().error, /binding changed/);
  assert.deepEqual(f.captures, ['First account']);
  f.ui.refresh(); await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(f.captures, ['First account', 'Second account']); assert.equal(f.ui.state().username, 'Second account');
});

test('pending page transitions disable both controls and reject a second load', async () => {
  const f = fixture(); await f.ui.load(20); let release; f.hold(new Promise(resolve => { release = resolve; }));
  const pending = f.ui.load(40); assert.equal(f.ui.state().previousDisabled, true); assert.equal(f.ui.state().nextDisabled, true);
  await f.ui.load(0); assert.equal(f.calls.length, 2); release(); await pending;
  assert.equal(f.ui.state().offset, 40); assert.equal(f.ui.state().rows.length, 5); assert.equal(f.ui.state().previousDisabled, false); assert.equal(f.ui.state().nextDisabled, true);
});

test('a genuinely empty first page has no fabricated rows or unnecessary page controls', async () => {
  const f = fixture(); f.shrink(0); await f.ui.load();
  assert.equal(f.ui.state().total, 0); assert.equal(f.ui.state().rows.length, 0); assert.equal(f.ui.state().navVisible, false); assert.equal(f.ui.state().error, '');
});
