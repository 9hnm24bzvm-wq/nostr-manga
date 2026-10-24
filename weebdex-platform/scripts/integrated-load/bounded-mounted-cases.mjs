// Actual compiled/mounted owned controls. Router completion and server responses
// are fixed synthetic seams, not browser navigation or backend qualification.
import assert from 'node:assert/strict';
export const BOUNDED_MOUNTED_CASES = Object.freeze([
  'complete progress', 'partial remount progress', 'paged group disclosure',
  'edition paging and independent winner', 'single-edition title Start',
  'multiple-edition title Start chooser', 'observed device Continue', 'identity mismatch reload status'
]);
export async function runBoundedMountedCases({ env, runtime, settled, component, read, library, until, counts, pending, mountedCases, navigationDispatches, uuid, window, phase }) {
  for (const name of ['Status', 'GroupPage', 'EditionPage', 'ChapterSelector', 'EditionSelector', 'TitleReadingActions']) env[name] = await component(name, read('platform-integrations/bounded-reader/' + name + '.svelte'));
  const ControlHarness = await component('ControlHarness', '<script>const { Provider, ChapterSelector, EditionSelector } = globalThis.__mountedLoad; let {chapter} = $props();</script><Provider {chapter}><ChapterSelector /><EditionSelector /></Provider>', false);
  const button = (host, text) => { const found = [...host.querySelectorAll('button')].find(node => node.textContent.trim() === text || node.textContent.trim().startsWith(text + ' ·')); assert(found, 'Actual button missing: ' + text); assert.equal(found.disabled, false, 'Actual button disabled: ' + text); return found; };
  const done = settled;
  async function withMounted(name, factory, work) {
    const row = { name, outcome: 'started', assertions: [] }; mountedCases.push(row);
    const host = document.createElement('main'); document.body.append(host); let instance;
    try { const mounted = await factory(host); instance = mounted.instance; await mounted.ready(); await work(host, row.assertions); }
    catch (error) { row.outcome = 'failed'; throw error; }
    finally { if (instance) await runtime.unmount(instance); host.remove(); await until(done); }
    assert.equal(counts.progress_failures, 0); assert.equal(pending.size, 0);
    row.assertions.push('actual unmount and owned request/progress settlement'); row.outcome = 'passed';
  }
  const reader = chapter => async host => {
    env.page.url = new URL('/chapter/' + uuid(chapter) + '/1', window.location.origin); env.page.params.page = '1'; env.page.state = {};
    window.history.replaceState({}, '', env.page.url.href);
    const response = await env.API.Chapter.Get(uuid(chapter)); assert.equal(response?.body?.id, uuid(chapter));
    const instance = runtime.mount(ControlHarness, { target: host, props: { chapter: response.body } });
    return { instance, ready: () => until(() => env.reader?.navigationState?.current?.id === uuid(chapter) && !env.reader.navigationState.pending && env.reader.currentPage?.isLoaded) };
  };
  const title = query => async host => {
    env.page.url = new URL('/title/' + uuid(1) + (query ? '?' + query : ''), window.location.origin); window.history.replaceState({}, '', env.page.url.href);
    const instance = runtime.mount(env.TitleReadingActions, { target: host, props: { mangaId: uuid(1) } });
    return { instance, ready: () => until(() => !host.querySelector('[aria-busy="true"]') && pending.size === 0) };
  };
  await withMounted('paged group disclosure', reader(102), async (host, checks) => {
    const details = [...host.querySelectorAll('details')].find(node => node.querySelector('summary')?.textContent === 'Chapters'); assert(details); details.open = true;
    await until(() => details.querySelectorAll('li').length === 2 && !env.reader.navigationState.pending);
    assert.equal(env.reader.navigationState.groups[0].id, uuid(102)); assert.equal(env.reader.navigationState.groups[1].id, uuid(101));
    button(details, 'Next chapter page').click(); await until(() => details.querySelectorAll('li').length === 1 && !env.reader.navigationState.pending);
    assert.equal(env.reader.navigationState.groups[0].id, uuid(103)); assert.equal(env.reader.currentPage.isLoaded, true);
    checks.push('actual disclosure requests the first bounded page', 'next page replaces old rows without losing loaded images');
  });
  await withMounted('edition paging and independent winner', reader(103), async (host, checks) => {
    const details = [...host.querySelectorAll('details')].find(node => node.querySelector('summary')?.textContent === 'Choose edition'); assert(details); details.open = true;
    await until(() => env.reader.navigationState.editions.length === 1 && !env.reader.navigationState.pending);
    assert.equal(env.reader.navigationState.editions[0].id, uuid(100)); assert.equal(env.reader.navigationState.winner.id, uuid(103));
    const before = navigationDispatches.length; button(details, 'Open complete-set preferred edition').click();
    await until(() => navigationDispatches.length === before + 1 && !env.reader.navigationState.pending);
    assert.equal(navigationDispatches.at(-1), '/chapter/' + uuid(103) + '/1');
    button(details, 'Next edition page').click(); await until(() => env.reader.navigationState.editions[0]?.id === uuid(103) && !env.reader.navigationState.pending);
    assert.equal(details.querySelectorAll('li').length, 1); assert.equal(navigationDispatches.length, before + 1);
    checks.push('full-set preferred edition remains selectable outside current page', 'next edition page replaces displayed rows');
  });
  await withMounted('single-edition title Start', title('group=' + uuid(501)), async (host, checks) => {
    const before = navigationDispatches.length; button(host, 'Start reading').click(); await until(() => navigationDispatches.length === before + 1 && pending.size === 0);
    assert.equal(navigationDispatches.at(-1), '/chapter/' + uuid(100) + '/1'); assert.equal(host.querySelector('[aria-label="Edition selection"]'), null);
    checks.push('actual title URL group override reaches the reader query', 'single eligible edition dispatches directly without chooser');
  });
  await withMounted('multiple-edition title Start chooser', title(''), async (host, checks) => {
    const before = navigationDispatches.length; button(host, 'Start reading').click(); await until(() => host.querySelector('[aria-label="Edition selection"] li') && pending.size === 0);
    assert.equal(navigationDispatches.length, before); assert(host.textContent.includes('Choose an edition of the first chapter'));
    button(host, 'Open complete-set preferred edition').click(); await until(() => navigationDispatches.length === before + 1 && pending.size === 0);
    assert.equal(navigationDispatches.at(-1), '/chapter/' + uuid(103) + '/1'); checks.push('multiple editions do not navigate before choice', 'actual chooser revalidates and dispatches selected winner');
  });
  await withMounted('observed device Continue', title(''), async (host, checks) => {
    const snapshot = await library.currentDeviceSnapshot(); assert.equal(snapshot.records.find(row => row.family === 'chapters' && row.id === uuid(100))?.completed, true);
    assert.equal(snapshot.records.find(row => row.family === 'chapters' && row.id === uuid(101))?.completed, false);
    const before = navigationDispatches.length; button(host, 'Continue scanning').click(); await until(() => host.querySelector('[aria-label="Observed continue candidate"]') && pending.size === 0);
    assert(host.textContent.includes('not an exact automatic Continue result')); assert.equal(navigationDispatches.length, before);
    button(host, 'Inspect candidate editions').click(); await until(() => host.querySelector('[aria-label="Edition selection"] li')?.textContent.includes(uuid(101)) && pending.size === 0);
    button(host, 'Open observed candidate').click(); await until(() => navigationDispatches.length === before + 1 && pending.size === 0);
    assert.equal(navigationDispatches.at(-1), '/chapter/' + uuid(101) + '/1'); checks.push('completed alternate edition excludes its observed group', 'partial history yields an explicitly observed candidate only', 'inspection and explicit open use the same candidate');
  });
  await withMounted('identity mismatch reload status', reader(102), async (host, checks) => {
    const details = [...host.querySelectorAll('details')].find(node => node.querySelector('summary')?.textContent === 'Chapters'); assert(details); details.open = true;
    await until(() => details.querySelectorAll('li').length === 2 && !env.reader.navigationState.pending);
    await phase('identity-mismatch'); const before = navigationDispatches.length;
    button(host, 'Restart chapter list').click(); await until(() => host.textContent.includes('Reload this page') && !env.reader.navigationState.pending);
    assert.equal(button(host, 'Reload this page').closest('details'), null);
    assert.equal(env.reader.navigationState.requiresReload, true); assert.equal(navigationDispatches.length, before);
    assert.equal(env.reader.currentPage.isLoaded, true); checks.push('server identity mismatch visibly requires reload', 'no navigation and directly readable images remain');
  });
  assert.deepEqual(mountedCases.map(row => row.name), [...BOUNDED_MOUNTED_CASES]);
  assert(mountedCases.every(row => row.outcome === 'passed' && row.assertions.length >= 2));
}
