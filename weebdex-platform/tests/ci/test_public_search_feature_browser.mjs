// Finite control/observer seams only. No browser, service, build or network runs.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { EventEmitter } from 'node:events';
import test from 'node:test';
import { publicSearchFeatureCases } from '../../scripts/public-search-feature-browser.mjs';

const origin = 'http://127.0.0.1:23456', id = number => '00000000-0000-8000-8000-' + String(number).padStart(12, '0');
const tags = count => Array.from({ length: count }, (_, index) => ({ id: id(index + 1), type: 'tag', attributes: { name: { en: 'Synthetic tag ' + index }, group: 'genre' } }));
const feed = data => ({ result: 'ok', response: 'collection', data, limit: data.length, offset: 0, total: data.length });
const titles = [{ id: id(999), type: 'manga', attributes: { title: { en: 'Synthetic fixture' } } }];
const labels = ['Safe', 'Suggestive', 'Erotica', 'Pornographic'];
class Locator {
  constructor(f, kind) { this.f = f; this.kind = kind; }
  locator(selector) {
    const mapping = { '#search': 'section', form: 'form', article: 'articles', 'article a[href^="/title/"]': 'links', 'button[data-slot="select-trigger"][aria-haspopup="listbox"]:visible': 'triggers', '[data-slot="select-content"][role="listbox"]:visible': 'list', '[data-slot="popover-content"]:visible': 'popup', 'label[for^="tag-"]': 'tagLabels', '[role="checkbox"][id^="tag-"]': 'tagControls', '[data-slot="select-item"][aria-selected="true"]': 'selected' };
    assert(mapping[selector], 'Unexpected finite selector'); this.f.selectors.add(selector); return new Locator(this.f, mapping[selector]);
  }
  filter(options) {
    if (options.hasText) {
      const candidates = [['sort', 'Sort: Uploaded Date'], ['order', `Order: ${this.f.order === 'asc' ? 'Ascending' : 'Descending'}`], ['rating', this.f.selected ? labels.slice(0, this.f.selected).join(', ') + ' +' + this.f.selected : 'Content Rating']];
      const matched = candidates.filter(([, text]) => options.hasText.test(text)); assert.equal(matched.length, 1, 'Real finite trigger text must match uniquely'); return new Locator(this.f, matched[0][0]);
    }
    assert(options.has); return this;
  }
  getByRole(role, options = {}) {
    if (role === 'heading') { assert.equal(options.name, 'Advanced Search'); return new Locator(this.f, 'heading'); }
    if (role === 'option') return new Locator(this.f, options.name === 'Ascending' ? 'ascending' : 'options');
    assert.equal(role, 'button'); const mapping = { Tags: 'tags', Search: 'submit', 'Content Rating': 'rating' }; assert(mapping[options.name]); return new Locator(this.f, mapping[options.name]);
  }
  getByPlaceholder(name) { assert(['Search...', 'Search tags...'].includes(name)); return new Locator(this.f, name === 'Search tags...' ? 'tagInput' : 'input'); }
  getByText(name) { assert.equal(name, 'No results found'); return new Locator(this.f, 'empty'); }
  async waitFor(options) { assert(options.timeout > 0 && options.timeout <= 8000); if (this.kind === 'empty') assert.equal(this.f.rendered.length, 0); if (options.state === 'hidden') assert.equal(this.f.popup, false); }
  async fill(value, options) { assert.equal(value, ''); assert(options.timeout > 0); }
  async click(options) {
    assert(options.timeout > 0 && options.timeout <= 8000); this.f.actions.push(this.kind);
    if (['tags', 'order', 'rating'].includes(this.kind)) this.f.popup = true;
    else if (this.kind === 'ascending') { this.f.order = 'asc'; this.f.popup = false; }
    else if (this.kind === 'selected') { assert(this.f.selected > 0); this.f.selected--; }
    else if (this.kind === 'submit') {
      if (!this.f.selected) { this.f.current = origin + '/search?contentRating=__none__'; this.f.rendered = []; if (this.f.config.emptyDispatch) this.f.emitRead('/api/v1/manga', feed(titles)); }
      else { this.f.current = origin + '/search?order=asc'; this.f.rendered = titles; this.f.emitRead('/api/v1/manga?order%5B' + (this.f.config.wrongSort ? 'title' : 'latestUploadedChapter') + '%5D=asc', feed(titles)); }
    } else assert.fail('Unexpected finite action');
  }
  first() { assert.equal(this.kind, 'selected'); return this; }
  async count() {
    if (this.kind === 'articles') return this.f.rendered.length;
    if (this.kind === 'selected') return this.f.selected;
    if (this.kind === 'tagControls') return this.f.config.tagCount ?? 2;
    assert.fail('Unexpected finite count');
  }
  async allTextContents() { assert.equal(this.kind, 'options'); return labels; }
  async evaluateAll(callback) {
    if (this.kind === 'tagLabels') return callback(tags(this.f.config.tagCount ?? 2).map((item, index) => ({ textContent: this.f.config.wrongTagLabel && index === 0 ? 'Wrong actual label' : item.attributes.name.en, getAttribute: name => { assert.equal(name, 'for'); return 'tag-' + item.id; } })));
    if (this.kind === 'links') return callback(this.f.rendered.map(item => ({ getAttribute: name => { assert.equal(name, 'href'); return '/title/' + item.id + '/actual-title'; } })));
    assert.fail('Unexpected finite DOM evaluation');
  }
}
class Page extends EventEmitter {
  constructor(config = {}) { super(); this.config = config; this.current = ''; this.order = 'desc'; this.selected = 3; this.rendered = titles; this.popup = false; this.actions = []; this.selectors = new Set(); this.now = 100000; this.keyboard = { press: async (key, options) => { assert.equal(key, 'Escape'); assert(options.timeout > 0); this.popup = false; } }; }
  locator(selector) { return new Locator(this, 'page').locator(selector); }
  getByPlaceholder(name) { return new Locator(this, 'page').getByPlaceholder(name); }
  emitRead(route, body, { terminal = true, status = 200, rejectBody = false } = {}) {
    const request = { url: () => origin + route, method: () => 'GET' }; this.emit('request', request);
    this.emit('response', { request: () => request, status: () => status, headers: () => ({ 'content-type': 'application/json' }), body: async () => { if (rejectBody) throw Error('Synthetic body failure'); return Buffer.from(JSON.stringify(body)); } });
    if (terminal) this.emit('requestfinished', request);
  }
  async goto(url, options) { assert.equal(url, origin + '/search'); assert(options.timeout > 0 && options.timeout <= 15000); this.current = url;
    this.emitRead(this.config.native ? '/api/manga' : '/api/v1/manga', feed(titles), { terminal: !this.config.unfinished, rejectBody: this.config.rejectBody });
    const tagBody = feed(tags(this.config.tagCount ?? 2)); if (this.config.truncatedTags) tagBody.total++;
    this.emitRead('/api/v1/manga/tag', tagBody); return { status: () => 200 };
  }
  async waitForURL(predicate, options) { assert(options.timeout > 0 && options.timeout <= 8000); assert(predicate(new URL(this.current)), 'Actual route predicate did not accept real finite transition'); }
  async waitForTimeout(milliseconds) { assert(milliseconds > 0 && milliseconds <= 50); this.now += milliseconds; await Promise.resolve(); }
}
async function run(t, index, config) {
  const page = new Page(config); t.mock.method(Date, 'now', () => page.now); const redactions = new Set();
  try { return await publicSearchFeatureCases[index].run({ page, context: { request: { get() { assert.fail('No independent or manufactured read permitted'); } } }, origin, fixture: {}, insist: (condition, message) => { if (!condition) throw Error(message); }, redactions }); }
  finally { for (const event of ['request', 'response', 'requestfinished', 'requestfailed']) assert.equal(page.listenerCount(event), 0, 'Every case releases its passive observers'); t.mock.restoreAll(); }
}

