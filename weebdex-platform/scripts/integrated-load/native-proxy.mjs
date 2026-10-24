// Actual HTTP forwarding, never a route.fulfill/fetch substitute. Import is inert.
import http from 'node:http';
import { Transform, pipeline } from 'node:stream';

const HOP = new Set(['connection', 'proxy-connection', 'keep-alive', 'te', 'trailer', 'transfer-encoding', 'upgrade', 'proxy-authorization', 'proxy-authenticate']);
export class Refused extends Error { constructor(code) { super(code); this.code = code; } }
const integer = (value, low, high) => Number.isSafeInteger(value) && value >= low && value <= high;
export function exactOrigin(value) {
  if (typeof value !== 'string' || !/^http:\/\/127\.0\.0\.1:[1-9][0-9]{0,4}$/.test(value)) throw new Refused('origin');
  const url = new URL(value);
  if (url.origin !== value || !integer(Number(url.port), 1025, 65535)) throw new Refused('origin');
  return url;
}
export function targetFor(raw, rawHeaders, allowed) {
  if (typeof raw !== 'string' || /[\s\\\x00-\x1f\x7f]/.test(raw)) throw new Refused('target');
  const target = new URL(raw);
  if (target.href !== raw || target.username || target.password || target.hash || !allowed.has(target.origin)) throw new Refused('target');
  const headers = new Map();
  for (let i = 0; i < rawHeaders.length; i += 2) {
    const key = rawHeaders[i].toLowerCase(), value = rawHeaders[i + 1];
    if (headers.has(key) && ['host', 'content-length', 'transfer-encoding', 'connection'].includes(key)) throw new Refused('duplicate-framing');
    headers.set(key, value);
  }
  if (headers.get('host') !== target.host) throw new Refused('host');
  if (headers.has('transfer-encoding') || headers.has('expect')) throw new Refused('unsupported-framing');
  if (headers.has('content-length') && !/^(0|[1-9][0-9]*)$/.test(headers.get('content-length'))) throw new Refused('length');
  return target;
}
export function endToEnd(headers) {
  const connection = String(headers.connection ?? '').split(',').map(x => x.trim().toLowerCase());
  // Do not let a malicious Connection token strip an end-to-end policy/auth field.
  if (connection.some(name => name && !HOP.has(name) && name !== 'close')) throw new Refused('connection-token');
  return Object.fromEntries(Object.entries(headers).filter(([name]) => !HOP.has(name.toLowerCase())));
}
export const laneFor = url => {
  const path = new URL(url).pathname;
  if (path.startsWith('/data/') || path.startsWith('/data-saver/') || path.startsWith('/covers/')) return 'media';
  if (path.startsWith('/api/v1/analytics/')) return 'analytics';
  if (path.startsWith('/api/')) return 'api';
  return 'document-static';
};

