// Finite composed DOM coverage, not a browser, backend, or bits-ui popup test.
// Run with --experimental-vm-modules and the existing shared dependency paths.
import assert from 'node:assert/strict';
import test from 'node:test';
import { cleanup, ui, input, field, submit, reset, button, settle, searchQuery, fixture, envelope, titleRow, uuid, normalize, plain, prepareChapterComponent, productionComponent, BrowserSearchParams, oldLanguageDefaultError, snapshotComponent } from './public-catalog-filter-controls-fixture.mjs';
test.after(cleanup);
const params = text => new BrowserSearchParams(text);
const wire = request => new URL(request.path, 'https://fixture.invalid').searchParams;
async function searchRead(query) {
  const admitted = searchQuery.effectiveMangaSearchReadQuery(query, false, { applyMangaPreferences() {} }, '42');
  assert.equal(admitted.error, undefined);
  const f = fixture(); await f.read('manga.search', admitted.query); return f;
}

test('mounted retained Year From/To controls independently submit inclusive native 0..9999 bounds without becoming exact year', async () => {
  for (const [from, to] of [['0', ''], ['', '9999'], ['0', '9999'], ['2001', '2001'], ['1800', '2200']]) {
    const f = await ui();
    try {
      for (const name of ['Year From', 'Year To']) { assert.equal(field(f.host, name).min, '0'); assert.equal(field(f.host, name).max, '9999'); }
      input(field(f.host, 'Year From'), from); input(field(f.host, 'Year To'), to); submit(f.host);
      assert.equal(f.errors.length, 0); assert.equal(f.routes.length, 1);
      const url = new URL(f.routes[0], 'https://fixture.invalid');
      assert.equal(url.searchParams.get('yearFrom'), from || null); assert.equal(url.searchParams.get('yearTo'), to || null); assert.equal(url.searchParams.has('year'), false);
      const read = await searchRead(url.searchParams); assert.equal(read.requests.length, 1);
      assert.equal(wire(read.requests[0]).get('yearFrom'), from || null); assert.equal(wire(read.requests[0]).get('yearTo'), to || null); assert.equal(wire(read.requests[0]).has('year'), false);
    } finally { await f.dispose(); }
  }
});

test('mounted mixed filters preserve title, rating, language, tags, ordering and page-size while range Apply drops pagination', async () => {
  const original = `/search?title=Actual+name&contentRating=safe&lang=ja&status=ongoing&demographic=seinen&tag=${uuid(20)}&tagx=${uuid(21)}&tmod=OR&txmod=AND&sort=title&order=asc&limit=28&page=3&yearFrom=1980&yearTo=2020`;
  const f = await ui({ path: original });
  try {
    assert.equal(field(f.host, 'Year From').value, '1980'); assert.equal(field(f.host, 'Year To').value, '2020');
    input(field(f.host, 'Year From'), '1990'); submit(f.host);
    assert.equal(f.errors.length, 0); assert.equal(f.routes.length, 1);
    const url = new URL(f.routes[0], 'https://fixture.invalid'), q = url.searchParams;
    for (const [key, value] of [['title', 'Actual name'], ['contentRating', 'safe'], ['lang', 'ja'], ['status', 'ongoing'], ['demographic', 'seinen'], ['tag', uuid(20)], ['tagx', uuid(21)], ['tmod', 'OR'], ['txmod', 'AND'], ['sort', 'title'], ['order', 'asc'], ['limit', '28'], ['yearFrom', '1990'], ['yearTo', '2020']]) assert.equal(q.get(key), value, key);
    assert.equal(q.has('page'), false);
    const read = await searchRead(q), sent = wire(read.requests[0]);
    for (const [key, value] of [['yearFrom', '1990'], ['yearTo', '2020'], ['includedTags[]', uuid(20)], ['excludedTags[]', uuid(21)], ['originalLanguage[]', 'ja'], ['contentRating[]', 'safe'], ['order[title]', 'asc'], ['includedTagsMode', 'OR'], ['excludedTagsMode', 'AND'], ['limit', '28']]) assert.equal(sent.get(key), value, key);
  } finally { await f.dispose(); }
});

