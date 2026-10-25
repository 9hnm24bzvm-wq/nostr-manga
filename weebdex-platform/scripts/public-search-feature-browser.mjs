// Prospective genuine-browser cases. Importing allocates no browser or fixture.
// Only real preserved UI actions and passive network/DOM observations are used.
import { selectListboxTrigger, searchRatingTrigger } from './browser-select-controls.mjs';

const timeout = 8000, caseBudget = 45000, rowBudget = 32, bodyBudget = 2 * 1024 * 1024;
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const ratings = ['Safe', 'Suggestive', 'Erotica', 'Pornographic'];
const plain = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const localized = value => plain(value) ? (typeof value.en === 'string' ? value.en : Object.values(value).find(value => typeof value === 'string') ?? '') : '';

async function boundedRead(operation, milliseconds, message) {
  let timer;
  try { return await Promise.race([Promise.resolve().then(operation), new Promise((_, reject) => { timer = setTimeout(() => reject(Error(message)), milliseconds); })]); }
  finally { clearTimeout(timer); }
}

function observe(page, origin, insist) {
  const rows = [], byRequest = new WeakMap(); let overflow = false, closed = false;
  const match = value => { try { const url = new URL(value.url()); return url.origin === origin && ['/api/v1/manga', '/api/v1/manga/tag', '/api/manga', '/api/manga/tag', '/api/tag'].includes(url.pathname); } catch { return false; } };
  const request = value => {
    if (!match(value) || closed) return;
    if (rows.length >= rowBudget) { overflow = true; return; }
    const row = { url: new URL(value.url()), method: value.method(), status: null, terminal: null, body: undefined, bodyDone: false, bodyFailed: false };
    rows.push(row); byRequest.set(value, row);
  };
  const response = value => {
    const row = byRequest.get(value.request()); if (!row || closed) return;
    row.status = value.status();
    // A200 header is insufficient. Require bounded decoded JSON AND the real
    // requestfinished terminal before any case admits a successful snapshot.
    boundedRead(() => value.body(), timeout, 'Search response body deadline exceeded').then(bytes => {
      if (closed) return;
      if (bytes.length > bodyBudget || !(value.headers()['content-type'] ?? '').includes('application/json')) { row.bodyFailed = true; return; }
      try { row.body = JSON.parse(bytes.toString('utf8')); row.bodyDone = true; } catch { row.bodyFailed = true; }
    }).catch(() => { if (!closed) row.bodyFailed = true; });
  };
  const finished = value => { const row = byRequest.get(value); if (row) row.terminal = 'finished'; };
  const failed = value => { const row = byRequest.get(value); if (row) row.terminal = 'failed'; };
  page.on('request', request); page.on('response', response); page.on('requestfinished', finished); page.on('requestfailed', failed);
  return {
    rows,
    healthy() { insist(!overflow, 'Search observation exceeded its fixed request budget'); insist(!rows.some(row => !row.url.pathname.startsWith('/api/v1/')), 'Search used a native catalogue fallback'); insist(rows.every(row => row.method === 'GET'), 'Search issued an unexpected catalogue mutation'); },
    close() { closed = true; page.off('request', request); page.off('response', response); page.off('requestfinished', finished); page.off('requestfailed', failed); },
  };
}