export class Credits {
  constructor({ attempts = 2200, response = 448 * 1024 ** 2, upload = 1024 ** 2, inflight = 16, queue = 32, seconds = 180, now = () => performance.now() } = {}) {
    if (!integer(attempts, 1, 2500) || !integer(response, 1, 512 * 1024 ** 2) || !integer(upload, 0, 32 * 1024 ** 2) || !integer(inflight, 1, 32) || !integer(queue, 0, 64) || !integer(seconds, 1, 240)) throw new Refused('limits');
    this.limits = { attempts, response, upload, inflight, queue, seconds }; this.now = now; this.start = now(); this.deadline = this.start + seconds * 1000;
    this.records = []; this.active = 0; this.waiting = 0; this.peak = 0; this.upload = 0; this.response = 0; this.receivedUpload = 0; this.receivedResponse = 0; this.failure = null;
  }
  check() { if (this.failure) throw new Refused(this.failure); if (this.now() >= this.deadline) throw new Refused('deadline'); }
  offer(lane, method) {
    this.check();
    if (this.records.length >= this.limits.attempts) throw new Refused('attempt-limit');
    if (this.waiting >= this.limits.queue) throw new Refused('queue-limit');
    const row = { phase: this.phase ?? 'warmup', lane, method, status: null, state: 'queued', offered_ms: this.now() - this.start, forwarded_ms: null, headers_ms: null, first_body_ms: null, complete_ms: null, upload_bytes: 0, response_bytes: 0 };
    this.records.push(row); this.waiting++; return row;
  }
  dispatch(row) {
    this.check();
    if (row.state !== 'queued' || this.active >= this.limits.inflight) throw new Refused('inflight-limit');
    this.waiting--; this.active++; this.peak = Math.max(this.peak, this.active); row.state = 'active'; row.forwarded_ms = this.now() - this.start;
  }
  chunk(row, kind, bytes, withinLength = true) {
    if (!['upload', 'response'].includes(kind) || !integer(bytes, 0, 16 * 1024 ** 2) || row.state !== 'active') throw new Refused('chunk-state');
    this[kind === 'upload' ? 'receivedUpload' : 'receivedResponse'] += bytes;
    this.check();
    if (!withinLength) throw new Refused('length-overrun');
    if (this[kind] + bytes > this.limits[kind]) throw new Refused(kind + '-limit');
    this[kind] += bytes; row[kind + '_bytes'] += bytes;
    if (kind === 'response' && bytes && row.first_body_ms === null) row.first_body_ms = this.now() - this.start;
  }
  settle(row, state) {
    if (!['complete', 'failed', 'cancelled'].includes(state) || !['queued', 'active'].includes(row.state)) throw new Refused('duplicate-terminal');
    if (row.state === 'queued') this.waiting--; else this.active--;
    row.state = state; row.complete_ms = this.now() - this.start;
  }
  snapshot() { return { limits: this.limits, attempts: this.records.length, active: this.active, waiting: this.waiting, peak_inflight: this.peak, upstream_response_payload_received: this.receivedResponse, browser_response_payload_forwarded: this.response, browser_upload_payload_received: this.receivedUpload, upstream_upload_payload_forwarded: this.upload, records: this.records.map(row => ({ ...row })), failure: this.failure, wire_and_os_buffers_bounded: false }; }
}

export function meter(credits, row, kind, declared = null, maximum = Infinity) {
  if (declared !== null && !integer(declared, 0, Number.MAX_SAFE_INTEGER)) throw new Refused('length');
  let seen = 0;
  return new Transform({ transform(chunk, encoding, callback) {
    try {
      seen += chunk.length;
      credits.chunk(row, kind, chunk.length, seen <= maximum && (declared === null || seen <= declared)); callback(null, chunk);
    } catch (error) { callback(error); }
  }, flush(callback) { callback(declared !== null && seen !== declared ? new Refused('length-incomplete') : null); } });
}

// A response may finish before its request upload. Keep its slot until BOTH
// pipeline outcomes are terminal; later upload errors must never become success.
export class Exchange {
  constructor(credits, row) {
    if (row.state !== 'active') throw new Refused('exchange-state');
    this.credits = credits; this.row = row; this.requested = null;
    row.upload_state = row.response_state = 'pending';
  }
  stop(outcome) {
    if (!['failed', 'cancelled'].includes(outcome)) throw new Refused('exchange-stop');
    if (this.row.state === 'active' && this.requested !== 'failed') this.requested = outcome;
  }
  finish(kind, outcome) {
    if (!['upload', 'response'].includes(kind) || !['complete', 'failed'].includes(outcome) || this.row[kind + '_state'] !== 'pending') throw new Refused('duplicate-stream-terminal');
    this.row[kind + '_state'] = outcome;
    if (outcome === 'failed') this.credits.failure ??= kind + '-stream-failed';
    if (this.row.upload_state === 'pending' || this.row.response_state === 'pending') return false;
    const final = this.requested ?? ([this.row.upload_state, this.row.response_state].includes('failed') ? 'failed' : 'complete');
    this.credits.settle(this.row, final);
    return true;
  }
}

