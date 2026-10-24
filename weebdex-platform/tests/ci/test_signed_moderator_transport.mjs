// Actual transport with finite in-memory primitives. No signatures, network or browser claim.
import fs from 'node:fs';import path from 'node:path';import vm from 'node:vm';import {pathToFileURL} from 'node:url';import {stripTypeScriptTypes} from 'node:module';import {createHash} from 'node:crypto';import test from 'node:test';import assert from 'node:assert/strict';
import {root} from './report-draft-fixture.mjs';import {readModeratorTransportAssociation,restoreModeratorTransportSource,moderatorTransportAssociationSha256} from './reviewed-moderator-transport-source-successors.mjs';
const source = fs.readFileSync(new URL('../../platform-integrations/public-api/nostr-upload-request.ts', import.meta.url), 'utf8');
const text = new TextEncoder();
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const clone = value => JSON.parse(JSON.stringify(value));
function deferred() { let resolve; const promise = new Promise(yes => { resolve = yes; }); return { promise, resolve }; }

async function fixture(options = {}) {
  let clock = 0, alive = true, identityCurrent = true, signerCalls = 0, proofCalls = 0, beforeCalls = 0;
  const timers = new Map(), requests = [], templates = [], timerHistory = [];
  const signing = deferred(), signEntered = deferred(), preflight = deferred(), preflightEntered = deferred();
  const owner = '1'.repeat(64), serviceKey = '2'.repeat(64);
  const scope = { origin: 'https://fixture.invalid', serviceKey, catalogNamespace: '33333333-3333-8333-8333-333333333333' };
  const identity = { owner, generation: 1, check() { if (!identityCurrent) throw Error('SECRET selected identity changed'); }, signer: {
    async signEvent(event) {
      signerCalls++; signEntered.resolve();
      if (options.holdSigner) await signing.promise;
      options.onSign?.();
      return { ...clone(event), id: 'synthetic-id', sig: 'synthetic-signature' };
    },
  } };
  const primitives = {
    async signExact(signer, selectedOwner, event) {
      proofCalls++; templates.push(clone(event));
      if (options.beforeSigner) await options.beforeSigner();
      return signer.signEvent({ ...event, pubkey: selectedOwner });
    },
    hash: digest,
    b64: bytes => Buffer.from(bytes).toString('base64'),
    nonce: () => 'synthetic_nonce_0001', now: () => 1700000000, clock: () => clock,
    setTimer(callback, delay) { const token = {}; timers.set(token, { callback, at: clock + delay }); timerHistory.push(delay); return token; },
    clearTimer(token) { timers.delete(token); },
    async fetch(url, init) {
      const request = { url, init, abortCount: 0 };
      init.signal.addEventListener('abort', () => { request.abortCount++; }, { once: true });
      requests.push(request);
      if (options.fetch) return options.fetch(url, init);
      return new Response('{"result":"ok"}', { status: 201, headers: { 'content-type': 'application/json' } });
    },
  };
  const context = vm.createContext({ URL, TextEncoder, TextDecoder, Uint8Array, AbortController, Promise, Error, Number, Object,
    setTimeout, clearTimeout, performance, crypto, fetch() { throw Error('A real fetch is forbidden'); } });
  const nostr = new vm.SyntheticModule(['signExact', 'origin'], function () {
    this.setExport('signExact', primitives.signExact);
    this.setExport('origin', value => {
      const url = new URL(value);
      if (url.protocol !== 'https:' || url.username || url.password || url.search || url.hash || url.pathname !== '/') throw Error('Invalid fixture origin');
      return url.origin;
    });
  }, { context });
  const codec = new vm.SyntheticModule(['hash', 'b64'], function () { this.setExport('hash', digest); this.setExport('b64', primitives.b64); }, { context });
  const module = new vm.SourceTextModule(stripTypeScriptTypes(options.transportSource ?? source), { context });
  await module.link(specifier => {
    if (specifier === '../local-library/protocol/nostr.js') return nostr;
    if (specifier === '../local-library/protocol/codec.js') return codec;
    throw Error('Unreviewed import: ' + specifier);
  });
  await module.evaluate();
  const api = module.namespace;
  const transport = api.createNostrUploadRequest({ scope, identity,
    assertCurrent() { if (!alive) throw Error('SECRET configured scope or view changed'); options.onCheck?.(); },
    async beforeSend(signal) {
      beforeCalls++; preflightEntered.resolve();
      if (options.holdBeforeSend) await preflight.promise;
      if (options.beforeSend) await options.beforeSend(signal);
    },
  }, primitives);
  const request = overrides => transport({ url: scope.origin + '/api/v1/upload/begin', method: 'POST',
    body: text.encode('{"manga":"synthetic"}'), contentType: 'application/json', ...overrides });
  return { api, transport, request, requests, templates, timers, timerHistory, scope, identity,
    get signerCalls() { return signerCalls; }, get proofCalls() { return proofCalls; }, get beforeCalls() { return beforeCalls; },
    close() { alive = false; }, changeIdentity() { identityCurrent = false; },
    advance(milliseconds) {
      clock += milliseconds;
      for (const [token, timer] of [...timers]) if (timer.at <= clock) { timers.delete(token); timer.callback(); }
    },
    signEntered: signEntered.promise, releaseSign: signing.resolve,
    preflightEntered: preflightEntered.promise, releasePreflight: preflight.resolve,
  };
}
const errorIs = (outcome, status) => error => {
  assert.equal(error.name, 'PublicUploadRequestError'); assert.equal(error.outcome, outcome);
  assert.equal(error.status, status); assert.equal(error.bodyAvailable, false);
  assert(!error.message.includes('SECRET')); assert(!error.message.includes('synthetic-signature'));
  return true;
};
const tags = f => Object.fromEntries(f.templates[0].tags);