test('range route remount restores edited and Back URLs; Reset clears bounds and all abandoned query state', async () => {
  const prior = '/search?yearFrom=0&yearTo=2000&title=Old&page=7';
  let f = await ui({ path: prior }); let edited;
  try { input(field(f.host, 'Year From'), '1999'); submit(f.host); edited = f.routes[0]; } finally { await f.dispose(); }
  f = await ui({ path: edited });
  try { assert.equal(field(f.host, 'Year From').value, '1999'); assert.equal(field(f.host, 'Year To').value, '2000'); } finally { await f.dispose(); }
  f = await ui({ path: prior });
  try {
    assert.equal(field(f.host, 'Year From').value, '0'); assert.equal(field(f.host, 'Year To').value, '2000');
    reset(f.host); assert.equal(f.errors.length, 0); assert.equal(field(f.host, 'Year From').value, ''); assert.equal(field(f.host, 'Year To').value, '');
    const q = new URL(f.routes.at(-1), 'https://fixture.invalid').searchParams;
    for (const key of ['yearFrom', 'yearTo', 'year', 'title', 'page']) assert.equal(q.has(key), false, key);
    assert.deepEqual(q.getAll('contentRating'), ['safe', 'suggestive', 'erotica']);
    const read = await searchRead(q); assert.equal(wire(read.requests[0]).has('yearFrom'), false); assert.equal(wire(read.requests[0]).has('yearTo'), false);
  } finally { await f.dispose(); }
});

test('mounted range validation refuses reversed, duplicate, fractional and out-of-bounds URLs until corrected or reset', async () => {
  for (const invalid of ['yearFrom=2020&yearTo=2000', 'yearFrom=2000&yearFrom=2001', 'yearTo=2000&yearTo=2001', 'yearFrom=1.5', 'yearTo=1e3', 'yearFrom=-1', 'yearTo=10000']) {
    const f = await ui({ path: '/search?' + invalid });
    try { submit(f.host); assert.equal(f.routes.length, 0, invalid); assert.equal(f.errors.length, 1, invalid); reset(f.host); assert.equal(f.routes.length, 1, invalid); } finally { await f.dispose(); }
  }
  const f = await ui();
  try { input(field(f.host, 'Year From'), '2020'); input(field(f.host, 'Year To'), '2000'); submit(f.host); assert.equal(f.routes.length, 0); assert.equal(f.errors.length, 1); input(field(f.host, 'Year To'), '2020'); submit(f.host); assert.equal(f.routes.length, 1); } finally { await f.dispose(); }
});

test('public exact-year and year=none remain exact while mixed exact/range and malformed controls fail before transport', async () => {
  for (const value of ['1', '2001', '9999', 'none']) {
    const f = fixture(); await f.read('manga.search', params({ year: value })); assert.equal(wire(f.requests[0]).get('year'), value); assert.equal(wire(f.requests[0]).has('yearFrom'), false); assert.equal(wire(f.requests[0]).has('yearTo'), false);
  }
  for (const invalid of ['year=0', 'year=2000&yearFrom=2000', 'year=none&yearTo=9999', 'yearFrom=2000&yearTo=1999', 'yearFrom=2&yearFrom=2', 'yearFrom=2&yearFrom[]=2', 'yearTo=2&yearTo=2', 'yearFrom=-1', 'yearTo=10000', 'yearFrom=1.5', 'yearTo=1e3', 'yearFrom=none', 'yearTo=']) {
    const f = fixture(); await assert.rejects(() => f.read('manga.search', params(invalid)), undefined, invalid); assert.equal(f.requests.length, 0, invalid);
  }
});