async function start(h) {
  const { page, origin, insist, redactions } = h, target = new URL(origin);
  insist(target.origin === origin && target.protocol === 'http:' && ['127.0.0.1', 'localhost'].includes(target.hostname) && target.port && !target.username && !target.password, 'Search requires the existing exact owned loopback origin');
  const deadline = Date.now() + caseBudget, seen = observe(page, origin, insist);
  const options = (maximum = timeout) => { const remaining = deadline - Date.now(); insist(remaining > 0, 'Search case exhausted its fixed 45-second deadline'); return { timeout: Math.min(maximum, remaining) }; };
  const read = operation => boundedRead(operation, options().timeout, 'Search DOM observation deadline exceeded');
  const until = async (read, valid, message) => {
    while (Date.now() < deadline) { seen.healthy(); const value = await boundedRead(read, options().timeout, 'Search observation deadline exceeded'); if (valid(value)) return value; await page.waitForTimeout(50); }
    throw Error(message);
  };
  const successful = row => row.status === 200 && row.terminal === 'finished' && row.bodyDone && !row.bodyFailed;
  const collection = (row, type, maximum) => {
    insist(successful(row), 'Search did not receive a completed JSON200 response'); const body = row.body;
    insist(body?.result === 'ok' && body.response === 'collection' && Array.isArray(body.data) && body.data.length <= maximum && Number.isSafeInteger(body.total) && body.total >= body.data.length && Number.isSafeInteger(body.offset) && body.offset >= 0, 'Search collection shape or bound differs');
    const ids = new Set();
    for (const item of body.data) { insist(item?.type === type && uuid.test(item.id) && !ids.has(item.id), 'Search collection identities differ'); ids.add(item.id); redactions?.add(item.id); }
    return body;
  };
  try {
    const response = await page.goto(origin + '/search', { waitUntil: 'domcontentloaded', timeout: options(15000).timeout });
    insist(response?.status() === 200, 'Search document was not HTTP200');
    const section = page.locator('#search'); await section.getByRole('heading', { name: 'Advanced Search', exact: true }).waitFor({ state: 'visible', ...options() });
    const form = section.locator('form'); await form.getByPlaceholder('Search...', { exact: true }).waitFor({ state: 'visible', ...options() });
    const baseline = await until(() => seen.rows.find(row => row.url.pathname === '/api/v1/manga' && successful(row)), Boolean, 'Initial search did not complete its actual public catalogue read');
    collection(baseline, 'manga', 100);
    await until(() => section.locator('article').count(), count => count === baseline.body.data.length, 'Initial catalogue body and rendered cards differ');
    // Do not switch preferences, sign in, seed data or use a fixture-control API.
    return { ...h, seen, section, form, until, successful, collection, options, read };
  } catch (error) { seen.close(); throw error; }
}

async function tagOptions(h) {
  const s = await start(h), { page, form, seen, until, successful, collection, insist, options, read } = s;
  try {
    const row = await until(() => seen.rows.find(row => row.url.pathname === '/api/v1/manga/tag' && successful(row)), Boolean, 'Tag selector did not complete its actual public snapshot read');
    insist(!row.url.search, 'Tag snapshot unexpectedly carried unsupported public query controls');
    const body = collection(row, 'tag', 4096); insist(body.offset === 0 && body.data.length === body.total && body.total > 0, 'Tag snapshot was empty, shifted or incomplete');
    const expected = body.data.map(item => ({ id: 'tag-' + item.id, name: localized(item.attributes?.name) })).sort((a, b) => a.id.localeCompare(b.id));
    insist(expected.every(item => item.name.length > 0), 'Actual tag snapshot lacks a display name');
    await form.getByRole('button', { name: 'Tags', exact: true }).click(options());
    const popup = page.locator('[data-slot="popover-content"]:visible').filter({ has: page.getByPlaceholder('Search tags...', { exact: true }) });
    await popup.waitFor({ state: 'visible', ...options() });
    await popup.getByPlaceholder('Search tags...', { exact: true }).fill('', options());
    const actual = await until(() => popup.locator('label[for^="tag-"]').evaluateAll(nodes => nodes.map(node => ({ id: node.getAttribute('for'), name: node.textContent.trim() })).sort((a, b) => a.id.localeCompare(b.id))), rows => rows.length === expected.length, 'Tag options did not contain the complete actual snapshot');
    insist(JSON.stringify(actual) === JSON.stringify(expected), 'Rendered tag identities or labels differ from the actual snapshot');
    insist(await read(() => popup.locator('[role="checkbox"][id^="tag-"]').count()) === expected.length, 'Tag options lack their real checkbox controls');
    await page.keyboard.press('Escape', options()); await popup.waitFor({ state: 'hidden', ...options() }); seen.healthy();
    return { completed_public_snapshot: true, actual_tag_count: expected.length, exact_option_ids_and_labels: true, actual_checkbox_count: expected.length, pagination_boundary_observed: expected.length > 100 };
  } finally { seen.close(); }
}

