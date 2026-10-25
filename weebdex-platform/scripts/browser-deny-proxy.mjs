import http from 'node:http';

// This proxy never opens an outbound connection. CONNECT transfers socket-error
// ownership away from Node's HTTP parser, so own only that denied tunnel's
// observed peer-reset case. All other socket/server errors retain fatal behavior.
export function createBrowserDenyProxy(report, safeUrl, save = () => {}) {
  const sockets = new Set();
  const value = report.proxy_lifecycle = { connect_denials: 0, expected_connect_resets: 0,
    observations_truncated: false, shutdown_started: false, closed: false, remaining_socket_count: 0 };
  const persist = () => { try { save(); } catch { /* Original mandatory report saves remain authoritative. */ } };
  const increment = key => {
    if (value[key] < 65535) value[key]++;
    else value.observations_truncated = true;
  };
  const proxy = http.createServer((request, response) => {
    report.proxy_requests_blocked.push({ method: request.method, url: safeUrl(request.url ?? '') });
    response.writeHead(403, { 'Content-Type': 'text/plain' });
    response.end('External browser traffic blocked by acceptance fixture');
  });
  proxy.on('connection', socket => {
    sockets.add(socket);
    value.remaining_socket_count = Math.min(65535, sockets.size);
    if (sockets.size > 65535) value.observations_truncated = true;
    socket.once('close', () => { sockets.delete(socket); value.remaining_socket_count = Math.min(65535, sockets.size); });
  });
  proxy.on('connect', (request, socket) => {
    report.proxy_requests_blocked.push({ method: 'CONNECT', destination: String(request.url).split('?')[0] });
    increment('connect_denials');
    socket.on('error', error => {
      if (error?.code !== 'ECONNRESET' || !sockets.has(socket)) throw error;
      increment('expected_connect_resets'); persist();
    });
    socket.end('HTTP/1.1 403 Forbidden\r\n\r\n');
  });
  async function close({ timeoutMs = 7000 } = {}) {
    value.shutdown_started = true; persist();
    const closed = [...sockets].map(socket => new Promise(resolve => {
      socket.once('close', resolve); socket.destroy();
    }));
    closed.push(new Promise((resolve, reject) => proxy.close(error => error ? reject(error) : resolve())));
    let timer;
    try {
      await Promise.race([Promise.all(closed), new Promise((_, reject) => {
        timer = setTimeout(() => reject(Error('Owned deny proxy shutdown timeout')), Math.max(1, Math.min(7000, Number.isFinite(timeoutMs) ? timeoutMs : 7000)));
      })]);
    } finally {
      clearTimeout(timer); value.remaining_socket_count = Math.min(65535, sockets.size); persist();
    }
    if (sockets.size || proxy.listening) throw Error('Owned deny proxy shutdown is unresolved');
    value.closed = true; persist();
  }
  return { proxy, sockets, close };
}