test('fixed immutable three-case contract imports without browser allocation', () => {
  assert(Object.isFrozen(publicSearchFeatureCases)); assert.deepEqual(publicSearchFeatureCases.map(row => row.id), ['public-search-complete-tag-options', 'public-search-default-sort-ascending', 'public-search-empty-rating-selection']);
  assert(publicSearchFeatureCases.every(row => Object.isFrozen(row) && typeof row.run === 'function' && Object.keys(row).sort().join() === 'id,run'));
  const source = fs.readFileSync(new URL('../../scripts/public-search-feature-browser.mjs', import.meta.url), 'utf8');
  for (const forbidden of ['.route(', '.fulfill(', 'addInitScript', 'localStorage', 'context.request', 'evaluate(()', '.launch(']) assert(!source.includes(forbidden), 'No production hook, browser allocation, transport synthesis or fixture mutation');
});
test('complete actual small tag snapshot agrees with genuine-shaped option labels and controls', async t => { const value = await run(t, 0); assert.deepEqual(value, { completed_public_snapshot: true, actual_tag_count: 2, exact_option_ids_and_labels: true, actual_checkbox_count: 2, pagination_boundary_observed: false }); });
test('finite-only 101-tag boundary checks every actual option, without changing the hosted fixture', async t => { const value = await run(t, 0, { tagCount: 101 }); assert.equal(value.actual_tag_count, 101); assert.equal(value.pagination_boundary_observed, true); });
test('incomplete public tag snapshot cannot receive a complete-options pass', async t => { await assert.rejects(() => run(t, 0, { truncatedTags: true }), /empty, shifted or incomplete/); });
test('same-size tag list with the wrong rendered label is rejected', async t => { await assert.rejects(() => run(t, 0, { wrongTagLabel: true }), /identities or labels differ/); });
test('real-shaped default-sort ascending gesture binds exact public query and rendered identities', async t => { const value = await run(t, 1); assert.equal(value.completed_public_ascending_query, true); assert.equal(value.rendered_response_ids_match, true); assert.equal(value.actual_result_count, 1); });
test('ascending gesture cannot pass a different public sort field', async t => { await assert.rejects(() => run(t, 1, { wrongSort: true }), /exact public sort/); });
test('four bounded rating options clear through actual gestures and require settled zero-request empty UI', async t => { const value = await run(t, 2); assert.equal(value.real_rating_controls_cleared, 3); assert.equal(value.catalogue_requests_after_submit, 0); assert.equal(value.quiet_window_ms, 500); });
test('empty-looking DOM cannot hide a newly dispatched catalogue search', async t => { await assert.rejects(() => run(t, 2, { emptyDispatch: true }), /unexpectedly dispatched/); });
test('native catalogue fallback fails immediately and detaches observers', async t => { await assert.rejects(() => run(t, 0, { native: true }), /native catalogue fallback/); });
test('HTTP200 body without a real finished terminal cannot pass initial readiness', async t => { await assert.rejects(() => run(t, 0, { unfinished: true }), /did not complete its actual public catalogue read/); });
test('HTTP200 headers with body-read failure cannot pass initial readiness', async t => { await assert.rejects(() => run(t, 0, { rejectBody: true }), /did not complete its actual public catalogue read/); });
test('selectors stay bound to preserved search form, Bits controls and tag labels', () => {
  const root = new URL('../../frontend/src/', import.meta.url), filter = fs.readFileSync(new URL('lib/components/MangaSearchFilter.svelte', root), 'utf8'), tag = fs.readFileSync(new URL('lib/components/TagComboBox.svelte', root), 'utf8'), select = fs.readFileSync(new URL('lib/components/ui/select/select-trigger.svelte', root), 'utf8'), popover = fs.readFileSync(new URL('lib/components/ui/popover/popover-content.svelte', root), 'utf8');
  for (const retained of ['<form ', 'placeholder="Search..."', 'bind:value={search.contentRating}', 'Sort: ', 'Order:', '>Ascending</Select.Item>', 'Tags</span>', 'type="submit"']) assert(filter.includes(retained));
  assert(tag.includes('for="tag-{tag.id}"')); assert(tag.includes('id="tag-{tag.id}"')); assert(tag.includes('Search tags...')); assert(select.includes('data-slot="select-trigger"')); assert(popover.includes('data-slot="popover-content"'));
});
