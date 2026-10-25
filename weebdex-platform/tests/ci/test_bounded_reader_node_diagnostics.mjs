// Source-bound QA instrumentation controls, not mounted/application qualification.
import fs from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { stripTypeScriptTypes } from 'node:module';
import { types as nodeUtilTypes } from 'node:util';
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const sourceUrl = new URL('../../scripts/integrated-load/mounted-reader.mjs', import.meta.url);
const source = fs.readFileSync(sourceUrl, 'utf8');
assert.equal(hash(source), 'badb60c9f59b93f2267a5a50bda41364acc7086d7bd2cff4e38a0d8153c998ee');
const a = source.indexOf('// BEGIN MOUNTED NODE DIAGNOSTICS'), b = source.indexOf('// END MOUNTED NODE DIAGNOSTICS', a);
const c = source.indexOf('const semanticObservations = [];'), d = source.indexOf('\nconst fail =', c);
assert(a >= 0 && b > a && c > b && d > c && b - a < 16384 && d - c < 8192);
const exact = source.slice(a, b) + '\n' + source.slice(c, d);
const build = vm.compileFunction("'use strict';\n" + exact + '\nreturn { createMountedNodeDiagnostics, diagnosticWarningShape, diagnosticDataOptions, observations: semanticObservations, overflow: () => semanticObservationOverflow };', ['counts', 'mountedCases', 'BOUNDED_MOUNTED_CASES', 'nodeUtilTypes', 'stripTypeScriptTypes', 'vm']);
const baseline = { error: console.error, emit: process.emit, emitWarning: process.emitWarning, nextTick: process.nextTick, listeners: process.rawListeners('warning') };
const messages = ['stripTypeScriptTypes is an experimental feature and might change at any time', 'VM Modules is an experimental feature and might change at any time'];
const categories = ['node-type-stripping', 'node-vm-modules'];
const cases = []; let live, positiveProof, observedOverflow = false, capturedNativeWarning;
const settle = () => new Promise(resolve => setImmediate(resolve));
const fake = (message = messages[0]) => Object.assign(new Error(message), { name: 'ExperimentalWarning' });
function open() {
  const counts = { semantic_failures: 0 }, phases = [];
  const handler = build(counts, phases, ['actual-case'], nodeUtilTypes, stripTypeScriptTypes, vm);
  const diagnostics = handler.createMountedNodeDiagnostics(true);
  return live = { counts, phases, handler, diagnostics };
}
function restored() {
  assert.equal(process.emit, baseline.emit); assert.equal(process.emitWarning, baseline.emitWarning);
  assert.equal(process.nextTick, baseline.nextTick); assert.equal(console.error, baseline.error);
  assert.deepEqual(process.rawListeners('warning'), baseline.listeners);
}
function close() { live.diagnostics.close(); live = null; restored(); }
function failExactly(operation, amount = 1) {
  const before = live.counts.semantic_failures;
  operation(); assert.equal(live.counts.semantic_failures, before + amount);
  assert.deepEqual(live.diagnostics.result().accepted, { 'node-type-stripping': 0, 'node-vm-modules': 0 });
}
try {
  // Run genuine providers first, so the first native forwarding hint is tested.
  open();
  const transformed = live.diagnostics.strip('export const value: number = 1;', { mode: 'transform' });
  assert.equal(typeof transformed, 'string'); assert(transformed.includes('value'));
  assert.equal(process.emitWarning, baseline.emitWarning); assert.equal(process.nextTick, baseline.nextTick);
  const module = live.diagnostics.module('export const value = 1;', { identifier: 'owned-node-warning-control' });
  assert(module instanceof vm.SourceTextModule);
  assert.equal(process.emitWarning, baseline.emitWarning); assert.equal(process.nextTick, baseline.nextTick);
  assert.equal(live.diagnostics.result().provider_scheduled, 2); assert.equal(live.diagnostics.result().provider_completed, 0);
  await live.diagnostics.settle(); assert.equal(live.diagnostics.result().provider_completed, 2);
  await module.link(() => { throw Error('Unexpected dependency'); }); await module.evaluate(); assert.equal(module.namespace.value, 1);
  await settle(); assert.equal(live.counts.semantic_failures, 0);
  live.diagnostics.finishSetup(); live.diagnostics.close();
  assert(live.diagnostics.ready()); positiveProof = live.diagnostics.result();
  assert.deepEqual(positiveProof, { accepted: { 'node-type-stripping': 1, 'node-vm-modules': 1 }, provider_scheduled: 2, provider_completed: 2, failures: 0, setup_closed: true, listener_cleanup: true });
  close(); cases.push('actual providers preserve results and exactly two owned pairs pass');
  cases.push('temporary hooks dispatch tokens and warning listeners fully settle');

  // Obtain a real Node-created Error without granting an owned dispatch token.
  open();
  const capture = error => { capturedNativeWarning = error; };
  process.once('warning', capture);
  process.emitWarning(messages[0], 'ExperimentalWarning'); await settle();
  assert(capturedNativeWarning instanceof Error); assert(live.counts.semantic_failures > 0);
  assert.deepEqual(live.diagnostics.result().accepted, { 'node-type-stripping': 0, 'node-vm-modules': 0 });
  close(); cases.push('unowned native emission and extra listener fail');

  open();
  failExactly(() => process.emit('warning', capturedNativeWarning));
  failExactly(() => process.emit('warning', capturedNativeWarning));
  for (const message of messages) failExactly(() => process.emit('warning', fake(message)));
  cases.push('native Error replays and both exact fake events fail');
  const forged = fake();
  const text = 'ExperimentalWarning: ' + messages[0] + '\n    at emitExperimentalWarning (node:internal/util:1:1)\n    at stripTypeScriptTypes (node:internal/modules/typescript:1:1)';
  forged.stack = text; // Includes native-getter-preserving setters on newer V8.
  failExactly(() => process.emit('warning', forged));
  const dataStack = fake(); Object.defineProperty(dataStack, 'stack', { value: text, writable: true, configurable: true });
  failExactly(() => process.emit('warning', dataStack));
  cases.push('setter-forged and data-stack provider evidence fail');
  failExactly(() => console.error(`(node:${process.pid}) ExperimentalWarning: ${messages[0]}`));
  failExactly(() => console.error(new Error('Ordinary synthetic error')));
  failExactly(() => process.emit('warning', Object.assign(new Error('Unknown diagnostic'), { name: 'Warning' })));
  assert(live.handler.overflow() > 0); observedOverflow = true;
  assert.equal(live.handler.observations.length, 8);
  cases.push('ordinary errors unknown warnings spoof strings and overflow count');
  let getters = 0;
  const accessor = fake(); Object.defineProperty(accessor, 'message', { get() { getters++; throw Error('Never invoke'); } });
  assert.equal(live.handler.diagnosticWarningShape(accessor, categories[0]), false);
  const proxy = new Proxy(fake(), { getOwnPropertyDescriptor() { getters++; throw Error('Never invoke'); }, ownKeys() { getters++; throw Error('Never invoke'); } });
  assert.equal(live.handler.diagnosticWarningShape(proxy, categories[0]), false);
  const options = Object.defineProperty({}, 'mode', { get() { getters++; throw Error('Never invoke'); } });
  assert.equal(live.handler.diagnosticDataOptions(options, ['mode']), false); assert.equal(getters, 0);
  close(); open(); failExactly(() => console.error(proxy)); assert.equal(getters, 0);
  cases.push('argument and option getters and proxy traps are never inspected');
  close();


  open(); failExactly(() => live.diagnostics.finishSetup());
  assert(!live.diagnostics.ready()); close(); cases.push('missing provider proof refuses setup completion');

  open();
  failExactly(() => process.emit('warning', capturedNativeWarning));
  process.emitWarning(messages[1], 'ExperimentalWarning'); await settle();
  assert.equal(live.counts.semantic_failures, 2);
  assert.deepEqual(live.diagnostics.result().accepted, { 'node-type-stripping': 0, 'node-vm-modules': 0 });
  live.phases.push({ name: 'actual-case', outcome: 'started' });
  failExactly(() => process.emit('warning', fake(messages[1])));
  close(); cases.push('extra duplicate and application-phase warnings never gain acceptance');

  open(); let preparations = 0;
  Object.defineProperty(Error, 'prepareStackTrace', { value() { preparations++; return 'forged'; }, writable: true, configurable: true });
  try { failExactly(() => console.error('Ordinary error while hook changed')); assert.equal(preparations, 0); }
  finally { delete Error.prepareStackTrace; }
  close(); cases.push('changed prepareStackTrace is never called by inspection');

  for (const key of ['emit', 'emitWarning', 'nextTick']) {
    open(); const original = process[key], replacement = function (...args) { return Reflect.apply(original, this, args); };
    process[key] = replacement;
    try {
      failExactly(() => assert.equal(typeof live.diagnostics.strip('const value: number = 1;', { mode: 'transform' }), 'string'));
      assert.equal(process[key], replacement);
    } finally { process[key] = original; }
    close();
  }
  cases.push('changed process hooks fail closed and are not overwritten');

  open();
  for (const [invoke, original] of [[() => live.diagnostics.strip(null), () => stripTypeScriptTypes(null)], [() => live.diagnostics.module(null), () => new vm.SourceTextModule(null)]]) {
    let expected, actual; try { original(); } catch (error) { expected = error; }
    try { invoke(); } catch (error) { actual = error; }
    assert(expected && actual); assert.equal(actual.constructor, expected.constructor); assert.equal(actual.code, expected.code);
    assert.equal(process.emitWarning, baseline.emitWarning); assert.equal(process.nextTick, baseline.nextTick);
  }
  close(); cases.push('provider argument throws preserve type code and hook restoration');

  open();
  process.removeListener('warning', baseline.listeners[0]);
  failExactly(() => process.emit('warning', fake()));
  process.on('warning', baseline.listeners[0]);
  live.diagnostics.close(); assert.equal(live.diagnostics.result().listener_cleanup, true);
  close(); cases.push('missing native forwarding fails while owned listener cleanup remains exact');

  open(); const outsider = () => {};
  process.on('warning', outsider); live.diagnostics.close();
  assert.equal(live.diagnostics.result().listener_cleanup, false); assert(process.rawListeners('warning').includes(outsider));
  process.removeListener('warning', outsider); close();
  open(); const replacementConsole = () => {}; console.error = replacementConsole;
  live.diagnostics.close(); assert.equal(console.error, replacementConsole); assert.equal(live.diagnostics.result().listener_cleanup, false);
  console.error = baseline.error; close(); cases.push('cleanup owns only its listeners and console interception');

  assert.equal(hash(fs.readFileSync(sourceUrl)), hash(source)); restored();
  const result = { passed: true, cases, sourceSha256: hash(source), handlerSha256: hash(exact), testSha256: hash(fs.readFileSync(new URL(import.meta.url))), positiveProof, overflowCounted: observedOverflow, sourceUnchanged: true };
  const output = JSON.stringify(result); assert(Buffer.byteLength(output) < 4096); process.stdout.write(output + '\n');
} catch (error) {
  process.stdout.write(JSON.stringify({ passed: false, cases, sourceSha256: hash(source), failure: error instanceof assert.AssertionError ? 'assertion' : 'unexpected', semanticFailures: live?.counts.semantic_failures ?? null, diagnostics: live?.diagnostics.result() ?? null }) + '\n');
  process.exitCode = 1;
} finally {
  if (live) { await live.diagnostics.settle(); live.diagnostics.close(); }
  console.error = baseline.error;
  process.emit = baseline.emit; process.emitWarning = baseline.emitWarning; process.nextTick = baseline.nextTick;
}
