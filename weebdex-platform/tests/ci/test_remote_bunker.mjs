// Real owned loopback WebSocket relay + synthetic NIP-44 bunker. No external
// relay, native account, real credential or application publication is used.
import test, { after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontend = process.env.WEEBDEX_FRONTEND_ROOT; assert(frontend);
const require = createRequire(path.join(frontend, 'package.json')), ts = require('typescript');
const stage = path.join(root, '.build/remote-signer-nostr-tools-2.25.2');
assert.equal(JSON.parse(fs.readFileSync(path.join(stage, 'node_modules/nostr-tools/package.json'))).version, '2.25.2');
const { WebSocketServer } = require(path.join(process.env.WEEBDEX_BROWSER_DOM_NODE_MODULES, 'ws'));
const temp = fs.mkdtempSync(path.join(root, '.build/remote-bunker-test-')); fs.writeFileSync(path.join(temp, 'package.json'), '{"type":"module"}');
fs.symlinkSync(path.join(stage, 'node_modules'), path.join(temp, 'node_modules'));
fs.mkdirSync(path.join(temp, 'protocol')); for (const name of ['codec.js', 'nostr.js']) fs.copyFileSync(path.join(root, 'platform-integrations/local-library/protocol', name), path.join(temp, 'protocol', name));
fs.writeFileSync(path.join(temp, 'remote-bunker.mjs'), ts.transpileModule(fs.readFileSync(path.join(root, 'platform-integrations/local-library/remote-bunker.ts'), 'utf8'), { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText);
after(() => fs.rmSync(temp, { recursive: true, force: true }));
const api = await import(pathToFileURL(path.join(temp, 'remote-bunker.mjs'))), { schnorr } = await import(pathToFileURL(path.join(stage, 'node_modules/@noble/curves/secp256k1.js'))), { sha256 } = await import(pathToFileURL(path.join(stage, 'node_modules/@noble/hashes/sha2.js'))), { v2: nip44 } = await import(pathToFileURL(path.join(stage, 'node_modules/nostr-tools/lib/esm/nip44.js')));
const hex = bytes => Buffer.from(bytes).toString('hex'), bytes = input => Buffer.from(input, 'hex'), text = new TextEncoder(), now = () => Math.floor(Date.now() / 1000);
const syntheticKey = value => { const key = new Uint8Array(32); key[31] = value; return key; };
function signed(template, secret) { const e = { ...template, pubkey: hex(schnorr.getPublicKey(secret)) }; const digest = sha256(text.encode(JSON.stringify([0, e.pubkey, e.created_at, e.kind, e.tags, e.content]))); return { ...e, id: hex(digest), sig: hex(schnorr.sign(digest, secret)) }; }
function valid(e) { const digest = sha256(text.encode(JSON.stringify([0, e.pubkey, e.created_at, e.kind, e.tags, e.content]))); return hex(digest) === e.id && schnorr.verify(bytes(e.sig), digest, bytes(e.pubkey)); }
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
async function fixture({ sameKey = false, switchRelay = false, switchError, expectedKey } = {}) {
  const bunkerSecret = syntheticKey(91), userSecret = sameKey ? bunkerSecret : syntheticKey(92), bunkerKey = hex(schnorr.getPublicKey(bunkerSecret)), userKey = hex(schnorr.getPublicKey(userSecret));
  const server = new WebSocketServer({ host: '127.0.0.1', port: 0 }); await new Promise(resolve => server.once('listening', resolve));
  const origin = `ws://127.0.0.1:${server.address().port}`, relay = origin + '/', requests = [], connections = [], held = [];
  let mode = '', authShown;
  server.on('connection', socket => { connections.push(socket); let subscription;
    socket.on('message', raw => {
      const frame = JSON.parse(raw.toString()); if (frame[0] === 'REQ') { subscription = frame[1]; return; } if (frame[0] !== 'EVENT') return;
      const event = frame[1]; assert(valid(event)); assert.equal(event.kind, 24133); assert.deepEqual(event.tags, [['p', bunkerKey]]);
      const conversation = nip44.utils.getConversationKey(bunkerSecret, event.pubkey), request = JSON.parse(nip44.decrypt(event.content, conversation)); requests.push(request);
      const response = (result, options = {}) => {
        let content = nip44.encrypt(JSON.stringify({ id: options.wrongId ? 'unrelated' : request.id, result, ...(options.error ? { error: options.error } : {}) }), conversation);
        if (options.badMac) { const b = Buffer.from(content, 'base64'); b[b.length - 1] ^= 1; content = b.toString('base64'); }
        const reply = signed({ created_at: now(), kind: 24133, tags: [['p', event.pubkey]], content }, options.wrongSender ? syntheticKey(93) : bunkerSecret);
        if (options.invalidSignature) reply.sig = '0'.repeat(128);
        if (socket.readyState === 1) socket.send(JSON.stringify(['EVENT', subscription, reply]));
      };
      if (request.method === 'connect') { assert.equal(request.params[0], bunkerKey); assert.equal(request.params[1], 'synthetic-once'); response('ack'); }
      else if (request.method === 'get_public_key') { if (mode === 'hold-public-key') held.push(() => response(expectedKey ?? userKey)); else response(expectedKey ?? userKey); }
      else if (request.method === 'switch_relays') { if(switchError!==undefined) response('',{error:switchError}); else if(switchRelay==='hold') held.push(()=>response('null')); else response(JSON.stringify(switchRelay==='same-canonical'?[origin]:switchRelay?['wss://unreviewed.invalid/']:null)); }
      else if (request.method === 'sign_event') {
        const template = JSON.parse(request.params[0]); const result = JSON.stringify(signed(mode === 'changed-template' ? { ...template, content: 'not reviewed' } : template, userSecret));
        if (mode === 'hold') held.push(() => response(result));
        else if (mode === 'auth') { response('auth_url', { error: 'https://signer.example.invalid/approve?challenge=synthetic' }); held.push(() => response(result)); }
        else if (mode === 'bad-auth') response('auth_url', { error: 'javascript:alert(1)' });
        else response(result, { wrongSender: mode === 'wrong-sender', wrongId: mode === 'wrong-id', badMac: mode === 'bad-mac', invalidSignature: mode === 'bad-signature' });
      } else assert.fail('Unapproved method: ' + request.method);
    });
  });
  const review = api.reviewBunker(`bunker://${bunkerKey}?relay=${encodeURIComponent(relay)}&secret=synthetic-once`, [origin], 'http://127.0.0.1:1'), abort = new AbortController();
  let current = true, session;
  return { api, review, abort, requests, connections, userKey, bunkerKey, relay,
    mode(value) { mode = value; }, changeAccount() { current = false; }, release() { for (const fn of held.splice(0)) fn(); }, get authShown() { return authShown; },
    async connect(options = {}) { session = await api.connectBunker(review, { approved: true, signal: abort.signal, assertCurrent() { if (!current) throw Error('Synthetic account changed'); }, onAuthURL(url) { authShown = url; }, requestTimeoutMs: 700, ...options }); return session; },
    async close() { session?.clear(); abort.abort(); for (const socket of server.clients) socket.terminate(); await new Promise(resolve => server.close(resolve)); bunkerSecret.fill(0); userSecret.fill(0); },
  };
}
const template = () => ({ kind: 0, created_at: now(), tags: [], content: '{"name":"Synthetic"}' });
test('bunker review rejects private keys, unsafe/multiple/unapproved endpoints and requires explicit consent before socket creation', async () => {
  const f = await fixture(); try {
    for (const input of ['nsec1not-a-public-connection', `bunker://${f.bunkerKey}?relay=wss://safe.invalid&relay=wss://second.invalid`, `bunker://${f.bunkerKey}?relay=wss://127.0.0.1`, `bunker://${f.bunkerKey}?relay=wss://safe.invalid&token=hidden`]) assert.throws(() => api.reviewBunker(input, ['wss://safe.invalid'], 'https://site.invalid'));
    assert.throws(() => api.reviewBunker(`bunker://${f.bunkerKey}?relay=wss://safe.invalid`, [], 'https://site.invalid'), /does not allow/);
    assert.equal(JSON.stringify(f.review).includes('synthetic-once'), false); let opened = 0;
    await assert.rejects(f.connect({ approved: false, socketFactory() { opened++; throw Error(); } }), /approve/); assert.equal(opened, 0);
  } finally { await f.close(); }
});
for (const sameKey of [false, true]) test(`real NIP44 bunker handshake and exact event signing; bunker/user keys ${sameKey ? 'same is allowed' : 'distinct'}`, async () => {
  const f = await fixture({ sameKey }); try { const session = await f.connect(); assert.equal(await session.getPublicKey(), f.userKey); const e = template(), result = await session.signEvent(e); assert(valid(result)); assert.equal(result.pubkey, f.userKey); assert.equal(result.content, e.content); assert.deepEqual(f.requests.map(r => r.method), ['connect', 'get_public_key', 'switch_relays', 'sign_event']); assert.equal(f.requests[0].params[2], api.REMOTE_SIGNING_KINDS.map(k => 'sign_event:' + k).join(',')); await assert.rejects(session.signEvent({ ...e, kind: 4 }), /outside/); assert.equal(f.requests.length, 4); assert.equal(f.connections.length, 1); } finally { await f.close(); }
});
for (const mode of ['changed-template', 'bad-mac', 'bad-auth', 'wrong-sender', 'wrong-id', 'bad-signature']) test(`untrusted remote response refused without retry: ${mode}`, async () => {
  const f = await fixture(); try { const session = await f.connect(); f.mode(mode); await assert.rejects(session.signEvent(template()), /changed|verified|timed out/); assert.equal(f.requests.filter(r => r.method === 'sign_event').length, 1); } finally { await f.close(); }
});
test('cancel pending signing wipes session and ignores late reply; concurrent sign is refused', async () => {
  const f = await fixture(); try { const session = await f.connect(); f.mode('hold'); const pending = session.signEvent(template()); pending.catch(() => {}); while (!f.requests.some(r => r.method === 'sign_event')) await pause(5); await assert.rejects(session.signEvent(template()), /already pending/); f.abort.abort(); f.release(); await assert.rejects(pending, /cancelled/); await assert.rejects(session.signEvent(template()), /disconnected/); assert.equal(f.requests.filter(r => r.method === 'sign_event').length, 1); } finally { await f.close(); }
});
test('verified auth URL is offered only as data, not opened; same request completes after approval', async () => {
  const f = await fixture(); try { const session = await f.connect(); f.mode('auth'); const pending = session.signEvent(template()); while (!f.authShown) await pause(5); assert.equal(new URL(f.authShown).origin, 'https://signer.example.invalid'); f.release(); assert(valid(await pending)); assert.equal(f.requests.filter(r => r.method === 'sign_event').length, 1); } finally { await f.close(); }
});
test('account change during remote approval rejects late signature', async () => {
  const f = await fixture(); try { const session = await f.connect(); f.mode('hold'); const pending = session.signEvent(template()); pending.catch(() => {}); while (!f.requests.some(r => r.method === 'sign_event')) await pause(5); f.changeAccount(); f.release(); await assert.rejects(pending, /changed|verified/); assert.equal(f.requests.filter(r => r.method === 'sign_event').length, 1); } finally { await f.close(); }
});
test('linked-account readback change discards otherwise valid signed result', async () => {
  const f = await fixture(); try { const session = await f.connect({ beforeSign: async () => async () => { throw Error('Synthetic link revision changed'); } }); await assert.rejects(session.signEvent(template()), /revision changed/); assert.equal(f.requests.filter(r => r.method === 'sign_event').length, 1); } finally { await f.close(); }
});
test('remote relay change fails closed without contacting a second endpoint', async () => { const f = await fixture({ switchRelay: true }); try { await assert.rejects(f.connect(), /different relay|does not allow/); assert.equal(f.connections.length, 1); assert.equal(f.requests.some(r => r.method === 'sign_event'), false); } finally { await f.close(); } });

test('cancel during get_public_key handshake ignores the held authenticated reply', async () => { const f = await fixture(); try { f.mode('hold-public-key'); const pending=f.connect(); pending.catch(()=>{}); while(!f.requests.some(r=>r.method==='get_public_key')) await pause(5); f.abort.abort(); f.release(); await assert.rejects(pending,/cancelled/); assert.equal(f.requests.some(r=>r.method==='sign_event'||r.method==='switch_relays'),false); } finally { await f.close(); } });
test('account change during get_public_key handshake cannot select a late identity', async () => { const f = await fixture(); try { f.mode('hold-public-key'); const pending=f.connect(); pending.catch(()=>{}); while(!f.requests.some(r=>r.method==='get_public_key')) await pause(5); f.changeAccount(); f.release(); await assert.rejects(pending,/changed|verified/); assert.equal(f.requests.some(r=>r.method==='sign_event'||r.method==='switch_relays'),false); } finally { await f.close(); } });
test('whole signing operation timeout fences a late native preflight before any sign_event RPC', async () => { const f=await fixture();let release;try{const gate=new Promise(resolve=>release=resolve);const session=await f.connect({beforeSign:async()=>{await gate;}});const pending=session.signEvent(template());await assert.rejects(pending,/timed out/);release();await pause(5);assert.equal(f.requests.some(r=>r.method==='sign_event'),false);}finally{release?.();await f.close();}});

test('malformed native guard callback is refused before sign_event transport',async()=>{const f=await fixture();try{const session=await f.connect({beforeSign:async()=>42});await assert.rejects(session.signEvent(template()),/callback is invalid/);assert.equal(f.requests.some(r=>r.method==='sign_event'),false);}finally{await f.close();}});

test('elapsed deadline blocks a late signer RPC even before a delayed timer callback runs',async()=>{const f=await fixture();try{const session=await f.connect({beforeSign:async()=>{const until=performance.now()+750;while(performance.now()<until){/* deterministic owned event-loop delay */}}});await assert.rejects(session.signEvent(template()),/timed out/);assert.equal(f.requests.some(r=>r.method==='sign_event'),false);}finally{await f.close();}});

test('authenticated explicit optional relay-method refusal retains only the exact reviewed relay',async()=>{const f=await fixture({switchError:'unsupported method'});try{const session=await f.connect();assert.equal(session.relayPreferencesAvailable,false);assert(valid(await session.signEvent(template())));assert.equal(f.connections.length,1);assert.equal(f.requests.filter(r=>r.method==='switch_relays').length,1);}finally{await f.close();}});
test('same canonical relay spelling is accepted without opening another connection',async()=>{const f=await fixture({switchRelay:'same-canonical'});try{const session=await f.connect();assert.equal(session.relayPreferencesAvailable,true);assert.equal(f.connections.length,1);}finally{await f.close();}});
for(const switchError of [{unsupported:true},'x'.repeat(1025)])test('malformed optional-method error is not compatible success: '+typeof switchError,async()=>{const f=await fixture({switchError});try{await assert.rejects(f.connect(),/verified/);assert.equal(f.requests.some(r=>r.method==='sign_event'),false);assert.equal(f.connections.length,1);}finally{await f.close();}});
test('optional relay-method timeout never becomes fallback success',async()=>{const f=await fixture({switchRelay:'hold'});try{await assert.rejects(f.connect(),/timed out/);assert.equal(f.requests.some(r=>r.method==='sign_event'),false);assert.equal(f.connections.length,1);}finally{await f.close();}});