async function ascending(h) {
  const s = await start(h), { page, origin, form, section, seen, until, successful, collection, insist, options, read } = s;
  try {
    await selectListboxTrigger(form).filter({ hasText: /^\s*Sort:\s*Uploaded Date\s*$/ }).waitFor({ state: 'visible', ...options() });
    await selectListboxTrigger(form).filter({ hasText: /^\s*Order:\s*Descending\s*$/ }).click(options());
    const list = page.locator('[data-slot="select-content"][role="listbox"]:visible');
    await list.getByRole('option', { name: 'Ascending', exact: true }).click(options());
    await selectListboxTrigger(form).filter({ hasText: /^\s*Order:\s*Ascending\s*$/ }).waitFor({ state: 'visible', ...options() });
    const boundary = seen.rows.length; await form.getByRole('button', { name: 'Search', exact: true }).click(options());
    await page.waitForURL(url => url.origin === origin && url.pathname === '/search' && url.searchParams.get('order') === 'asc' && !url.searchParams.has('sort'), options());
    const row = await until(() => seen.rows.slice(boundary).find(row => row.url.pathname === '/api/v1/manga' && successful(row)), Boolean, 'Ascending control did not complete its actual public catalogue read');
    insist(row.url.searchParams.getAll('order[latestUploadedChapter]').join(',') === 'asc' && !row.url.searchParams.has('order') && !row.url.searchParams.has('sort'), 'Ascending default sort was not translated into the exact public sort');
    const body = collection(row, 'manga', 100); insist(body.data.length > 0, 'Ascending query produced no genuine fixture result');
    const expected = body.data.map(item => item.id).sort();
    const actual = await until(() => section.locator('article a[href^="/title/"]').evaluateAll(nodes => [...new Set(nodes.map(node => node.getAttribute('href').split('/')[2]))].sort()), ids => JSON.stringify(ids) === JSON.stringify(expected), 'Ascending public result identities and rendered cards differ');
    seen.healthy(); return { real_order_control: true, route_omits_default_sort: true, completed_public_ascending_query: true, rendered_response_ids_match: true, actual_result_count: actual.length };
  } finally { seen.close(); }
}

async function emptyRatings(h) {
  const s = await start(h), { page, origin, form, section, seen, until, insist, options, read } = s;
  try {
    await searchRatingTrigger(page).click(options());
    const list = page.locator('[data-slot="select-content"][role="listbox"]:visible'); await list.waitFor({ state: 'visible', ...options() });
    const labels = await read(() => list.getByRole('option').allTextContents());
    insist(labels.length === 4 && labels.map(value => value.trim()).sort().join('|') === [...ratings].sort().join('|'), 'Rating popup differs from the four preserved real controls');
    const selected = list.locator('[data-slot="select-item"][aria-selected="true"]'); let count = await read(() => selected.count()), cleared = 0;
    insist(count > 0 && count <= 4, 'Initial rating selection is not a nonempty bounded control state');
    while (count > 0 && cleared < 4) { await selected.first().click(options()); const next = count - 1; await until(() => selected.count(), value => value === next, 'A real rating option did not deselect'); count = next; cleared++; }
    insist(count === 0, 'Rating options did not clear'); await page.keyboard.press('Escape', options()); await list.waitFor({ state: 'hidden', ...options() });
    await form.getByRole('button', { name: 'Content Rating', exact: true }).waitFor({ state: 'visible', ...options() });
    const boundary = seen.rows.length; await form.getByRole('button', { name: 'Search', exact: true }).click(options());
    await page.waitForURL(url => url.origin === origin && url.pathname === '/search' && url.searchParams.getAll('contentRating').join(',') === '__none__', options());
    await section.getByText('No results found', { exact: true }).waitFor({ state: 'visible', ...options() });
    insist(await read(() => section.locator('article').count()) === 0, 'Empty rating selection retained result cards');
    // The empty bridge deliberately emits no public HTTP request. The settled
    // empty UI plus this quiet window are observed, rather than forging a200.
    insist(options().timeout >= 500, 'Empty rating observation lacks its bounded quiet window');
    const quietUntil = Date.now() + 500;
    while (Date.now() < quietUntil) { seen.healthy(); insist(!seen.rows.slice(boundary).some(row => ['/api/v1/manga', '/api/manga'].includes(row.url.pathname)), 'Empty rating selection unexpectedly dispatched a catalogue search'); await page.waitForTimeout(50); }
    seen.healthy(); return { real_rating_controls_cleared: cleared, explicit_empty_route: true, empty_results_visible: true, rendered_card_count: 0, catalogue_requests_after_submit: 0, quiet_window_ms: 500 };
  } finally { seen.close(); }
}

export const publicSearchFeatureCases = Object.freeze([
  Object.freeze({ id: 'public-search-complete-tag-options', run: tagOptions }),
  Object.freeze({ id: 'public-search-default-sort-ascending', run: ascending }),
  Object.freeze({ id: 'public-search-empty-rating-selection', run: emptyRatings }),
]);