test('chapter-parent composition carries independent and equal year ranges only to manga discovery', async () => {
  for (const bounds of [{ yearFrom: '0' }, { yearTo: '9999' }, { yearFrom: '2001', yearTo: '2001' }]) {
    const f = fixture(request => {
      const url = new URL(request.path, 'https://fixture.invalid');
      return url.pathname === '/manga' ? envelope([titleRow()], { limit: 100 }) : envelope([], { limit: 500 });
    });
    const response = await f.read('chapter.search', params({ ...bounds, tlang: 'en', limit: '42' }));
    assert.equal(response.body.total, 0); assert.equal(f.requests.length, 2);
    assert.equal(new URL(f.requests[0].path, 'https://fixture.invalid').pathname, '/manga');
    for (const [key, value] of Object.entries(bounds)) assert.equal(wire(f.requests[0]).get(key), value);
    assert.equal(wire(f.requests[0]).has('year'), false);
    assert.match(f.requests[1].path, new RegExp('/manga/' + uuid(1) + '/feed'));
    assert.equal(wire(f.requests[1]).has('yearFrom'), false); assert.equal(wire(f.requests[1]).has('yearTo'), false); assert.equal(wire(f.requests[1]).get('translatedLanguage[]'), 'en');
  }
});


test('exact old public-language fallback crashes a bound undefined search value, unlike the mounted fixed control', async () => {
  const error = await oldLanguageDefaultError(); assert.match(error?.message ?? '', /props_invalid_value/);
});


test('title normalization advertises paged full-title groups only with the explicit capability marker', () => {
  for (const attributes of [{}, { available_groups: [{ id: uuid(10), name: 'Untrusted legacy page group' }] }, { nostr: { availableGroups: [{ id: uuid(10), name: 'Unmarked group' }] } }, { nostr: { availableGroupSemantics: 'unknown' } }, { nostr: [] }, { nostr: null }, { nostr: 'visible-title-groups-v1' }, { nostr: { availableGroupSemantics: [] } }]) {
    const title = normalize.normalizeManga(titleRow(attributes)); assert.equal(Object.hasOwn(title.relationships, 'available_groups'), false);
  }
  const title = normalize.normalizeManga(titleRow({ nostr: { availableGroupSemantics: 'visible-title-groups-v1' } }));
  assert.deepEqual(plain(title.relationships.available_groups), []);
  assert.equal(title.title, 'Actual title'); assert.deepEqual(plain(title.relationships.available_languages), ['en', 'fr']);
});

let groupsPrepared;
async function prepareGroups(legacy=false) {
  if (groupsPrepared===legacy) return;
  const PublicTitleGroupFilter = legacy?await snapshotComponent('LegacyPublicTitleGroupFilter','PublicTitleGroupFilter-68a5e06.svelte'):await productionComponent('PublicTitleGroupFilter');
  await prepareChapterComponent({ PublicTitleGroupFilter },legacy); groupsPrepared = legacy;
}
const groupId = number => uuid(1000 + number);
const groupRow = number => ({ id: groupId(number), type: 'scanlation_group', attributes: { name: 'Actual group ' + number }, relationships: [] });
const markedTitle = () => normalize.normalizeManga(titleRow({ nostr: { availableGroupSemantics: 'visible-title-groups-v1' } }));
function groupFixture(count = 205) {
  let rows = Array.from({ length: count }, (_, index) => groupRow(index + 1)), hook;
  const f = fixture(async request => {
    const custom = await hook?.(request); if (custom !== undefined) return custom;
    const url = new URL(request.path, 'https://fixture.invalid');
    if (!url.pathname.endsWith('/available-groups')) return envelope([]);
    const q = url.searchParams, requested = q.getAll('ids[]'), scoped = requested.length ? rows.filter(row => requested.includes(row.id)) : rows;
    const remaining = scoped.filter(row => !q.has('after') || row.id > q.get('after')), limit = Number(q.get('limit')), data = remaining.slice(0, limit);
    return { status: 200, body: { result: 'ok', semantics: 'visible-title-groups-v1', data, total: scoped.length, position: scoped.length-remaining.length, limit, next: remaining.length > limit ? data.at(-1).id : null } };
  });
  return { ...f, rows: () => rows, setRows: value => { rows = value; }, hook: value => { hook = value; } };
}
async function groupUi(f, options = {}) {
  await prepareGroups(options.legacy??false);
  return ui({ chapter: true, title: markedTitle(), path: '/title/' + uuid(1), feed: { total: 1, data: [{ language: 'en', relationships: { groups: [{ id: groupId(1), name: 'First page only' }] } }] }, environment: { publicCatalogueRead: f.readOptions }, ...options });
}
function option(host, id) { const found = host.querySelector(`[role="option"][data-value="${id}"]`); assert.ok(found, 'group option ' + id); return found; }
function groupLabel(host) { return [...host.querySelectorAll('[data-fixture-select]')].at(-1).querySelector(':scope > button').textContent.trim(); }
function groupOptions(host) { return [...host.querySelectorAll('[role="option"]')].filter(node => node.textContent.startsWith('Actual group ')); }
const deferred = () => { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; };

