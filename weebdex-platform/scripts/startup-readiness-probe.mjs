// Diagnostic only. A late render never changes the original 1200 ms verdict.
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
