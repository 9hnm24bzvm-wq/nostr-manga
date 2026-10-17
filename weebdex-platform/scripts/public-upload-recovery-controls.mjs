// Newly reconstructed, source-only controls for the preserved signed upload UI.
// Importing starts no browser, fixture, socket, build or process. Browser actions
// are genuine locator/pointer actions; evaluate is restricted to reading DOM.
const PANEL = 'section:has(> header > h2:text-is("Upload Chapter"))';
const GRID = ':scope > form > div.space-y-4 > div.grid[style*="--columns"]';
const CARDS = ':scope > div.relative[class~="!visible"]';
const SHADOW = 'data-is-dnd-shadow-item-internal';
const ORIGINAL = 'data-is-dnd-original-dragged-item';
const CLONE = 'dnd-action-dragged-el';
const exactText = text => new RegExp('^\\s*' + text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '\\s*$');
export const uploadPanel = page => page.locator(PANEL);
export const uploadPageGrid = page => uploadPanel(page).locator(GRID);
export const uploadPageCards = page => uploadPageGrid(page).locator(CARDS);
export const uploadPageCard = (page, name) => uploadPageCards(page).filter({ has: page.locator('h3:text-is(' + JSON.stringify(name) + ')') });
export const uploadPanelButton = (page, label) => uploadPanel(page).locator(':scope > form button[data-slot="button"]:visible').filter({ hasText: exactText(label) });

function filenames(names) {
  if (!Array.isArray(names) || names.some(name => typeof name !== 'string' || !name || name.trim() !== name || /[\x00-\x1f\x7f]/.test(name)) || new Set(names).size !== names.length) throw Error('Require unique exact synthetic upload filenames');
  return [...names];
}
const same = (a, b) => a.length === b.length && a.every((value, index) => value === b[index]);

// This function executes against the one strict grid locator. Global queries
// inspect only the installed DnD 0.9.69 lifecycle markers, never filename text.
function domSnapshot(grid, markers) {
  const children = Array.from(grid.children, child => ({
    card: child.matches('div.relative[class~="!visible"]'),
    add: child.matches('div') && !child.matches('div.relative[class~="!visible"]') && child.querySelectorAll('button[data-slot="button"]').length === 1 && child.querySelectorAll('h3').length === 0,
    shadow: child.hasAttribute(markers.shadow),
    dragging: child.classList.contains('dragging'),
    names: Array.from(child.querySelectorAll('h3'), node => node.textContent.trim()),
    images: Array.from(child.querySelectorAll('img'), node => ({ alt: node.getAttribute('alt'), decoded: node.complete && node.naturalWidth > 0 && node.naturalHeight > 0 })),
    // Both preserved ImageLoader and signed preview placeholder render this
    // exact Empty.Root > Spinner topology while the actual image is loading.
    loading: child.querySelectorAll('[data-slot="empty"] > [role="status"][aria-label="Loading"]').length,
  }));
  return {
    children,
    clones: grid.ownerDocument.querySelectorAll('[id="' + markers.clone + '"]').length,
    originals: grid.ownerDocument.querySelectorAll('[' + markers.original + ']').length,
    shadows: grid.ownerDocument.querySelectorAll('[' + markers.shadow + ']').length,
  };
}

// Classification never catches driver errors. Only explicit library markers
// may postpone strict topology/name assertions during a real drag lifecycle.
function inspect(snapshot, expected) {
  for (const key of ['clones', 'originals', 'shadows']) {
    if (!Number.isInteger(snapshot[key]) || snapshot[key] < 0 || snapshot[key] > 1) throw Error('Unexpected upload DnD ' + key + ' count');
  }
  const cards = snapshot.children.filter(child => child.card);
  const shadows = cards.filter(card => card.shadow || card.dragging);
  if (shadows.length > 1 || snapshot.shadows !== cards.filter(card => card.shadow).length) throw Error('Unexpected upload DnD shadow scope');
  if (snapshot.children.length !== cards.length + 1 || !snapshot.children.at(-1)?.add || snapshot.children.slice(0, -1).some(child => !child.card)) throw Error('Unexpected upload grid child topology');
  // Even during a genuine lifecycle, an extra card or an unknown/conflicting
  // filename is never a readiness state. Only one shadow may lack its heading.
  if (cards.length !== expected.length) throw Error('Upload page card count differs from expected');
  const presentNames = [];
  for (const card of cards) {
    if (card.names.length > 1 || card.images.length > 1) throw Error('Conflicting upload page filename topology');
    if (card.names.length === 1) {
      if (!expected.includes(card.names[0])) throw Error('Unknown upload page filename');
      presentNames.push(card.names[0]);
    }
    if (card.images.length === 1 && (!expected.includes(card.images[0].alt) || card.names.length === 1 && card.images[0].alt !== card.names[0])) throw Error('Conflicting upload page filename image');
  }
  if (new Set(presentNames).size !== presentNames.length) throw Error('Duplicate upload page filename');
  const dndSettling = snapshot.clones === 1 || snapshot.originals === 1 || snapshot.shadows === 1 || shadows.length === 1;
  // A global clone cannot excuse an unmarked broken card. The genuine shadow
  // may omit its content; every other card needs its actual loader or filename.
  // The only independent preview-pending branch is the actual loader topology
  // with no heading or image. It cannot excuse a wrong or conflicting filename.
  const previewLoading = cards.some(card => card.names.length === 0 && card.images.length === 0 && card.loading === 1);
  for (const card of cards) {
    if (dndSettling && (card.shadow || card.dragging)) continue;
    if (card.names.length === 0 && card.images.length === 0 && card.loading === 1) continue;
    if (card.names.length !== 1 || !card.names[0] || card.images.length !== 1 || card.images[0].alt !== card.names[0]) throw Error('Upload page requires one exact filename heading and matching image');
    if (!card.images[0].decoded) throw Error('Upload page image has not decoded');
  }
  if (dndSettling) return { pending: 'dnd-settling', cards, shadows };
  if (previewLoading) return { pending: 'preview-loading', cards, shadows };
  const names = cards.map(card => card.names[0]);
  if (!same(names, expected)) throw Error('Upload page order differs from expected');
  return { pending: null, cards, shadows, names };
}