test('actual composed ChapterListFilter pages all-title groups independently of first chapter page and language, then applies canonical UUID', async () => {
  const f = groupFixture(), view = await groupUi(f, { path: `/title/${uuid(1)}?tlang=en` });
  try {
    assert.equal(f.requests.length, 1); assert.equal(groupOptions(view.host).length, 100); assert.equal(groupLabel(view.host), 'Any Group');
    assert.match(view.host.textContent, /205 visible groups across all chapters and languages/);
    assert.equal(wire(f.requests[0]).has('tlang'), false); assert.equal(wire(f.requests[0]).has('translatedLanguage[]'), false);
    button(view.host, 'Next groups').click(); await settle(); assert.equal(f.requests.length, 2); assert.equal(groupOptions(view.host).length, 100); assert.equal(wire(f.requests[1]).get('after'), groupId(100));
    button(view.host, 'Next groups').click(); await settle(); assert.equal(f.requests.length, 3); assert.equal(groupOptions(view.host).length, 5); assert.equal(wire(f.requests[2]).get('after'), groupId(200));
    assert.equal([...view.host.querySelectorAll('button')].some(node => node.textContent === 'Next groups'), false);
    option(view.host, groupId(205)).click(); await settle(); assert.equal(groupLabel(view.host), 'Actual group 205'); assert.equal(f.requests.length, 3, 'a loaded later-page selection must not hydrate again');
    button(view.host, 'Apply').click(); await settle(); assert.equal(view.routes.length, 1);
    const applied = new URL(view.routes[0], 'https://fixture.invalid'); assert.equal(applied.searchParams.get('group'), groupId(205)); assert.equal(applied.searchParams.get('tlang'), 'en');
    assert.equal(applied.searchParams.toString().includes('Actual'), false); assert.equal(button(view.host, 'Apply').disabled, true);
    await f.read('chapter.list', applied.searchParams, uuid(1)); const chapter = f.requests.at(-1);
    assert.equal(new URL(chapter.path, 'https://fixture.invalid').pathname, '/chapter'); assert.equal(wire(chapter).get('groups[]'), groupId(205)); assert.equal(wire(chapter).get('manga'), uuid(1));
    assert.equal(groupOptions(view.host).length, 100, 'Apply starts a fresh first group page'); assert.equal(groupLabel(view.host), 'Actual group 205');
  } finally { await view.dispose(); }
});

test('selected group beyond page one hydrates actual title-scoped name without downloading intervening pages, and live Back/reset restore URL baseline', async () => {
  const f = groupFixture(), selected = `/title/${uuid(1)}?tlang=fr&group=${groupId(205)}&order=asc`, view = await groupUi(f, { path: selected });
  try {
    assert.equal(f.requests.length, 2); assert.equal(groupOptions(view.host).length, 100); assert.equal(groupLabel(view.host), 'Actual group 205');
    const hydration = f.requests.find(request => wire(request).has('ids[]')); assert.ok(hydration); assert.deepEqual(wire(hydration).getAll('ids[]'), [groupId(205)]);
    assert.equal(f.requests.some(request => wire(request).has('after')), false); assert.equal(button(view.host, 'Apply').disabled, true);
    option(view.host, groupId(1)).click(); await settle(); assert.match(groupLabel(view.host), /Actual group 205, Actual group 1/); assert.equal(button(view.host, 'Apply').disabled, false);
    view.navigate(`/title/${uuid(1)}?group=${groupId(2)}`); await settle(); assert.equal(groupLabel(view.host), 'Actual group 2'); assert.equal(button(view.host, 'Apply').disabled, true);
    view.navigate(selected); await settle(); assert.equal(groupLabel(view.host), 'Actual group 205'); assert.equal(button(view.host, 'Apply').disabled, true);
    reset(view.host); await settle(); assert.equal(groupLabel(view.host), 'Any Group'); assert.equal(view.routes.at(-1), '/title/' + uuid(1)); assert.equal(button(view.host, 'Reset').disabled, true); assert.equal(button(view.host, 'Apply').disabled, true);
  } finally { await view.dispose(); }
});

