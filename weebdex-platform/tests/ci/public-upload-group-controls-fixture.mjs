// Explicit finite locators and source-extracted actual runner functions. No
// browser/DOM implementation, HTTP, native process, database or real signer.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createUploadJourneyProgress } from '../../scripts/public-upload-journey-progress.mjs';
import { uniqueUploadControl } from '../../scripts/public-upload-key-import.mjs';

export const runnerSource = readFileSync(new URL('../../scripts/public-upload-recovery-browser.mjs', import.meta.url), 'utf8');
export const groupsSource = readFileSync(new URL('../../frontend/src/lib/components/upload/UploadChapterGroups.svelte', import.meta.url), 'utf8');
export const inputGroupSource = readFileSync(new URL('../../frontend/src/lib/components/ui/input-group/input-group.svelte', import.meta.url), 'utf8');
export const inputTextSource = readFileSync(new URL('../../frontend/src/lib/components/ui/input-group/input-group-text.svelte', import.meta.url), 'utf8');
export function functionBody(source, signature, nextSignature) {
  const token = 'async function ' + signature + ' {';
  const start = source.indexOf(token), end = source.indexOf(nextSignature, start + token.length);
  assert.ok(start >= 0 && end > start); const body = source.slice(start + token.length, end).trim();
  assert.ok(body.endsWith('}')); return body.slice(0, -1);
}
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
export function runWrapper(progress, report, observedErrors = []) {
  const body = functionBody(runnerSource, 'run(name, action)', '\n  async function control');
  const step = async (name, action) => {
    const row = { status: 'running' }; report.steps.push(row);
    try { await action(); row.status = 'passed'; } catch (error) { observedErrors.push(error); row.status = 'failed'; }
  };
  return new Function('progress', 'report', 'step', 'assert', 'let nextCheck = 0; return async function(name, action) {' + body + '};')(progress, report, step, assert);
}
export function groupFixture(options = {}) {
  const rows = [], snapshots = [], report = { steps: [] }, observedErrors = [];
  let saves = 0, triggerReads = 0, resultReads = 0;
  const saveError = Error('synthetic persistence failure');
  const progress = createUploadJourneyProgress(rows, () => {
    snapshots.push(structuredClone(rows)); if (options.failSaves?.includes(++saves)) throw saveError;
  });
  const run = runWrapper(progress, report, observedErrors);
  const state = { triggerClicks: 0, resultClicks: 0, selected: false, dialogOpen: false, query: '', escape: 0 };
  const triggerRole = inputGroupSource.match(/<div[\s\S]*?role="([^"]+)"/)[1];
  assert.equal(triggerRole, 'group');
  const groupName = 'Synthetic signed upload group';
  const locator = (count, click = async () => {}, enabled = () => true) => ({ count: async () => count(), click, isEnabled: async () => enabled() });
  const timeout = () => Object.assign(Error('finite readiness expired'), { code: 'UPLOAD_UI_READINESS_TIMEOUT' });
  const until = async (read, valid) => { for (let i = 0; i < 6; i++) { const value = await read(); if (valid(value)) return value; } throw timeout(); };
  const unique = target => uniqueUploadControl(target, until);
  const trigger = locator(() => options.duplicateTrigger ? 2 : ++triggerReads <= (options.triggerDelay ?? 0) ? 0 : 1, async () => {
    if (options.disabledTrigger) throw Object.assign(Error('finite disabled pointer target'), { name: 'TimeoutError' });
    state.triggerClicks++; state.dialogOpen = true;
  });
  const result = locator(() => !state.dialogOpen || !state.query || options.searchFails ? 0 : options.duplicateResult ? 2 : ++resultReads <= (options.resultDelay ?? 0) ? 0 : 1,
    async () => { state.resultClicks++; state.selected = !state.selected; }, () => !options.disabledResult);
  const dialog = {
    ...locator(() => state.dialogOpen ? 1 : 0),
    getByPlaceholder(name, settings) { assert.equal(name, 'Search...'); assert.equal(settings.exact, true); return { fill: async query => { state.query = query; } }; },
    locator(selector) { assert.equal(selector, 'button[title="' + groupName + '"]'); return result; },
    getByRole(role, settings) { assert.equal(role, 'button'); assert.equal(settings.name, groupName); assert.equal(settings.exact, true); return result; },
  };
  const panel = async () => ({
    getByText(text, settings) { assert.equal(text, 'Search...'); assert.equal(settings.exact, true); return trigger; },
    getByRole(role, settings) { assert.equal(settings.name, 'Search...'); return role === triggerRole ? trigger : locator(() => 0); },
  });
  const page = {
    getByRole(role) { assert.equal(role, 'dialog'); return dialog; },
    keyboard: { press: async key => { assert.equal(key, 'Escape'); state.escape++; if (!options.closeFails) state.dialogOpen = false; } },
  };
  const button = async (scope, name) => { const target = await unique(scope.getByRole('button', { name, exact: true })); await until(() => target.isEnabled(), Boolean); return target; };
  const source = options.source ?? runnerSource;
  const chooseGroup = new AsyncFunction('mark', 'panel', 'unique', 'page', 'button', 'until', 'assert', functionBody(source, 'chooseGroup()', '\n  async function connectKey'));
  const mark = stage => { progress.mark(stage); if (options.failStage === stage) throw options.actionError; };
  return { rows, snapshots, report, state, observedErrors, saveError,
    async run() { await run('fixed check 0', async () => {}); await run('fixed check 1', async () => {}); return run('fixed check 2', () => chooseGroup(mark, panel, unique, page, button, until, assert)); } };
}
