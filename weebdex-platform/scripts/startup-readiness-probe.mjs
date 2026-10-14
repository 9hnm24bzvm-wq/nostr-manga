// Finite startup readiness and read-only observations. The diagnostic helper
// still never changes a snapshot which its caller has already recorded.
export function readSeedTitleState() {
  const heading = [...document.querySelectorAll('h1')].some(node =>
    node.textContent?.trim() === 'Fixture Adventure' && node.getClientRects().length > 0);
  return {
    document_ready: ['interactive', 'complete'].includes(document.readyState),
    body_meaningful: document.body.innerText.trim().length > 20,
    fixture_heading_visible: heading,
    fixture_route: /^\/title\/SeedManga1(?:\/[^/]+)?\/?$/.test(location.pathname),
  };
}

export function seedTitleReady() {
  return ['interactive', 'complete'].includes(document.readyState) &&
    /^\/title\/SeedManga1(?:\/[^/]+)?\/?$/.test(location.pathname) &&
    document.body.innerText.trim().length > 20 &&
    [...document.querySelectorAll('h1')].some(node =>
      node.textContent?.trim() === 'Fixture Adventure' && node.getClientRects().length > 0);
}

export async function waitForSeedTitleReady(page, { timeoutMs = 7000 } = {}) {
  const deadline = Date.now() + Math.max(1, Math.min(7000, Number.isFinite(timeoutMs) ? timeoutMs : 7000));
  const timeout = () => { const error = Error('Expected seeded title did not become ready'); error.name = 'TimeoutError'; return error; };
  const bounded = async action => {
    const remaining = deadline - Date.now();
    if (remaining <= 0) throw timeout();
    let timer;
    try {
      // Observe late rejection without extending the original finite deadline.
      return await Promise.race([Promise.resolve().then(action), new Promise((_, reject) => {
        timer = setTimeout(() => reject(timeout()), remaining);
      })]);
    } finally { clearTimeout(timer); }
  };
  await bounded(() => page.waitForFunction(seedTitleReady, undefined,
    { timeout: Math.max(1, deadline - Date.now() - 250), polling: 100 }));
  const state = structuredClone(await bounded(() => page.evaluate(readSeedTitleState)));
  if (!state || !['document_ready', 'body_meaningful', 'fixture_heading_visible', 'fixture_route'].every(key => state[key] === true)) {
    throw Error('Expected seeded title readiness changed before its snapshot');
  }
  return state;
}

export async function observeSeedTitleReadiness(page, originalBodyMeaningful, { timeoutMs = 7000 } = {}) {
  const deadline = Date.now() + Math.max(1, Math.min(7000, Number.isFinite(timeoutMs) ? timeoutMs : 7000));
  const deadlineError = () => { const error = Error('Diagnostic deadline'); error.name = 'TimeoutError'; return error; };
  const bounded = async action => {
    const remaining = deadline - Date.now();
    if (remaining <= 0) throw deadlineError();
    let timer;
    try {
      // Promise.race installs handlers on the original operation immediately;
      // a late renderer rejection after our deadline cannot become unhandled.
      return await Promise.race([Promise.resolve().then(action), new Promise((_, reject) => {
        timer = setTimeout(() => reject(deadlineError()), remaining);
      })]);
    } finally { clearTimeout(timer); }
  };
  const value = { original_body_meaningful: originalBodyMeaningful, initial: undefined,
    late: undefined, wait_outcome: 'not-needed' };
  try {
    value.initial = structuredClone(await bounded(() => page.evaluate(readSeedTitleState)));
    if (!originalBodyMeaningful) {
      try {
        await bounded(() => page.waitForFunction(() => /^\/title\/SeedManga1(?:\/[^/]+)?\/?$/.test(location.pathname) &&
          document.body.innerText.trim().length > 20 &&
          [...document.querySelectorAll('h1')].some(node => node.textContent?.trim() === 'Fixture Adventure' && node.getClientRects().length > 0),
        // Keep a small final-sample window inside, never beyond, the same 7s.
        undefined, { timeout: Math.max(1, deadline - Date.now() - 250), polling: 100 }));
        value.wait_outcome = 'ready';
      } catch (error) { value.wait_outcome = error?.name === 'TimeoutError' ? 'deadline' : 'probe-error'; }
    }
    value.late = structuredClone(await bounded(() => page.evaluate(readSeedTitleState)));
  } catch (error) { value.wait_outcome = error?.name === 'TimeoutError' ? 'deadline' : 'probe-error'; }
  return structuredClone(value);
}