test('empty advertised group set is truthful, and absent extension neither fabricates controls nor reads a group endpoint', async () => {
  let f = groupFixture(0), view = await groupUi(f);
  try { assert.equal(f.requests.length, 1); assert.equal(groupOptions(view.host).length, 0); assert.equal(groupLabel(view.host), 'Any Group'); assert.match(view.host.textContent, /0 visible groups/); assert.equal(view.host.querySelector('[role="alert"]'), null); } finally { await view.dispose(); }
  f = groupFixture(); view = await groupUi(f, { title: normalize.normalizeManga(titleRow()) });
  try { assert.equal(f.requests.length, 0); assert.equal(groupOptions(view.host).length, 0); assert.doesNotMatch(view.host.textContent, /Any Group|Next groups|visible groups/); assert.ok(button(view.host, 'Apply')); } finally { await view.dispose(); }
});

test('group read failure keeps the title filter usable, shows explicit retry, and never silently falls back to page-local groups', async () => {
  const f = groupFixture(); f.hook(() => ({ status: 503, body: {} })); const view = await groupUi(f);
  try {
    assert.equal(groupOptions(view.host).length, 0); assert.match(view.host.textContent, /HTTP 503/); assert.match(view.host.textContent, /Title and chapters remain available/); assert.equal(f.requests.length, 1);
    assert.doesNotMatch(view.host.textContent, /First page only/); assert.ok(button(view.host, 'Apply'));
    f.hook(undefined); button(view.host, 'Retry groups').click(); await settle(); assert.equal(f.requests.length, 2); assert.equal(groupOptions(view.host).length, 100); assert.equal(view.host.querySelector('[role="alert"]'), null);
  } finally { await view.dispose(); }
});

test('loading a group page is bounded, disables pager, and unmount cancels delayed response without stale attachment', async () => {
  const f = groupFixture(), view = await groupUi(f), gate = deferred(); let disposed = false;
  try {
    f.hook(async request => { if (wire(request).has('after')) { await gate.promise; return { status: 503, body: {} }; } });
    button(view.host, 'Next groups').click(); await settle(); assert.equal(f.requests.length, 2); assert.match(view.host.textContent, /Loading groups/); assert.equal(button(view.host, 'Next groups').disabled, true);
    button(view.host, 'Next groups').click(); await settle(); assert.equal(f.requests.length, 2);
    const pending = f.requests[1]; await view.dispose(); disposed = true; assert.equal(pending.signal.aborted, true);
    gate.resolve(); await settle(); assert.equal(view.host.textContent, ''); assert.equal(view.routes.length, 0); assert.equal(f.requests.length, 2);
  } finally { gate.resolve(); if (!disposed) await view.dispose(); }
});

test('changing title-wide total while paging clears unreliable rows and offers a first-page restart', async () => {
  const f = groupFixture(), view = await groupUi(f);
  try {
    f.setRows(f.rows().slice(0, 204)); button(view.host, 'Next groups').click(); await settle();
    assert.equal(groupOptions(view.host).length, 0); assert.match(view.host.textContent, /Group choices changed/); assert.ok(button(view.host, 'First groups'));
    button(view.host, 'First groups').click(); await settle(); assert.equal(groupOptions(view.host).length, 100); assert.match(view.host.textContent, /204 visible groups/); assert.equal(view.host.querySelector('[role="alert"]'), null);
  } finally { await view.dispose(); }
});

