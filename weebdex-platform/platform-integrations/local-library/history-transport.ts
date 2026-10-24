import { HistoryBudget, HISTORY_LIMITS, HistoryTransportUnsettled } from './history-pagination';
// One owned history transport slot also covers cleanup after a caller stops
// waiting. A broken dependency cannot accumulate detached reads by reopening a
// view. Account-mode transports do not use this module.
let active: object | undefined;
export async function historyJson(url: string, budget: HistoryBudget, native = false): Promise<{ status: number; body: unknown }> {
  budget.check();
  if (active) throw new HistoryTransportUnsettled();
  budget.request();
  const owner = active = {}, request = new AbortController();
  let reader: ReadableStreamDefaultReader<Uint8Array> | undefined, response: Response | undefined;
  let fetchWork: Promise<Response> | undefined, readWork: Promise<ReadableStreamReadResult<Uint8Array>> | undefined;
  let eof = false, failure: unknown;
  const cancel = () => request.abort(budget.signal.reason);
  let rejectAbort: (reason: unknown) => void = () => {};
  const aborted = new Promise<never>((_resolve, reject) => { rejectAbort = reject; });
  const onAbort = () => rejectAbort(request.signal.reason);
  request.signal.addEventListener('abort', onAbort, { once: true });
  // Observe cancellation even when it happens between two guarded operations.
  void aborted.catch(() => {});
  budget.signal.addEventListener('abort', cancel, { once: true });
  if (budget.signal.aborted) cancel();
  const check = () => { budget.check(); if (request.signal.aborted) throw request.signal.reason; };
  const release = () => { reader?.releaseLock(); if (active === owner) active = undefined; };
  try {
    check();
    fetchWork = Promise.resolve().then(() => {
      check();
      return fetch(url, { method: 'GET', credentials: native ? 'omit' : 'same-origin', cache: 'no-store', redirect: 'error', signal: request.signal, headers: { Accept: 'application/json' } });
    }).then(value => { response = value; return value; });
    response = await Promise.race([fetchWork, aborted]);
    check();
    reader = response.body?.getReader();
    if (!reader) throw Error('History lookup returned no response body');
    if (response.headers.get('Content-Type')?.split(';', 1)[0].trim().toLowerCase() !== 'application/json') throw Error('History lookup returned non-JSON data');
    // Fixed allocation: neither tiny nor zero-length fragments allocate a
    // retained entry. Fragment counts independently bound pathological reads.
    const data = new Uint8Array(HISTORY_LIMITS.responseBytes);
    let size = 0, fragments = 0, empty = 0;
    for (;;) {
      check();
      if (++fragments > HISTORY_LIMITS.fragments) throw Error('History response exceeds its fragment budget. This batch was not skipped.');
      readWork = Promise.resolve().then(() => { check(); return reader!.read(); }).then(part => { if (part.done) eof = true; return part; });
      const next = await Promise.race([readWork, aborted]);
      check();
      if (next.done) break;
      if (!ArrayBuffer.isView(next.value) || Object.prototype.toString.call(next.value) !== '[object Uint8Array]') throw Error('History response returned invalid byte data');
      budget.consume(next.value.byteLength);
      if (next.value.byteLength > data.length - size) throw Error('History response exceeds its byte budget. This batch was not skipped.');
      if (!next.value.byteLength && ++empty > HISTORY_LIMITS.emptyFragments) throw Error('History response exceeds its empty-fragment budget. This batch was not skipped.');
      data.set(next.value, size); size += next.value.byteLength;
    }
    check();
    return { status: response.status, body: JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(data.subarray(0, size))) };
  } catch (error) { failure = error; throw error; }
  finally {
    budget.signal.removeEventListener('abort', cancel);
    request.signal.removeEventListener('abort', onAbort);
    if (eof) {
      // EOF is a healthy transport terminal even if JSON/scope validation fails
      // afterwards. Later action aborts must not retroactively cancel this read.
      try { release(); } catch { throw new HistoryTransportUnsettled(); }
    } else {
      request.abort(failure);
      const cleanup = (async () => {
        // A fetch that ignores abort still owns its eventual response. Keep the
        // slot until that body and any pending read/cancellation really settle.
        if (fetchWork) await fetchWork.catch(() => undefined);
        if (!reader && response?.body) reader = response.body.getReader();
        if (reader) await Promise.allSettled([Promise.resolve().then(() => reader!.cancel()), ...(readWork ? [readWork] : [])]);
        release();
      })();
      // This observer survives the finite grace window, including rejection.
      const settled = cleanup.then(() => true, () => false);
      let timer: ReturnType<typeof setTimeout> | undefined;
      try {
        if (!await Promise.race([settled, new Promise<boolean>(resolve => { timer = setTimeout(() => resolve(false), HISTORY_LIMITS.cleanupMs); })])) throw new HistoryTransportUnsettled();
      } finally { if (timer !== undefined) clearTimeout(timer); }
    }
  }
}