export function ownTerminal(terminals, start) {
  let finish;
  const pending = new Promise(resolve => { finish = resolve; });
  terminals.add(pending); pending.then(() => terminals.delete(pending));
  try { start(finish); } catch (error) { finish(); throw error; }
  return pending;
}

export async function settleTerminals(terminals, additional = [], milliseconds = 5000) {
  let timer;
  try {
    await Promise.race([Promise.all([...additional, ...terminals]), new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Refused('proxy-close-timeout')), milliseconds);
    })]);
    if (terminals.size) throw new Refused('proxy-cleanup-uncertain');
  } finally { clearTimeout(timer); }
}

export function createNativeProxy({ origins, credits, rate = 60, onAttempt = () => {} }) {
  const allowed = new Set(origins.map(value => exactOrigin(value).origin));
  if (!integer(rate, 1, 60)) throw new Refused('rate');
  const sockets = new Set(), upstream = new Set(), pending = [], terminals = new Set(), exchanges = new Map();
  let timer, last = -Infinity, stopped = false, denials = 0;
  const fail = error => {
    credits.failure ??= error instanceof Refused ? error.code : 'stream-error'; stopped = true;
    clearTimeout(timer);
    for (const row of credits.records) {
      if (row.state === 'active' && exchanges.has(row)) exchanges.get(row).stop('failed');
      else if (['active', 'queued'].includes(row.state)) credits.settle(row, 'failed');
    }
    for (const request of upstream) request.destroy(); for (const socket of sockets) socket.destroy();
    pending.length = 0;
  };
  function pump() {
    clearTimeout(timer);
    if (stopped || !pending.length || credits.active >= credits.limits.inflight) return;
    try { credits.check(); } catch (error) { fail(error); return; }
    const wait = Math.max(0, last + 1000 / rate - performance.now());
    if (wait) { timer = setTimeout(pump, wait); return; }
    const job = pending.shift(); last = performance.now();
    try { credits.dispatch(job.row); forward(job); } catch (error) { fail(error); }
    pump();
  }
  function forward({ request, response, target, row }) {
    // The exact encoded path/query, method, body, Host and end-to-end headers
    // reach the real app. No retry, redirect follow or response/body buffering.
    const headers = endToEnd(request.headers);
    const sent = http.request({ hostname: '127.0.0.1', port: Number(target.port), method: request.method, path: target.pathname + target.search, headers, agent: false });
    const exchange = new Exchange(credits, row); exchanges.set(row, exchange);
    let responseSeen = false;
    const finish = (kind, error) => {
      try { if (exchange.finish(kind, error ? 'failed' : 'complete')) exchanges.delete(row); }
      catch (failure) { fail(failure); }
      if (error) fail(error);
      pump();
    };
    upstream.add(sent); sent.once('close', () => {
      upstream.delete(sent);
      if (!responseSeen) finish('response', new Refused('upstream-closed-without-response'));
    });
    sent.setTimeout(Math.min(10000, Math.max(1, credits.deadline - credits.now())), () => fail(new Refused('upstream-timeout')));
    sent.once('error', fail);
    const declared = request.headers['content-length'] === undefined ? 0 : Number(request.headers['content-length']);
    ownTerminal(terminals, done => pipeline(request, meter(credits, row, 'upload', declared, 65536), sent, error => {
      finish('upload', error); done();
    }));
    sent.once('response', actual => {
      responseSeen = true;
      try {
        row.status = actual.statusCode; row.headers_ms = credits.now() - credits.start;
        if (actual.statusCode >= 400) throw new Refused('unexpected-http-status');
        const fields = endToEnd(actual.headers), rawLength = actual.headers['content-length'];
        if (rawLength !== undefined && !/^(0|[1-9][0-9]*)$/.test(rawLength)) throw new Refused('response-length');
        const absent = request.method === 'HEAD' || [204, 304].includes(actual.statusCode);
        const length = absent ? 0 : rawLength === undefined ? null : Number(rawLength);
        if (length !== null && length > credits.limits.response - credits.response) throw new Refused('response-limit');
        response.writeHead(actual.statusCode, fields);
        ownTerminal(terminals, resolve => {
          pipeline(actual, meter(credits, row, 'response', length), response, error => {
            finish('response', error); resolve();
          });
        });
      } catch (error) {
        // No response pipeline was admitted. Own the rejected stream's close
        // before releasing its active credit, including header/status refusal.
        ownTerminal(terminals, done => {
          const closed = () => { finish('response', error); done(); };
          if (actual.closed) closed(); else actual.once('close', closed);
          actual.destroy();
        });
        fail(error);
      }
    });
    request.resume();
  }
  const server = http.createServer({ maxHeaderSize: 16384, requestTimeout: 15000, headersTimeout: 15000 }, (request, response) => {
    request.pause();
    try {
      const target = targetFor(request.url, request.rawHeaders, allowed);
      if (!['GET', 'HEAD', 'POST'].includes(request.method) || request.method === 'POST' && !/^\/api\/(?:v1\/nostr\/native-forum\/contexts|v1\/analytics\/(?:manga|chapter)\/[^/]+\/views|protocol\/resources)$/.test(target.pathname)) throw new Refused('method-scope');
      if (Number(request.headers['content-length'] ?? 0) > 65536) throw new Refused('request-length');
      const row = credits.offer(laneFor(request.url), request.method);
      onAttempt(request.method + ' ' + request.url);
      response.once('close', () => { if (!response.writableFinished && !stopped) fail(new Refused('client-disconnect')); });
      pending.push({ request, response, target, row }); pump();
    } catch (error) { denials++; fail(error); }
  });
  server.on('connection', socket => { sockets.add(socket); socket.once('close', () => sockets.delete(socket)); socket.on('error', () => {}); if (sockets.size > 64) fail(new Refused('socket-limit')); });
  server.on('connect', (_, socket) => { denials++; socket.destroy(); fail(new Refused('connect-denied')); });
  server.on('upgrade', (_, socket) => { denials++; socket.destroy(); fail(new Refused('upgrade-denied')); });
  server.on('clientError', (_, socket) => { socket.destroy(); fail(new Refused('client-framing')); });
  server.on('error', fail);
  const deadlineTimer = setTimeout(() => fail(new Refused('deadline')), Math.max(1, credits.deadline - credits.now()));
  deadlineTimer.unref();
  return { server, setPhase(name, requestsPerSecond) {
    if (!['warmup', 'smoke', 'baseline', 'mixed', 'pressure', 'drain'].includes(name) || !integer(requestsPerSecond, 1, 60) || credits.active || credits.waiting) throw new Refused('phase-transition');
    credits.phase = name; rate = requestsPerSecond;
  }, fail, async close() {
    stopped = true; clearTimeout(timer); clearTimeout(deadlineTimer);
    const unfinished = credits.active + credits.waiting;
    if (unfinished) {
      credits.failure ??= 'unfinished-streams';
      for (const row of credits.records) {
        if (row.state === 'active' && exchanges.has(row)) exchanges.get(row).stop('cancelled');
        else if (['active', 'queued'].includes(row.state)) credits.settle(row, 'cancelled');
      }
    }
    const upstreamEnds = [...upstream].map(request => new Promise(resolve => { request.once('close', resolve); request.destroy(); }));
    const socketEnds = [...sockets].map(socket => new Promise(resolve => { socket.once('close', resolve); socket.destroy(); }));
    const listener = new Promise((resolve, reject) => server.close(error => error && error.code !== 'ERR_SERVER_NOT_RUNNING' ? reject(error) : resolve()));
    await settleTerminals(terminals, [listener, ...socketEnds, ...upstreamEnds]);
    if (sockets.size || upstream.size || terminals.size || exchanges.size || credits.active || credits.waiting || server.listening) throw new Refused('proxy-cleanup-uncertain');
    return { listener_closed: true, sockets: 0, upstream: 0, terminals: 0, unforced: unfinished === 0 && !credits.failure, denied_requests: denials };
  } };
}