test('missing selected title-scoped groups remain explicit and retryable instead of inventing a name or clearing the URL choice', async () => {
  const f = groupFixture(2), view = await groupUi(f, { path: `/title/${uuid(1)}?group=${groupId(205)}` });
  try {
    assert.equal(groupLabel(view.host), 'Selected group ' + groupId(205)); assert.match(view.host.textContent, /no longer visible for this title/); assert.equal(button(view.host, 'Apply').disabled, true);
    f.setRows([...f.rows(), groupRow(205)]); button(view.host, 'Retry selected names').click(); await settle(); assert.equal(groupLabel(view.host), 'Actual group 205'); assert.doesNotMatch(view.host.textContent, /no longer visible/);
  } finally { await view.dispose(); }
});

test('malformed, duplicate, unsorted, unrelated, oversized or unnamed group pages fail closed before installing options', async () => {
  const valid = { result: 'ok', semantics: 'visible-title-groups-v1', data: [groupRow(1)], total: 1, position: 0, limit: 100, next: null };
  const invalid = [
    { ...valid, semantics: 'unknown' }, { ...valid, data: {} }, { ...valid, data: [groupRow(1), groupRow(1)], total: 2 },
    { ...valid, data: [groupRow(2), groupRow(1)], total: 2 }, { ...valid, data: [{ ...groupRow(1), id: 'native-group' }] },
    { ...valid, data: [{ ...groupRow(1), attributes: {} }] }, { ...valid, data: [{ ...groupRow(1), attributes: { name: '' } }] },
    { ...valid, data: [{ ...groupRow(1), attributes: { name: 7 } }] }, { ...valid, data: [{ ...groupRow(1), attributes: { name: 'x'.repeat(4097) } }] },
    { ...valid, data: Array.from({ length: 101 }, (_, index) => groupRow(index + 1)), total: 101 },
    { ...valid, limit: 99 }, { ...valid, total: 0 }, { ...valid, next: groupId(2) },
    { ...valid, data: Array.from({ length: 100 }, (_, index) => groupRow(index + 1)), total: 205, next: null }
  ];
  for (const body of invalid) { const f = fixture(() => ({ status: 200, body })); await assert.rejects(() => f.read('manga.availableGroups', params({ limit: '100' }), uuid(1))); assert.equal(f.requests.length, 1); }
  const f = fixture(() => ({ status: 200, body: valid })); await assert.rejects(() => f.read('manga.availableGroups', params([['ids[]', groupId(2)]]), uuid(1)));
  const mounted = groupFixture(); mounted.hook(() => ({ status: 200, body: invalid[2] })); const view = await groupUi(mounted);
  try { assert.equal(groupOptions(view.host).length, 0); assert.ok(view.host.querySelector('[role="alert"]')); assert.ok(button(view.host, 'Retry groups')); assert.doesNotMatch(view.host.textContent, /visible groups across/); } finally { await view.dispose(); }
});

test('unsupported group query controls and invalid cursors fail before transport; selected IDs are canonicalized and bounded', async () => {
  for (const text of ['limit=0', 'limit=101', 'limit=1&limit=2', 'offset=100', 'tlang=en', 'after=not-a-uuid', `after=${groupId(1)}&after=${groupId(2)}`, `after=${groupId(1)}&ids[]=${groupId(2)}`, 'ids[]=native-group']) {
    const f = groupFixture(); await assert.rejects(() => f.read('manga.availableGroups', params(text), uuid(1)), undefined, text); assert.equal(f.requests.length, 0);
  }
  const tooMany = params(); for (let index = 1; index <= 101; index++) tooMany.append('ids[]', groupId(index));
  const f = groupFixture(); await assert.rejects(() => f.read('manga.availableGroups', tooMany, uuid(1))); assert.equal(f.requests.length, 0);
  const id = 'abcd0000-0000-8000-8000-000000000001', body = { result: 'ok', semantics: 'visible-title-groups-v1', data: [{ id: id.toUpperCase(), type: 'scanlation_group', attributes: { name: 'Actual canonical group' }, relationships: [] }], total: 1, position: 0, limit: 100, next: null };
  const upper = fixture(() => ({ status: 200, body })), response = await upper.read('manga.availableGroups', params([['ids[]', id.toUpperCase()]]), uuid(1));
  assert.equal(wire(upper.requests[0]).get('ids[]'), id); assert.equal(response.body.data[0].id, id); assert.equal(response.body.data[0].name, 'Actual canonical group');
});


