// Exact-byte NIP-98 transport for the public upload API. Unlike relay
// signedRequest, this API does not issue x-manga-response-proof headers.
import { signExact, origin as canonicalOrigin } from '../local-library/protocol/nostr.js';
import { hash, b64 } from '../local-library/protocol/codec.js';

export interface UploadRequestScope { origin: string; serviceKey: string; catalogNamespace: string }
export interface UploadRequestIdentity {
  owner: string;
  generation: number;
  signer: { signEvent(event: unknown): Promise<unknown> };
  check(): void;
}
export interface UploadRequestContext {
  // The adapter proves the service and captures the configured catalogue and
  // signer first. assertCurrent must fence that exact captured scope and view.
  scope: UploadRequestScope;
  identity: UploadRequestIdentity;
  assertCurrent(): void;
  beforeSend?(signal: AbortSignal): Promise<void>;
}
export interface PublicUploadRequest {
  url: string;
  method: 'GET' | 'POST' | 'PATCH' | 'PUT' | 'DELETE';
  // Serialize multipart ONCE before calling. The boundary must match these
  // bytes. Never pass FormData to fetch after signing a different encoding.
  body?: Uint8Array;
  contentType?: string;
  responseType?: 'json' | 'image';
  signal?: AbortSignal;
  timeoutMs?: number;
  requestLimit?: number;
  responseLimit?: number;
}
export interface PublicUploadResponse {
  status: number;
  body: unknown;
  bodyAvailable: true;
  headers: {
    'retry-after': string | null;
    'content-type': string | null;
    'cache-control': string | null;
    vary: string | null;
    'x-content-type-options': string | null;
    'x-weebdex-instance-removal': string | null;
    'x-weebdex-source-deletion-status': string | null;
    'x-weebdex-source-deletion-request': string | null;
  };
}
export type PublicUploadOutcome = 'unsent' | 'uncertain' | 'response';
export class PublicUploadRequestError extends Error {
  readonly bodyAvailable = false;
  readonly outcome: PublicUploadOutcome;
  readonly status?: number;
  constructor(message: string, outcome: PublicUploadOutcome, status?: number) {
    super(message);
    this.name = 'PublicUploadRequestError';
    this.outcome = outcome;
    this.status = status;
  }
}
export const uploadRequestRefusalStages = Object.freeze([
  'checkpoint', 'fetch', 'response-header', 'response-media', 'declared-size',
  'body-read', 'body-budget', 'decode', 'other',
] as const);
export type UploadRequestRefusalStage = typeof uploadRequestRefusalStages[number];
// Only the exact error created by this transport's catch owns a code. Looking
// up an unknown/forged/proxied error must never inspect its properties/getters.
const requestRefusals = new WeakMap<object, UploadRequestRefusalStage>();
export function uploadRequestRefusalStage(error: unknown): UploadRequestRefusalStage | 'unavailable' {
  return error !== null && (typeof error === 'object' || typeof error === 'function')
    ? requestRefusals.get(error) ?? 'unavailable' : 'unavailable';
}
// QA_UPLOAD_REFUSAL_RING_BEGIN
// The exact test-build plugin alone replaces this default-off constant.
const uploadRequestRefusalRecording = false;
type UploadRequestRefusalRecord = Readonly<{ event_id: string; stage: UploadRequestRefusalStage; transport_status: number | null }>;
const uploadRequestRefusalRecords: UploadRequestRefusalRecord[] = [];
let uploadRequestRefusalRecordValid = true, uploadRequestRefusalTruncated = false;
function recordUploadRequestRefusal(eventId: string | undefined, stage: UploadRequestRefusalStage, status: number | undefined): void {
  if (!uploadRequestRefusalRecording) return;
  if (uploadRequestRefusalRecords.length >= 16) uploadRequestRefusalTruncated = true;
  if (typeof eventId !== 'string' || !/^[0-9a-f]{64}$/.test(eventId) || uploadRequestRefusalRecords.some(row => row.event_id === eventId)) { uploadRequestRefusalRecordValid = false; return; }
  if (uploadRequestRefusalRecords.length >= 16) return;
  uploadRequestRefusalRecords.push(Object.freeze({ event_id: eventId, stage, transport_status: status ?? null }));
}
export function readUploadRequestRefusals() {
  return { schema: 1, enabled: uploadRequestRefusalRecording, record_valid: uploadRequestRefusalRecordValid,
    truncated: uploadRequestRefusalTruncated, records: uploadRequestRefusalRecords.map(row => ({ ...row })) };
}
// QA_UPLOAD_REFUSAL_RING_END
type AuthTemplate = { kind: number; created_at: number; content: string; tags: string[][] };
export interface UploadRequestPrimitives {
  signExact(signer: UploadRequestIdentity['signer'], owner: string, event: AuthTemplate): Promise<unknown>;
  hash(bytes: Uint8Array): string;
  b64(bytes: Uint8Array): string;
  fetch: typeof fetch;
  nonce(): string;
  now(): number;
  clock(): number;
  setTimer(callback: () => void, milliseconds: number): ReturnType<typeof setTimeout>;
  clearTimer(timer: ReturnType<typeof setTimeout>): void;
}
const defaults: UploadRequestPrimitives = {
  signExact: (signer, owner, event) => signExact(signer as Parameters<typeof signExact>[0], owner, event),
  hash, b64,
  fetch: (...args) => fetch(...args),
  nonce: () => crypto.randomUUID(),
  now: () => Math.floor(Date.now() / 1000),
  clock: () => performance.now(),
  setTimer: (callback, milliseconds) => setTimeout(callback, milliseconds),
  clearTimer: timer => clearTimeout(timer),
};
const JSON_LIMIT = 32 * 1024, MULTIPART_LIMIT = 32 * 1024 * 1024;
const JSON_RESPONSE_LIMIT = 1024 * 1024, IMAGE_LIMIT = 32 * 1024 * 1024;
// Only the bounded native-discussion endpoint may carry existing 50k-byte
// comments (JSON escaping can multiply their serialized size by six).
const DISCUSSION_JSON_LIMIT = 393216, DISCUSSION_RESPONSE_LIMIT = 8 * 1024 * 1024, NATIVE_REPORT_JSON_LIMIT = 131072;
const encoder = new TextEncoder(), decoder = new TextDecoder('utf-8', { fatal: true });
function budget(value: number | undefined, maximum: number): number {
  if (value === undefined) return maximum;
  if (!Number.isSafeInteger(value) || value < 0 || value > maximum) throw Error('Invalid upload request budget');
  return value;
}
function discard(response: Response | undefined): void {
  // An ignored/late fetch or a failed header check must not retain its body.
  try { void response?.body?.cancel().catch(() => {}); } catch { /* already locked */ }
}

