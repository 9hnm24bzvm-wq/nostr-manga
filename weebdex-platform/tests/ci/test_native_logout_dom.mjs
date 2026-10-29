// Actual composed logout route, actual Svelte component and actual coordinator.
// Native HTTP, Session/navigation and visual primitives are finite seams only.
import test, { after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { composed, composedBaseline } from './composed-account-library-fixture.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'), frontend = process.env.WEEBDEX_FRONTEND_ROOT; assert(frontend);
const require = createRequire(path.join(frontend, 'package.json')), ts = require('typescript'), { compile } = require('svelte/compiler');
const { JSDOM } = require(path.join(process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES, 'jsdom'));
const { window } = new JSDOM('<main></main>', { pretendToBeVisual: true, url: 'https://site.invalid/auth/logout' });
for (const name of ['window','document','Node','Element','HTMLElement','HTMLButtonElement','HTMLInputElement','Text','Comment','Event','CustomEvent','MutationObserver','getComputedStyle','requestAnimationFrame','cancelAnimationFrame']) globalThis[name] = window[name];
const client = pathToFileURL(path.join(frontend, 'node_modules/svelte/src/index-client.js')).href, store = pathToFileURL(path.join(frontend, 'node_modules/svelte/src/store/index-client.js')).href;
const { mount, unmount, flushSync } = await import(client), { writable, get } = await import(store);
const temp = fs.mkdtempSync(path.join(root, '.build/native-logout-dom-')); fs.symlinkSync(path.join(frontend, 'node_modules'), path.join(temp, 'node_modules'));
const originalFetch = globalThis.fetch;
after(() => { globalThis.fetch = originalFetch; window.close(); fs.rmSync(temp, { recursive: true, force: true }); delete globalThis.__logoutDOM; });
async function module(file) { const source = fs.readFileSync(path.join(root, 'platform-integrations/frontend-behavior', file), 'utf8'); return import('data:text/javascript;base64,' + Buffer.from(ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText).toString('base64')); }
const logout = await module('native-logout.ts'), { safeRegistrationReturnTo } = await module('account-onboarding.ts');
async function component(name, source, replace = true) {
  if (replace) source = source.replace(/<script lang="ts">([\s\S]*?)<\/script>/, (_, script) => {
    const ast = ts.createSourceFile(name + '.ts', script, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
    for (const statement of [...ast.statements].reverse()) { if (!ts.isImportDeclaration(statement)) continue;
      const specifier = statement.moduleSpecifier.text, clause = statement.importClause;
      let replacement = '';
      if (specifier === 'svelte' || specifier === 'svelte/store') replacement = statement.getText(ast).replace(/(['"])svelte(?:\/store)?\1/, JSON.stringify(specifier === 'svelte' ? client : store));
      else if (clause?.namedBindings && ts.isNamedImports(clause.namedBindings)) replacement = `const {${clause.namedBindings.elements.filter(entry => !entry.isTypeOnly).map(entry => entry.name.text).join(',')}}=globalThis.__logoutDOM;`;
      script = script.slice(0, statement.getStart(ast)) + replacement + script.slice(statement.end);
    } return '<script lang="ts">' + script + '</script>';
  });
  const result = compile(source, { filename: name + '.svelte', generate: 'client', runes: true });
  const file = path.join(temp, name + '.mjs'); fs.writeFileSync(file, result.js.code); return (await import(pathToFileURL(file))).default;
}
const Button = await component('Button', '<script>let{children,href,variant,...rest}=$props();</script>{#if href}<a {href} {...rest}>{@render children?.()}</a>{:else}<button {...rest}>{@render children?.()}</button>{/if}', false);
const Spinner = await component('Spinner', '<span data-synthetic-spinner>Pending</span>', false);
const Component = await component('NativeLogout', fs.readFileSync(path.join(root, 'platform-integrations/frontend-behavior/NativeLogout.svelte'), 'utf8'));
const settle = async () => { for (let index = 0; index < 12; index++) { await new Promise(resolve => setTimeout(resolve, 0)); flushSync(); } };
const button = (host, label) => { const found = [...host.querySelectorAll('button')].filter(element => element.textContent.trim() === label); assert.equal(found.length, 1, label); return found[0]; };
async function fixture({ mode = 'refuse', hold = false, navigationFails = false, redirect = '' } = {}) {
  window.history.replaceState({}, '', '/auth/logout' + (redirect ? '?redirect=' + encodeURIComponent(redirect) : ''));
  let active = true, current = true, phase = mode, release; const gate = new Promise(resolve => release = resolve), calls = [], navigations = [];
  const Session = writable({ identity: { id: 'NativeAccountA', username: 'Synthetic reader' } });
  const json = (value, status = 200) => new Response(JSON.stringify(value), { status, headers: { 'content-type': 'application/json' } });
  globalThis.fetch = async (url, init) => {
    const pathname = new URL(url).pathname; calls.push({ path: pathname, method: init.method });
    if (pathname.endsWith('/whoami')) return json({ active: true, id: 'SyntheticSessionA', identity: { id: 'NativeAccountA' } });
    if (pathname.endsWith('/logout/browser')) return json({ logout_token: 't'.repeat(43), session_id: 'SyntheticSessionA', account_id: 'NativeAccountA' });
    if (pathname === '/auth/self-service/logout') { if (hold) await gate; if (phase === 'success' || phase === 'lost') active = false; if (phase === 'lost') throw Error('Synthetic response lost'); return json({}, phase === 'refuse' ? 403 : 200); }
    assert.equal(pathname, '/api/auth/check'); return json({}, active ? 200 : 401);
  };
  globalThis.__logoutDOM = { Button, Spinner, Session, PUBLIC_SITE_NAME: 'Synthetic site', PUBLIC_API_URL: '/api', PUBLIC_KRATOS_PUBLIC_API: '/auth', page: { url: new URL(window.location.href) }, safeRegistrationReturnTo, ...logout,
    createAuthFormOwner: () => ({ observe() {}, capture() { return { alive: () => current }; } }),
    async goto(target) { navigations.push(target); if (navigationFails) throw Error('Synthetic navigation refused'); },
  };
  const host = document.createElement('section'); document.querySelector('main').append(host); const mounted = mount(Component, { target: host }); flushSync(); await settle();
  return { host, Session, calls, navigations, release, setMode(value) { phase = value; }, clearServer() { active = false; }, replaceView() { current = false; }, async close() { current = false; await unmount(mounted); host.remove(); release(); await settle(); } };
}
test('final composed logout route delegates once to the settled native implementation', async () => {
  const relative = 'src/routes/(app)/auth/logout/+page.svelte';
  const transformed = await composed(relative);
  assert.match(await composedBaseline(relative), /createBrowserLogoutFlow|updateLogoutFlow/);
  assert.match(transformed, /import NativeLogout from ['"]\$platform\/NativeLogout\.svelte['"]/);
  assert.equal((transformed.match(/<NativeLogout\s*\/>/g) ?? []).length, 1);
  assert.doesNotMatch(transformed, /createBrowserLogoutFlow|updateLogoutFlow|Logging out\.\.\./);
});
test('actual logout refusal settles, retains native session, and exposes ordinary recovery controls', async () => {
  const f = await fixture(); try { assert.match(f.host.textContent, /Sign out not confirmed/); assert.equal(f.host.querySelector('[data-synthetic-spinner]'), null); assert(get(f.Session)); assert.equal(f.navigations.length, 0); for (const label of ['Try signing out again','Check sign-out status','Reload account state']) button(f.host, label); } finally { await f.close(); }
});
test('explicit retry prepares once and clears native session only after confirmed absence', async () => {
  const f = await fixture(); try { f.setMode('success'); const retry = button(f.host, 'Try signing out again'); retry.click(); retry.click(); await settle(); assert.equal(f.calls.filter(row => row.method === 'POST').length, 2); assert.equal(get(f.Session), undefined); assert.deepEqual(f.navigations, ['/']); } finally { await f.close(); }
});
test('Check status is read-only and can settle a previously uncertain logout without another POST', async () => {
  const f = await fixture(); try { f.clearServer(); button(f.host, 'Check sign-out status').click(); await settle(); assert.equal(f.calls.filter(row => row.method === 'POST').length, 1); assert.equal(get(f.Session), undefined); } finally { await f.close(); }
});
test('late logout response cannot clear a replacement account or navigate its view', async () => {
  const f = await fixture({ mode: 'success', hold: true }); try { assert(f.host.querySelector('[data-synthetic-spinner]')); f.Session.set({ identity: { id: 'NativeAccountB', username: 'Other synthetic account' } }); f.release(); await settle(); assert.equal(get(f.Session).identity.id, 'NativeAccountB'); assert.equal(f.navigations.length, 0); assert.equal(f.host.querySelector('[data-synthetic-spinner]'), null); assert.match(f.host.textContent, /sign-in or this page changed/); } finally { await f.close(); }
});
test('closed logout route cannot act on a late completed response', async () => {
  const f = await fixture({ mode: 'success', hold: true }); await f.close(); assert.equal(get(f.Session).identity.id, 'NativeAccountA'); assert.equal(f.navigations.length, 0);
});
test('confirmed sign-out remains settled if navigation fails and unsafe redirect is not used', async () => {
  const f = await fixture({ mode: 'success', navigationFails: true, redirect: 'https://untrusted.invalid/path' }); try { assert.equal(get(f.Session), undefined); assert.match(f.host.textContent, /You are signed out/); assert.equal(f.host.querySelector('[data-synthetic-spinner]'), null); assert.deepEqual(f.navigations, ['/']); assert.equal(f.host.querySelector('a')?.getAttribute('href'), '/'); } finally { await f.close(); }
});