test('Back during delayed selected-name hydration aborts the old lookup and cannot replace the new applied choice', async () => {
  const f = groupFixture(), gate = deferred();
  f.hook(async request => {
    if (wire(request).get('ids[]') === groupId(205)) { await gate.promise; return { status: 200, body: { result: 'ok', semantics: 'visible-title-groups-v1', data: [groupRow(205)], total: 1, position: 0, limit: 100, next: null } }; }
  });
  const view = await groupUi(f, { path: `/title/${uuid(1)}?group=${groupId(205)}` });
  try {
    assert.equal(f.requests.length, 2); assert.equal(groupLabel(view.host), 'Selected group ' + groupId(205));
    const pending = f.requests.find(request => wire(request).has('ids[]'));
    view.navigate(`/title/${uuid(1)}?group=${groupId(1)}`); await settle();
    assert.equal(pending.signal.aborted, true); assert.equal(groupLabel(view.host), 'Actual group 1');
    gate.resolve(); await settle(); assert.equal(groupLabel(view.host), 'Actual group 1'); assert.equal(button(view.host, 'Apply').disabled, true); assert.doesNotMatch(view.host.textContent, /Actual group 205/);
  } finally { gate.resolve(); await view.dispose(); }
});

test('changing the title while its first group page is pending cancels the old request and cannot attach another title choices', async () => {
  const f = groupFixture(2), gate = deferred();
  f.hook(async request => {
    if (new URL(request.path, 'https://fixture.invalid').pathname === `/manga/${uuid(1)}/available-groups`) { await gate.promise; return { status: 200, body: { result: 'ok', semantics: 'visible-title-groups-v1', data: [groupRow(205)], total: 1, position: 0, limit: 100, next: null } }; }
  });
  const view = await groupUi(f);
  try {
    assert.equal(f.requests.length, 1); assert.match(view.host.textContent, /Loading groups/);
    const next = normalize.normalizeManga({ ...titleRow({ nostr: { availableGroupSemantics: 'visible-title-groups-v1' } }), id: uuid(2) });
    view.setTitle(next); view.navigate('/title/' + uuid(2)); await settle();
    assert.ok(f.requests.length>=2&&f.requests.length<=3); assert.equal(f.requests[0].signal.aborted, true); assert.equal(groupOptions(view.host).length, 2);
    assert.equal(new URL(f.requests[1].path, 'https://fixture.invalid').pathname, `/manga/${uuid(2)}/available-groups`);
    gate.resolve(); await settle(); assert.equal(groupOptions(view.host).length, 2); assert.doesNotMatch(view.host.textContent, /Actual group 205/); assert.equal(view.host.querySelector('[role="alert"]'), null);
  } finally { gate.resolve(); await view.dispose(); }
});


test('same-count reordered continuation is rejected without showing a partial next page or losing explicit restart', async () => {
  const f = groupFixture(), view = await groupUi(f);
  try {
    f.hook(request => {
      if (!wire(request).has('after')) return;
      const data = f.rows().slice(100, 200); [data[0], data[1]] = [data[1], data[0]];
      return { status: 200, body: { result: 'ok', semantics: 'visible-title-groups-v1', data, total: 205, position: 100, limit: 100, next: groupId(200) } };
    });
    button(view.host, 'Next groups').click(); await settle(); assert.equal(groupOptions(view.host).length, 0); assert.ok(view.host.querySelector('[role="alert"]')); assert.ok(button(view.host, 'First groups'));
    assert.doesNotMatch(view.host.textContent, /100 choices on this page/);
    f.hook(undefined); button(view.host, 'First groups').click(); await settle(); assert.equal(groupOptions(view.host).length, 100); assert.equal(view.host.querySelector('[role="alert"]'), null);
  } finally { await view.dispose(); }
});