export function createNostrUploadRequest(context: UploadRequestContext, injected: Partial<UploadRequestPrimitives> = {}) {
  const primitives = { ...defaults, ...injected };
  const scope = Object.freeze({ ...context.scope });
  const selected = context.identity;
  const owner = selected.owner, generation = selected.generation, signer = selected.signer;
  const identityCheck = selected.check.bind(selected), assertCurrent = context.assertCurrent.bind(context);
  const beforeSend = context.beforeSend?.bind(context);
  if (canonicalOrigin(scope.origin) !== scope.origin || !/^[0-9a-f]{64}$/.test(scope.serviceKey) ||
      !/^[0-9a-f]{64}$/.test(owner) || !Number.isSafeInteger(generation) || generation < 0 ||
      typeof scope.catalogNamespace !== 'string' || !scope.catalogNamespace || scope.catalogNamespace.length > 128 ||
      /[\u0000-\u0020\u007f]/.test(scope.catalogNamespace)) throw Error('Invalid verified upload scope');

  return async (input: PublicUploadRequest): Promise<PublicUploadResponse> => {
    const started = primitives.clock();
    let dispatched = false, status: number | undefined, response: Response | undefined;
    let reader: ReadableStreamDefaultReader<Uint8Array> | undefined;
    let bodyFinished = false, finished = false, completed = false;
    let refusalStage: UploadRequestRefusalStage = 'other';
    // QA_UPLOAD_REFUSAL_ID_BEGIN
    let refusalEventId: string | undefined;
    // QA_UPLOAD_REFUSAL_ID_END
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout> | undefined;
    let rejectAbort: (reason: Error) => void = () => {};
    const aborted = new Promise<never>((_, reject) => { rejectAbort = reject; });
    // Install before any operation can abort; even validation failures clean up.
    const onAbort = () => rejectAbort(Error('Upload request cancelled or deadline exceeded'));
    controller.signal.addEventListener('abort', onAbort, { once: true });
    const externalAbort = () => controller.abort();
    const externalSignal = input.signal;
    externalSignal?.addEventListener('abort', externalAbort, { once: true });
    // The abort promise is also used by the wrapped production signExact signer.
    // A deadline abandons an extension result and settles signExact's own timer.
    void aborted.catch(() => {});
    let deadline = started;
    function checkpoint(): void {
      const previous = refusalStage; refusalStage = 'checkpoint';
      if (finished || controller.signal.aborted || primitives.clock() >= deadline) throw Error('Upload request cancelled or deadline exceeded');
      identityCheck();
      if (selected.owner !== owner || selected.generation !== generation || selected.signer !== signer) throw Error('Upload signer selection changed');
      assertCurrent();
      if (controller.signal.aborted || primitives.clock() >= deadline) throw Error('Upload request cancelled or deadline exceeded');
      refusalStage = previous;
    }
    async function guarded<T>(operation: () => T | PromiseLike<T>): Promise<T> {
      checkpoint();
      const result = await Promise.race([Promise.resolve().then(() => { checkpoint(); return operation(); }), aborted]);
      checkpoint();
      return result;
    }
    try {
      const timeout = budget(input.timeoutMs, 60000);
      if (timeout === 0) throw Error('Upload request deadline exceeded');
      deadline = started + timeout;
      timer = primitives.setTimer(() => controller.abort(), Math.max(0, deadline - primitives.clock()));
      if (externalSignal?.aborted) controller.abort();
      checkpoint();
      // Snapshot caller-controlled values before the first asynchronous callback.
      const url = input.url, method = input.method, contentType = input.contentType;
      const responseType = input.responseType ?? 'json';
      const parsed = new URL(url);
      if (url.length > 32768 || parsed.href !== url || parsed.origin !== scope.origin || parsed.username || parsed.password ||
          url.includes('#') || /[\\\u0000-\u0020\u007f]/.test(url)) throw Error('Upload request URL escaped the captured origin');
      if (!['GET', 'POST', 'PATCH', 'PUT', 'DELETE'].includes(method)) throw Error('Unsupported upload request method');
      if (!['json', 'image'].includes(responseType)) throw Error('Unsupported upload response type');
      const multipart = contentType !== undefined && /^multipart\/form-data;\s*boundary=[A-Za-z0-9_-]{1,70}$/.test(contentType);
      if (contentType !== undefined && contentType !== 'application/json' && !multipart) throw Error('Upload body requires JSON or an explicit multipart boundary');
      const discussion = /^\/api\/v1\/nostr\/user\/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\/discussion$/.test(parsed.pathname) && ['GET','POST'].includes(method) && responseType === 'json';
      const genericDiscussion = /^\/api\/v1\/nostr\/(?:manga|chapter|group)\/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\/discussion$/.test(parsed.pathname) && responseType === 'json' && (method === 'GET' || method === 'POST' && !parsed.search && contentType === 'application/json');
      const discussionWrite = (discussion || genericDiscussion) && method === 'POST' && !parsed.search && contentType === 'application/json';
      const nativeReportWrite = /^\/api\/v1\/nostr\/(?:user|manga|chapter|group)\/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\/discussion\/reports$/.test(parsed.pathname) && method === 'POST' && !parsed.search && contentType === 'application/json' && responseType === 'json';
      const moderatorDecisionWrite = /^\/api\/v1\/nostr\/moderation\/reports\/(?:[A-Za-z0-9_:-]|%3A){1,128}$/.test(parsed.pathname) && method === 'POST' && !url.includes('?') && contentType === 'application/json' && responseType === 'json';
      const requestLimit = budget(input.requestLimit, multipart ? MULTIPART_LIMIT : discussionWrite ? DISCUSSION_JSON_LIMIT : (nativeReportWrite || moderatorDecisionWrite) ? NATIVE_REPORT_JSON_LIMIT : JSON_LIMIT);
      const responseLimit = budget(input.responseLimit, responseType === 'image' ? IMAGE_LIMIT : discussion || genericDiscussion ? DISCUSSION_RESPONSE_LIMIT : JSON_RESPONSE_LIMIT);
      if (input.body !== undefined && !(input.body instanceof Uint8Array)) throw Error('Upload body must contain serialized bytes');
      if ((input.body?.byteLength ?? 0) > requestLimit) throw Error('Upload request body exceeds budget');
      const bytes = input.body === undefined ? new Uint8Array() : new Uint8Array(input.body);
      if (method === 'GET' && (bytes.length || contentType !== undefined)) throw Error('Upload GET requests must have an empty body');
      if (bytes.length && contentType === undefined) throw Error('Upload body content type is required');
      const nonce = primitives.nonce();
      if (typeof nonce !== 'string' || !/^[A-Za-z0-9_-]{16,128}$/.test(nonce)) throw Error('Invalid upload request nonce');
      const tags = [['u', url], ['method', method], ['service', scope.serviceKey], ['nonce', nonce]];
      if (method !== 'GET') tags.push(['payload', primitives.hash(bytes)]);
      const proof = await guarded(() => primitives.signExact({
        signEvent: template => guarded(() => signer.signEvent(template)),
      }, owner, { kind: 27235, created_at: primitives.now(), content: '', tags }));
      const proofBytes = encoder.encode(JSON.stringify(proof));
      const authorization = 'Nostr ' + primitives.b64(proofBytes);
      if (authorization.length > 16384) throw Error('Upload authorization exceeds budget');
      // QA_UPLOAD_REFUSAL_PARSE_BEGIN
      if (uploadRequestRefusalRecording) try {
        // Parse only the exact bounded bytes already used for authorization.
        // Never reread signer result properties, invoke getters, or retain proof.
        const serializedProof = JSON.parse(decoder.decode(proofBytes));
        if (serializedProof && typeof serializedProof === 'object' && !Array.isArray(serializedProof) && Object.hasOwn(serializedProof, 'id') &&
            typeof serializedProof.id === 'string' && /^[0-9a-f]{64}$/.test(serializedProof.id)) refusalEventId = serializedProof.id;
      } catch { /* A dispatched refusal without canonical proof ID is unverified. */ }
      // QA_UPLOAD_REFUSAL_PARSE_END
      const headers: Record<string, string> = { authorization, accept: responseType === 'image' ? 'image/*, application/json' : 'application/json' };
      if (contentType !== undefined) headers['content-type'] = contentType;
      if (beforeSend) await guarded(() => beforeSend(controller.signal));
      refusalStage = 'fetch';
      response = await guarded(() => {
        // Nothing asynchronous occurs between this fence and the only dispatch.
        checkpoint();
        dispatched = true;
        return primitives.fetch(url, {
          method, headers, body: method === 'GET' ? undefined : bytes,
          credentials: 'omit', redirect: 'error', cache: 'no-store', signal: controller.signal,
        }).then(result => {
          refusalStage = 'response-header';
          if (finished) { discard(result); throw Error('Late upload response discarded'); }
          response = result;
          if (Number.isInteger(result.status) && result.status >= 200 && result.status <= 599) status = result.status;
          return result;
        });
      });
      if (status === undefined) throw Error('Upload response status unavailable');
      const receivedResponse = response;
      if (!receivedResponse) throw Error('Upload response unavailable');
      const responseHeaders = receivedResponse.headers;
      function safeHeader(name: string): string | null {
        const value = responseHeaders.get(name);
        if (value !== null && (value.length > 4096 || /[\u0000-\u001f\u007f]/.test(value))) throw Error('Upload response header exceeds policy');
        return value;
      }
      // Expose only these bounded actual response fields. In particular, never
      // copy authorization, cookies or relay-proof headers to the adapter.
      const receivedHeaders = {
        'retry-after': safeHeader('retry-after'),
        'content-type': safeHeader('content-type'),
        'cache-control': safeHeader('cache-control'),
        vary: safeHeader('vary'),
        'x-content-type-options': safeHeader('x-content-type-options'),
        'x-weebdex-instance-removal': safeHeader('x-weebdex-instance-removal'),
        'x-weebdex-source-deletion-status': safeHeader('x-weebdex-source-deletion-status'),
        'x-weebdex-source-deletion-request': safeHeader('x-weebdex-source-deletion-request'),
      };
      refusalStage = 'response-media';
      const mediaType = receivedHeaders['content-type']?.split(';', 1)[0].trim().toLowerCase();
      const image = responseType === 'image' && status >= 200 && status < 300;
      if (image ? !/^image\/(?:png|jpeg|webp|gif|avif)$/.test(mediaType ?? '') :
          ![204, 205].includes(status) && mediaType !== 'application/json') throw Error('Unexpected upload response content type');
      const limit = image ? responseLimit : Math.min(responseLimit, discussion || genericDiscussion ? DISCUSSION_RESPONSE_LIMIT : JSON_RESPONSE_LIMIT);
      refusalStage = 'declared-size';
      const declaredSize = responseHeaders.get('content-length');
      if (declaredSize !== null && /^\d+$/.test(declaredSize) && Number(declaredSize) > limit) throw Error('Upload response exceeds budget');
      refusalStage = 'body-read'; reader = receivedResponse.body?.getReader();
      const chunks: Uint8Array[] = [];
      let size = 0;
      if (reader) for (;;) {
        refusalStage = 'body-read';
        const part = await guarded(() => reader!.read());
        if (part.done) break;
        refusalStage = 'body-budget';
        if (!(part.value instanceof Uint8Array) || part.value.byteLength > limit - size) throw Error('Upload response exceeds budget');
        size += part.value.byteLength;
        chunks.push(new Uint8Array(part.value));
      }
      bodyFinished = true;
      refusalStage = 'body-budget';
      const received = new Uint8Array(size);
      let offset = 0;
      for (const chunk of chunks) { received.set(chunk, offset); offset += chunk.length; }
      let body: unknown;
      refusalStage = 'decode';
      if (image) body = received;
      else if ([204, 205].includes(status) && !received.length) body = undefined;
      else body = JSON.parse(decoder.decode(received));
      checkpoint();
      completed = true;
      return { status, body, bodyAvailable: true, headers: receivedHeaders };
    } catch {
      // Never include signer errors, URLs, authorization or server text in a
      // diagnostic. A dispatched mutation with no status must be reconciled,
      // never automatically replayed. A known status remains known even if
      // cancellation or malformed JSON makes its body unavailable.
      const outcome = status !== undefined ? 'response' : dispatched ? 'uncertain' : 'unsent';
      const error = new PublicUploadRequestError(status !== undefined ? `Upload response body unavailable (HTTP ${status})` :
        dispatched ? 'Upload request outcome is unknown; reconcile before retrying' : 'Upload request was not sent', outcome, status);
      requestRefusals.set(error, refusalStage);
      // QA_UPLOAD_REFUSAL_RECORD_BEGIN
      if (dispatched) try { recordUploadRequestRefusal(refusalEventId, refusalStage, status); }
      catch { if (uploadRequestRefusalRecording) uploadRequestRefusalRecordValid = false; }
      // QA_UPLOAD_REFUSAL_RECORD_END
      throw error;
    } finally {
      finished = true;
      if (timer !== undefined) primitives.clearTimer(timer);
      externalSignal?.removeEventListener('abort', externalAbort);
      if (reader) {
        if (!bodyFinished) try { void reader.cancel().catch(() => {}); } catch { /* best effort */ }
        try { reader.releaseLock(); } catch { /* pending read is being cancelled */ }
      } else if (!bodyFinished) discard(response);
      // Abandon outstanding callbacks on failure, including a pending signer
      // prompt. A fully consumed successful response is never made cancelled.
      if (!completed) controller.abort();
      controller.signal.removeEventListener('abort', onAbort);
    }
  };
}
