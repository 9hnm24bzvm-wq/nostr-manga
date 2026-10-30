// Independent implementation acceptance only. Importing this file does not
// generate keys, execute nak, bind sockets or contact any service.
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawn } from 'node:child_process';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
export const NAK_SHA256 = 'e2fbbdf24cda6fae99c777f11003e0b517b43ac1a40224c78fa87a051354e394';
export const MAX_SECONDS = 1200;
const keyPattern = /^[0-9a-f]{64}$/;
const digest = value => createHash('sha256').update(value).digest('hex');
const hex = value => Buffer.from(value).toString('hex');
export function localOrigin(value, protocol = 'http:') {
  const url = new URL(value);
  if (url.protocol !== protocol || url.hostname !== '127.0.0.1' || !url.port || Number(url.port) < 1024 || url.username || url.password || url.pathname !== '/' || url.search || url.hash || value !== url.origin) throw Error('An exact unprivileged IPv4 loopback origin is required.');
  return url.origin;
}
export function plannedRelayPort(value) {
  if (!Number.isInteger(value) || value < 1024 || value > 65535) throw Error('One exact unprivileged relay port is required.');
  return value;
}
export function narrowFilter(value, recipient, author) {
  if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).some(k => !['kinds', '#p', 'authors', 'since', 'until', 'limit'].includes(k))) return false;
  if (JSON.stringify(value.kinds) !== '[24133]' || JSON.stringify(value['#p']) !== JSON.stringify([recipient])) return false;
  if (value.authors !== undefined && JSON.stringify(value.authors) !== JSON.stringify([author])) return false;
  return ['since', 'until', 'limit'].every(k => value[k] === undefined || Number.isSafeInteger(value[k]) && value[k] >= 0 && (k !== 'limit' || value[k] <= 64));
}
export async function createIndependentNakFixture({ approved, domDependencies, relayPort }) {
  if (approved !== true) throw Error('Explicit disposable independent-signer approval is required.');
  const listenPort = relayPort === undefined ? 0 : plannedRelayPort(relayPort);
  const binary = path.join(root, '.build/independent-signer-nak-0.21.2/nak-nodb-v0.21.2-linux-amd64');
  if (fs.statSync(binary).size !== 45612305 || digest(fs.readFileSync(binary)) !== NAK_SHA256) throw Error('Independent signer binary does not match its reviewed acquisition.');
  const require = createRequire(import.meta.url);
  const { WebSocketServer } = require(path.join(path.resolve(domDependencies), 'ws'));
  const dependencies = path.join(root, '.build/remote-signer-nostr-tools-2.25.2/node_modules');
  const { schnorr } = await import(pathToFileURL(path.join(dependencies, '@noble/curves/secp256k1.js')));
  const runtime = fs.mkdtempSync(path.join(os.tmpdir(), 'weebdex-nak-owned-'));
  fs.chmodSync(runtime, 0o700);
  for (const directory of ['home', 'config', 'tmp']) fs.mkdirSync(path.join(runtime, directory), { mode: 0o700 });
  const logPath = path.join(runtime, 'private-nak.log'), linkPath = path.join(runtime, 'Independent signer connection.txt');
  const log = fs.openSync(logPath, 'wx', 0o600);
  let secret, publicKey = '', server, ws, child, childExit, ttl, origin = '', browserOrigin = '', clientKey = '', stopped = false, closing;
  const expiresAt = Date.now() + MAX_SECONDS * 1000;
  let phase = 'starting-owned-relay', identitiesCreated = 0;
  let logBytes = 0, tail = '', events = 0, forwarded = 0, connections = 0, rejected = 0, signatureFailures = 0, uriReady = false;
  const sockets = new Set(), peers = new Map(), proofs = new Map();
  const snapshot = () => ({ fixtureOnly: true, implementation: 'official-nak-v0.21.2', binarySha256: NAK_SHA256, publicKey, identitiesCreated, relayOrigin: origin, browserOrigin: browserOrigin || null, maximumLifetimeSeconds: MAX_SECONDS, expiresAt: new Date(expiresAt).toISOString(), phase, activeConnections: peers.size, connections, relayEvents: events, forwarded, rejected, signatureFailures, proofEvents: [...proofs.values()], stopped });
  function validEvent(event) {
    try {
      return event && !Array.isArray(event) && keyPattern.test(event.pubkey) && keyPattern.test(event.id) && /^[0-9a-f]{128}$/.test(event.sig) && Number.isSafeInteger(event.created_at) && Array.isArray(event.tags) && typeof event.content === 'string' && event.id === digest(JSON.stringify([0, event.pubkey, event.created_at, event.kind, event.tags, event.content])) && schnorr.verify(Buffer.from(event.sig, 'hex'), Buffer.from(event.id, 'hex'), Buffer.from(event.pubkey, 'hex'));
    } catch { return false; }
  }
  function collectPrivateProofReceipt() {
    // Read only our private bounded nak output, never return its text. The
    // independent signer logs JSON responses; retain only verified proof IDs.
    const text = fs.readFileSync(logPath, 'utf8').replace(/\x1b\[[0-9;]*m/g, '');
    for (const match of text.matchAll(/~ responding with (\{[\s\S]*?\n\})/g)) {
      try {
        const response = JSON.parse(match[1]); if (typeof response.result !== 'string' || !response.result.startsWith('{')) continue;
        const event = JSON.parse(response.result); if (!validEvent(event) || event.pubkey !== publicKey) continue;
        const tags = event.tags;
        if (event.kind === 27235 && event.content === '' && tags.length === 3 && JSON.stringify(tags[0]) === JSON.stringify(['u', browserOrigin + '/api/v1/account/identity-link/confirm']) && JSON.stringify(tags[1]) === '["method","POST"]' && tags[2]?.[0] === 'payload' && tags[2]?.length === 2 && keyPattern.test(tags[2][1])) proofs.set(event.id, { eventId: event.id, kind: 27235, signatureVerified: true, nativeIdentityProofEndpoint: true });
      } catch { /* malformed/private log text never enters evidence */ }
    }
  }
  async function close() {
    if (closing) return closing;
    stopped = true; clearTimeout(ttl);
    closing = (async () => {
      if (child && child.exitCode === null && child.signalCode === null) {
        try { process.kill(-child.pid, 'SIGTERM'); } catch { /* own group may already be gone */ }
        const force = setTimeout(() => { try { process.kill(-child.pid, 'SIGKILL'); } catch { /* already stopped */ } }, 1500);
        await childExit; clearTimeout(force);
      }
      for (const peer of peers.keys()) peer.terminate();
      for (const socket of sockets) socket.destroy();
      if (ws) await new Promise(resolve => ws.close(resolve));
      if (server?.listening) await new Promise(resolve => server.close(resolve));
      collectPrivateProofReceipt(); fs.closeSync(log); secret?.fill(0); tail = '';
      fs.rmSync(runtime, { recursive: true, force: true });
      return { ...snapshot(), childStopped: !child || child.exitCode !== null || child.signalCode !== null, listenerStopped: !server?.listening, privateRuntimeRemoved: !fs.existsSync(runtime) };
    })();
    return closing;
  }
  try {
    server = createServer((req, res) => {
      if (req.headers.host !== new URL(origin).host || req.headers.origin && req.headers.origin !== browserOrigin || req.method !== 'GET' || req.url !== '/health') { res.writeHead(403).end(); return; }
      res.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store', 'content-security-policy': "default-src 'none'; frame-ancestors 'none'" });
      res.end(JSON.stringify(snapshot()));
    });
    server.on('connection', socket => { sockets.add(socket); socket.once('close', () => sockets.delete(socket)); });
    server.on('connect', (_request, socket) => socket.destroy());
    ws = new WebSocketServer({ noServer: true, maxPayload: 65536 });
    server.on('upgrade', (req, socket, head) => {
      const role = req.headers.origin === browserOrigin && browserOrigin ? 'browser' : !req.headers.origin && req.headers['user-agent'] === 'nak/b' ? 'nak' : null;
      if (stopped || req.url !== '/relay' || req.headers.host !== new URL(origin).host || !role || [...peers.values()].some(peer => peer.role === role) || ++connections > 6) { rejected++; socket.destroy(); return; }
      ws.handleUpgrade(req, socket, head, value => ws.emit('connection', value, role));
    });
    ws.on('connection', (socket, role) => {
      const peer = { role, subscription: '', filter: null, frames: 0 }; peers.set(socket, peer);
      socket.once('close', () => peers.delete(socket));
      socket.on('message', (raw, binaryMessage) => {
        try {
          if (binaryMessage || ++peer.frames > 100 || raw.length > 65536) throw Error();
          const frame = JSON.parse(raw.toString()); if (!Array.isArray(frame)) throw Error();
          if (frame[0] === 'REQ') {
            if (frame.length !== 3 || typeof frame[1] !== 'string' || !frame[1] || frame[1].length > 128) throw Error();
            const recipient = role === 'nak' ? publicKey : frame[2]?.['#p']?.[0];
            if (!keyPattern.test(recipient) || !narrowFilter(frame[2], recipient, role === 'browser' ? publicKey : clientKey)) throw Error();
            if (role === 'browser') { if (clientKey && clientKey !== recipient) throw Error(); clientKey = recipient; }
            peer.subscription = frame[1]; peer.filter = frame[2]; socket.send(JSON.stringify(['EOSE', frame[1]])); return;
          }
          if (frame[0] === 'CLOSE' && frame.length === 2 && frame[1] === peer.subscription) { peer.subscription = ''; peer.filter = null; return; }
          if (frame[0] !== 'EVENT' || frame.length !== 2 || !peer.subscription || ++events > 64) throw Error();
          const event = frame[1], sender = role === 'nak' ? publicKey : clientKey, target = role === 'nak' ? clientKey : publicKey;
          if (!validEvent(event)) { signatureFailures++; throw Error(); }
          if (event.kind !== 24133 || event.pubkey !== sender || JSON.stringify(event.tags) !== JSON.stringify([['p', target]]) || Math.abs(event.created_at - Math.floor(Date.now() / 1000)) > 90 || event.content.length > 50000) throw Error();
          socket.send(JSON.stringify(['OK', event.id, true, '']));
          for (const [other, filter] of peers) if (other !== socket && filter.subscription && filter.filter?.['#p']?.[0] === target && (!filter.filter.authors || filter.filter.authors.includes(sender)) && other.readyState === 1) { other.send(JSON.stringify(['EVENT', filter.subscription, event])); forwarded++; }
        } catch { rejected++; socket.close(1008, 'Owned relay boundary refused'); }
      });
    });
    // Bind the planned port before generating the single approved identity. A
    // collision fails without creating a key; never choose a fallback port.
    await new Promise((resolve, reject) => { server.once('error', reject); server.listen(listenPort, '127.0.0.1', resolve); });
    origin = `ws://127.0.0.1:${server.address().port}`;
    phase = 'generating-one-disposable-identity';
    // The product independently creates its normal ephemeral NIP46 transport
    // key only after explicit UI consent; it is not a user signing identity.
    secret = schnorr.utils.randomSecretKey(); identitiesCreated++; publicKey = hex(schnorr.getPublicKey(secret));
    const relay = origin + '/relay';
    // No inherited credentials, proxies, config or external endpoints. timeout
    // bounds the actual child even if this supervising Node process exits.
    const env = { PATH: '/usr/bin:/bin', HOME: path.join(runtime, 'home'), XDG_CONFIG_HOME: path.join(runtime, 'config'), TMPDIR: path.join(runtime, 'tmp'), NO_COLOR: '1', LANG: 'C', NOSTR_SECRET_KEY: hex(secret) };
    phase = 'starting-official-nak';
    child = spawn('/usr/bin/timeout', ['--signal=TERM', '--kill-after=2s', `${MAX_SECONDS}s`, binary, '--config-path', path.join(runtime, 'config'), '--connect-timeout', '5s', 'bunker', relay], { cwd: runtime, env, detached: true, stdio: ['ignore', 'pipe', 'pipe'] });
    env.NOSTR_SECRET_KEY = ''; secret.fill(0);
    childExit = new Promise(resolve => { child.once('exit', resolve); child.once('error', resolve); });
    const privateOutput = chunk => {
      if (stopped) return;
      logBytes += chunk.length; if (logBytes > 1024 * 1024) { void close(); return; }
      fs.writeSync(log, chunk); tail = (tail + chunk.toString()).slice(-16384).replace(/\x1b\[[0-9;]*m/g, '');
      for (const found of tail.matchAll(/bunker:\/\/[0-9a-f]{64}\?[^\s]+(?=\s)/g)) {
        try {
          const link = new URL(found[0]);
          if (link.hostname !== publicKey || link.searchParams.getAll('relay').length !== 1 || link.searchParams.get('relay') !== relay || link.searchParams.getAll('secret').length !== 1 || !/^[A-Za-z0-9_-]{1,128}$/.test(link.searchParams.get('secret')) || [...link.searchParams.keys()].some(key => !['relay', 'secret'].includes(key))) continue;
          // Private user-delivery file only. Never expose this through HTTP,
          // console, receipts or automatic browser entry.
          fs.writeFileSync(linkPath, found[0] + '\n', { mode: 0o600 }); uriReady = true;
        } catch { /* only a complete exact advertised URI is retained */ }
      }
    };
    child.stdout.on('data', privateOutput); child.stderr.on('data', privateOutput);
    ttl = setTimeout(() => { void close(); }, Math.max(0, expiresAt - Date.now()));
    phase = 'waiting-for-advertised-link-and-nak-subscription';
    const deadline = Date.now() + 10000;
    while (!uriReady || ![...peers.values()].some(peer => peer.role === 'nak' && peer.subscription)) {
      if (stopped || child.exitCode !== null || child.signalCode !== null || Date.now() >= deadline) throw Error('Independent signer did not become ready.');
      await new Promise(resolve => setTimeout(resolve, 25));
    }
    phase = 'ready-awaiting-exact-browser-origin';
    return { relayOrigin: origin, privateConnectionFile: linkPath, expiresAt, snapshot, close, setBrowserOrigin(value) { if (browserOrigin || stopped) throw Error('Browser origin is already bound or fixture stopped.'); browserOrigin = localOrigin(value); phase = 'ready-for-explicit-user-connection'; } };
  } catch { const receipt = await close(); const error = Error('Independent signer preparation failed; private runtime cleaned.'); error.receipt = receipt; throw error; }
}
