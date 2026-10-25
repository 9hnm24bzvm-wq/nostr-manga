// Passive, exact-request observation only. No request is created, replayed or
// changed. All response bytes remain private; only fixed journey check statuses
// may leave the runner. Bodies are read only after requestfinished, never while
// waiting for a network stream. close() joins every owned decode task.
export function watchSignedConversationResponses(page, { nativeOrigin, protocolOrigin, kind, id, eventId }) {
  const records = { mapping: [], root: [], feed: [] }, requests = new Map(), tasks = [];
  let started = false, closed = false;
  const match = request => {
    const url = new URL(request.url());
    if (request.method() === 'POST' && url.href === nativeOrigin + '/api/protocol/resources') {
      let body; try { body = request.postDataJSON(); } catch { return; }
      if (body && Object.keys(body).length === 1 && Array.isArray(body.resources) && body.resources.length === 1 &&
          Object.keys(body.resources[0] ?? {}).sort().join('|') === 'id|kind' && body.resources[0].kind === kind && body.resources[0].id === id) return 'mapping';
    }
    if (request.method() !== 'GET') return;
    if (url.href === protocolOrigin + '/events/' + eventId) return 'root';
    if (url.href === protocolOrigin + '/social?target=' + eventId + '&limit=50&offset=0') return 'feed';
  };
  const response = value => {
    if (!started || closed) return;
    const request = value.request();
    let type; try { type = match(request); } catch { return; }
    if (!type) return;
    const record = { response: value, finished: false, decoded: false, limit: type === 'root' ? 16384 : 1024 * 1024 };
    records[type].push(record); requests.set(request, record);
  };
  const finished = request => {
    const record = requests.get(request);
    if (!record || record.finished || closed) return;
    record.finished = true;
    // A finished response is buffered by Playwright. This operation has no
    // additional network wait/deadline and every failure is owned immediately.
    const task = Promise.resolve().then(async () => {
      const bytes = await record.response.body();
      if (bytes.length > record.limit || !/json/i.test(record.response.headers()['content-type'] ?? '')) return;
      record.status = record.response.status();
      record.body = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
      record.decoded = true;
    }).catch(() => {});
    tasks.push(task);
  };
  page.on('response', response); page.on('requestfinished', finished);
  return {
    start() { if (started || closed) throw Error('Conversation observer already used'); started = true; },
    one(type) {
      if (!Object.hasOwn(records, type) || records[type].length !== 1 || !records[type][0].finished || !records[type][0].decoded) throw Error('Exact conversation response was absent, duplicated, unfinished, oversized or invalid JSON');
      return records[type][0];
    },
    async close() { if (!closed) { closed = true; page.off('response', response); page.off('requestfinished', finished); } await Promise.allSettled(tasks); },
  };
}