/**
 * Every public operation receives one absolute deadline, including all reads,
 * health checks, pointer movement, polling, release checks and final validation.
 * The caller owns browser/fixture lifecycle and must supply assertHealthy.
 * Pointer command promises cannot be cancelled by Playwright; a late resolution
 * is rejected after await, and an attempted down always gets an attempted up.
 */
export function createPublicUploadRecoveryControls(page, {
  origin, pathname, assertHealthy, timeoutMs = 10000, pollMs = 25,
  now = Date.now, sleep = ms => new Promise(resolve => setTimeout(resolve, ms)),
} = {}) {
  const base = new URL(origin);
  if (base.origin !== origin || !/^https?:$/.test(base.protocol) || base.username || base.password || typeof pathname !== 'string' || !pathname.startsWith('/') || pathname.startsWith('//') || new URL(pathname, base).pathname !== pathname || typeof assertHealthy !== 'function' || !Number.isFinite(timeoutMs) || timeoutMs <= 0 || !Number.isFinite(pollMs) || pollMs <= 0 || typeof now !== 'function' || typeof sleep !== 'function') throw Error('Invalid bounded upload controls contract');
  let fenced = false, primaryError = null, active = false;
  let cleanup = { downAttempted: false, releaseAttempted: false, releaseSucceeded: false, releaseError: null, guardErrors: [] };
  const location = () => {
    const current = new URL(page.url());
    if (current.origin !== origin || current.pathname !== pathname || current.search || current.hash || current.username || current.password) throw Error('Upload controls left the exact owned origin or route');
  };
  const remaining = deadline => {
    const value = deadline - now();
    if (!(value > 0)) throw Error('Upload controls absolute deadline exceeded');
    return value;
  };
  const check = async deadline => {
    const failures = [];
    try { remaining(deadline); location(); } catch (error) { failures.push(error); }
    try { await assertHealthy(); } catch (error) { failures.push(error); }
    try { location(); remaining(deadline); } catch (error) { failures.push(error); }
    if (failures.length) { cleanup.guardErrors.push(...failures.slice(1)); throw failures[0]; }
  };
  const checked = async (operation, deadline) => {
    await check(deadline);
    let value, failure, failed = false;
    try { value = await operation(remaining(deadline)); } catch (error) { failure = error; failed = true; }
    try { await check(deadline); } catch (error) {
      if (!failed) throw error;
      cleanup.guardErrors.push(error);
    }
    if (failed) throw failure;
    return value;
  };
  const unique = async (locator, name, deadline) => {
    const count = await checked(() => locator.count(), deadline);
    if (count !== 1) throw Error('Require exactly one upload ' + name);
    return locator;
  };
  const snapshot = async deadline => {
    await unique(uploadPanel(page), 'panel', deadline);
    const grid = await unique(uploadPageGrid(page), 'grid', deadline);
    return checked(() => grid.evaluate(domSnapshot, { shadow: SHADOW, original: ORIGINAL, clone: CLONE }), deadline);
  };
  const pause = deadline => checked(() => sleep(Math.min(pollMs, remaining(deadline))), deadline);
  const settle = async (expected, deadline) => {
    for (;;) {
      const state = inspect(await snapshot(deadline), expected);
      if (!state.pending) return state.names;
      await pause(deadline);
    }
  };
  const run = async (operation, deadlineAt) => {
    if (fenced) throw Error('Upload controls fenced after a prior failure', { cause: primaryError });
    if (active) throw Error('Concurrent upload control operation refused');
    const deadline = deadlineAt ?? now() + timeoutMs;
    if (!Number.isFinite(deadline)) throw Error('Require a finite absolute upload deadline');
    active = true;
    cleanup = { downAttempted: false, releaseAttempted: false, releaseSucceeded: false, releaseError: null, guardErrors: [] };
    try { await check(deadline); const value = await operation(deadline); await check(deadline); return value; }
    catch (error) { fenced = true; primaryError = error; throw error; }
    finally { active = false; }
  };
  const rectangle = async (card, deadline) => {
    const box = await checked(() => card.boundingBox(), deadline);
    if (!box || ![box.x, box.y, box.width, box.height].every(Number.isFinite) || box.width < 12 || box.height < 12) throw Error('Upload page pointer geometry unavailable');
    const point = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
    const viewport = page.viewportSize();
    if (!viewport || point.x < 6 || point.y < 6 || point.x > viewport.width - 6 || point.y > viewport.height - 6) throw Error('Synthetic upload pointer endpoints must both be in the viewport');
    return point;
  };
  const available = () => { if (fenced) throw Error('Upload controls fenced after a prior failure', { cause: primaryError }); };
  return Object.freeze({
    panel() { available(); return uploadPanel(page); },
    grid() { available(); return uploadPageGrid(page); },
    card(name) { available(); filenames([name]); return uploadPageCard(page, name); },
    async assertReady({ deadlineAt } = {}) { return run(async deadline => { await unique(uploadPanel(page), 'panel', deadline); }, deadlineAt); },
    async readSettledOrder(expectedNames, { deadlineAt } = {}) { const expected = filenames(expectedNames); return run(deadline => settle(expected, deadline), deadlineAt); },
    async clickButton(label, { deadlineAt } = {}) {
      if (typeof label !== 'string' || !label.trim()) throw Error('Require an exact upload button label');
      return run(async deadline => {
        await unique(uploadPanel(page), 'panel', deadline);
        const button = await unique(uploadPanelButton(page, label), 'button', deadline);
        await checked(timeout => button.click({ timeout }), deadline);
      }, deadlineAt);
    },
    async reorderLastToFirst(expectedNames, { deadlineAt } = {}) {
      const expected = filenames(expectedNames);
      if (expected.length < 2) throw Error('Pointer reorder requires at least two upload pages');
      return run(async deadline => {
        let failure, failed = false;
        const target = [expected.at(-1), ...expected.slice(0, -1)];
        try {
          await settle(expected, deadline);
          const first = await unique(uploadPageCard(page, expected[0]), 'first page card', deadline);
          const last = await unique(uploadPageCard(page, expected.at(-1)), 'last page card', deadline);
          await checked(timeout => last.scrollIntoViewIfNeeded({ timeout }), deadline);
          const start = await rectangle(last, deadline), end = await rectangle(first, deadline);
          await checked(() => page.mouse.move(start.x, start.y), deadline);
          // Mouseenter enables the retained dndzone; allow its real Svelte update.
          await pause(deadline);
          // Mark BEFORE await: a rejected down can still have pressed the button.
          await checked(() => { cleanup.downAttempted = true; return page.mouse.down({ button: 'left' }); }, deadline);
          await checked(() => page.mouse.move(start.x + 8, start.y, { steps: 2 }), deadline);
          await checked(() => page.mouse.move(end.x, end.y, { steps: 12 }), deadline);
          // The installed action observes pointer position asynchronously. Do not
          // release before the genuine shadow has actually reached index zero.
          for (;;) {
            const state = inspect(await snapshot(deadline), expected);
            if (state.pending === 'dnd-settling' && state.cards.length === expected.length && state.cards[0].shadow && state.shadows.length === 1) {
              const names = state.cards.slice(1).map(card => card.names.length === 1 ? card.names[0] : null);
              if (!same(names, expected.slice(0, -1))) throw Error('Unexpected non-shadow upload page order during pointer drag');
              break;
            }
            await pause(deadline);
          }
        } catch (error) { failure = error; failed = true; }
        finally {
          if (cleanup.downAttempted) {
            cleanup.releaseAttempted = true;
            // Release is cleanup and remains mandatory even after the deadline,
            // route or health fence. Never gate it behind another await/check.
            try { await page.mouse.up({ button: 'left' }); cleanup.releaseSucceeded = true; }
            catch (error) { cleanup.releaseError = error; if (!failed) { failure = error; failed = true; } }
            try { await check(deadline); } catch (error) { cleanup.guardErrors.push(error); if (!failed) { failure = error; failed = true; } }
          }
        }
        if (failed) throw failure;
        return settle(target, deadline);
      }, deadlineAt);
    },
    cleanupSnapshot() {
      return Object.freeze({ ...cleanup, guardErrors: [...cleanup.guardErrors], fenced, primaryError });
    },
  });
}