test('101 already-named draft selections show the exact limit and cannot become an oversized chapter request', async () => {
  const f = groupFixture(), q = params(); for (let index=1;index<=100;index++)q.append('group',groupId(index));
  const view=await groupUi(f,{path:`/title/${uuid(1)}?${q.toString().replace(/^\?/,'')}`});
  try{
    button(view.host,'Next groups').click();await settle();const before=f.requests.length;
    option(view.host,groupId(101)).click();await settle();assert.match(view.host.textContent,/At most 100 groups can be selected/);assert.equal(f.requests.length,before);
    q.append('group',groupId(101));await assert.rejects(()=>f.read('chapter.list',q,uuid(1)));assert.equal(f.requests.length,before);
    option(view.host,groupId(101)).click();await settle();assert.doesNotMatch(view.host.textContent,/At most 100 groups can be selected/);
  }finally{await view.dispose();}
});

test('every continuation rejects premature end or changed rank without retaining all visited rows and offers First groups',async()=>{
 for(const defect of ['missing-next','rank-shift']){
  const f=groupFixture(),view=await groupUi(f);
  try{
   f.hook(request=>{if(!wire(request).has('after'))return;const start=defect==='rank-shift'?101:100;return{status:200,body:{result:'ok',semantics:'visible-title-groups-v1',data:f.rows().slice(start,start+100),total:205,position:start,limit:100,next:defect==='missing-next'?null:groupId(start+100)}}});
   button(view.host,'Next groups').click();await settle();assert.equal(groupOptions(view.host).length,0);assert.ok(view.host.querySelector('[role="alert"]'));assert.ok(button(view.host,'First groups'));
   f.hook(undefined);button(view.host,'First groups').click();await settle();assert.equal(groupOptions(view.host).length,100);assert.equal(view.host.querySelector('[role="alert"]'),null);
  }finally{await view.dispose();}
 }
});

test('actual Apply with101 draft groups keeps feed and URL intact; exact68a5e06 composed control reproduces destructive navigation',async()=>{
 for(const legacy of [true,false]){
  const f=groupFixture(),q=params();for(let index=1;index<=100;index++)q.append('group',groupId(index));
  const view=await groupUi(f,{legacy,path:`/title/${uuid(1)}?${q.toString().replace(/^\?/,'')}`});
  try{
   button(view.host,'Next groups').click();await settle();option(view.host,groupId(101)).click();await settle();
   const originalFeed=view.feed.data;assert.ok(originalFeed);assert.equal(button(view.host,'Apply').disabled,false);
   button(view.host,'Apply').click();await settle();
   if(legacy){assert.equal(view.routes.length,1);assert.equal(view.feed.data,undefined);}
   else{assert.equal(view.routes.length,0);assert.equal(view.feed.data,originalFeed);assert.match(view.host.querySelector('[role="alert"]').textContent,/At most 100/);assert.match(groupLabel(view.host),/Actual group 101/);}
  }finally{await view.dispose();}
 }
});

test('same-title Back and Reset cancel delayed Next; exact68a5e06 component reproduces stale page installation',async()=>{
 for(const legacy of [true,false])for(const action of ['back','reset-same-url','reset-applied-url']){
  const f=groupFixture(),gate=deferred(),view=await groupUi(f,{legacy,path:`/title/${uuid(1)}${action==='reset-applied-url'?'?tlang=fr':''}`});
  try{
   if(action==='reset-same-url'){option(view.host,groupId(1)).click();await settle();}
   f.hook(async request=>{if(!wire(request).has('after'))return;await gate.promise;return{status:200,body:{result:'ok',semantics:'visible-title-groups-v1',data:f.rows().slice(100,200),total:205,position:100,limit:100,next:groupId(200)}}});
   button(view.host,'Next groups').click();await settle();const pending=f.requests.at(-1);assert.equal(wire(pending).get('after'),groupId(100));
   if(action==='back')view.navigate(`/title/${uuid(1)}?group=${groupId(1)}`);else button(view.host,'Reset').click();await settle();
   assert.equal(pending.signal.aborted,!legacy,action);gate.resolve();await settle();
   assert.equal(groupOptions(view.host).length,100);assert.equal(groupOptions(view.host)[0].dataset.value,groupId(legacy?101:1));
  }finally{gate.resolve();await view.dispose();}
 }
});