const association=readModeratorTransportAssociation(),route='/api/v1/nostr/moderation/reports/report%3A1';
const raw=notes=>text.encode(JSON.stringify({expectedAccountId:'1'.repeat(36),expectedReportVersion:1,expectedReasonVersion:1,expectedTargetRevision:'a'.repeat(64),expectedContextHash:'b'.repeat(64),expectedEffect:{},action:'accept',notes}));
test('actual current transport admits full escaped20000 native notes under exact131072 POST route; old source fails unsent',async()=>{const body=raw('\x01'.repeat(20000));assert.ok(body.length>120000&&body.length<131072);const f=await fixture();await f.request({url:f.scope.origin+route,body});assert.equal(f.requests.length,1);assert.equal(f.requests[0].init.credentials,'omit');assert.equal(tags(f).payload,digest(body));const old=await fixture({transportSource:restoreModeratorTransportSource(association.path,source,'raw')});await assert.rejects(()=>old.request({url:old.scope.origin+route,body}),errorIs('unsent'));assert.equal(old.requests.length,0);assert.equal(old.signerCalls,0);});
test('actual transport accepts exact131072 bytes and fails overcap before signer even with requested larger limit',async()=>{const f=await fixture();await f.request({url:f.scope.origin+route,body:new Uint8Array(131072)});assert.equal(f.requests.length,1);for(const requestLimit of [undefined,131073,999999]){const next=await fixture();await assert.rejects(()=>next.request({url:next.scope.origin+route,body:new Uint8Array(131073),requestLimit}),errorIs('unsent'));assert.equal(next.signerCalls,0);}});
for(const suffix of ['/','/extra','?unknown=1','?','%2Fother','%253A1','%3a1'])test('hostile moderator path/query does not inherit large JSON exception '+suffix,async()=>{const f=await fixture();await assert.rejects(()=>f.request({url:f.scope.origin+route+suffix,body:raw('\x01'.repeat(20000))}),errorIs('unsent'));assert.equal(f.signerCalls,0);});
for(const id of ['',':'.repeat(129),'bad%20id','bad%2Fid','bad!id'])test('invalid native ID cannot receive large exception '+id.slice(0,20),async()=>{const f=await fixture();await assert.rejects(()=>f.request({url:f.scope.origin+'/api/v1/nostr/moderation/reports/'+id,body:raw('\x01'.repeat(20000))}),errorIs('unsent'));assert.equal(f.signerCalls,0);});
for(const patch of [{method:'GET'},{method:'PATCH'},{method:'PUT'},{method:'DELETE'},{responseType:'image'},{contentType:undefined},{contentType:'text/plain'}])test('method/type exception remains exact '+JSON.stringify(patch),async()=>{const f=await fixture();await assert.rejects(()=>f.request({url:f.scope.origin+route,body:raw('\x01'.repeat(20000)),...patch}),errorIs('unsent'));assert.equal(f.signerCalls,0);});
test('small exact GET remains empty-body read and unrelated native routes preserve old limits',async()=>{const f=await fixture();await f.request({url:f.scope.origin+route+'?expectedAccountId=x',method:'GET',body:undefined,contentType:undefined});assert.equal(f.requests.length,1);for(const p of ['/api/v1/nostr/reports/user/id','/api/v1/nostr/moderation/reports','/api/v1/reports/id/accept']){const n=await fixture();await assert.rejects(()=>n.request({url:n.scope.origin+p,body:raw('\x01'.repeat(20000))}),errorIs('unsent'));assert.equal(n.signerCalls,0);}});
test('raw/QA exact source successor maps only frozen bytes and never changes old metadata',async()=>{const modulePath=process.env.WEEBDEX_QA_HELPER??path.join(root,'scripts/public-upload-refusal-qa.mjs');const {stripUploadRefusalQaSource:strip}=await import(pathToFileURL(modulePath));const bytes=fs.readFileSync(path.join(root,'docs/fixtures/moderator-transport-source-association.json'));assert.equal(digest(bytes),moderatorTransportAssociationSha256);for(const mode of ['raw','qa_stripped']){const current=mode==='raw'?source:strip(source),previous=restoreModeratorTransportSource(association.path,current,mode),row=association[mode];assert.equal(digest(current),row.current_sha256);assert.equal(digest(previous),row.previous_sha256);for(const value of [current+' ',previous,{},null,Buffer.from(current+' ')])assert.throws(()=>restoreModeratorTransportSource(association.path,value,mode));assert.throws(()=>restoreModeratorTransportSource(association.path+'x',current,mode));for(const key of Object.keys(association)){const changed=structuredClone(association);changed[key]='bad';assert.throws(()=>readModeratorTransportAssociation(Buffer.from(JSON.stringify(changed))));}}assert.equal(strip(restoreModeratorTransportSource(association.path,source,'raw')),restoreModeratorTransportSource(association.path,strip(source),'qa_stripped'));assert.equal(association.browser_executed,false);assert.equal(association.http_executed,false);assert.equal(association.pipe_timing_qualified,false);assert.doesNotMatch(source,/restoreModeratorTransportSource|reviewed-moderator/);});
