// Shared owner extracted from native-forum/reader.ts. A caller deadline does not
// release a fetch/read/cancel that ignores abort. No detached cancellation work.
export interface OwnedJsonDependencies {
  fetch: typeof fetch;
  origin(): string;
  setTimer(callback: () => void, ms: number): ReturnType<typeof setTimeout>;
  clearTimer(timer: ReturnType<typeof setTimeout>): void;
}
export type JsonLimits = Readonly<{ bytes: number; fragments: number; emptyFragments: number; deadlineMs: number; cleanupMs: number }>;
export class JsonOwnerUnsettled extends Error {
  readonly unsettled = true;
  constructor() { super('Response cleanup is unconfirmed. New reads are stopped until fetch, read and cancellation settle.'); }
}
const defaults: OwnedJsonDependencies = {
  fetch: (...args) => fetch(...args), origin: () => window.location.origin,
  setTimer: (fn, ms) => setTimeout(fn, ms), clearTimer: timer => clearTimeout(timer)
};
type Owner = { active?: object };
type JsonErrors = { unsettled(status: number): Error; failure(message: string, status: number): Error };
const defaultErrors: JsonErrors = { unsettled: () => new JsonOwnerUnsettled(), failure: message => Error(message) };
export function createOwnedJsonReader(limits: JsonLimits, injected: Partial<OwnedJsonDependencies> = {}, errors: JsonErrors = defaultErrors, noAbortStatus?: number) {
  // Anonymous/public/forum callers cannot opt in to credentialed completion.
  return createJsonOwner(limits, injected, errors, noAbortStatus);
}
// Private transport authority: only the route-specific factory below can supply
// an expected account. Neither generic callers nor request input can add headers.
function createJsonOwner(limits: JsonLimits, injected: Partial<OwnedJsonDependencies>, errors: JsonErrors, noAbortStatus?: number, ownership: Owner = {}, expectedAccount?: string) {
  const unsettled = (status = 499) => errors.unsettled(status);
  const deps = { ...defaults, ...injected };
  return async (url: string, options: {
    credentials: 'omit' | 'same-origin'; signal?: AbortSignal; assertCurrent(): void; consume?(bytes: number): void;
  }): Promise<{ status: number; body: unknown }> => {
    options.assertCurrent();
    if (options.credentials !== 'omit' && options.credentials !== 'same-origin') throw Error('Unsupported JSON credentials');
    const origin = new URL(deps.origin()).origin, target = new URL(url, origin);
    if (target.origin !== origin || target.username || target.password || target.hash) throw Error('JSON destination must be same-origin');
    if (ownership.active) throw unsettled();
    const owner = ownership.active = {}, abort = new AbortController();
    let timedOut = false, eof = false, response: Response | undefined, reader: ReadableStreamDefaultReader<Uint8Array> | undefined;
    let fetchWork: Promise<Response> | undefined, readWork: Promise<ReadableStreamReadResult<Uint8Array>> | undefined;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const cancelled = () => errors.failure(timedOut ? 'Response deadline exceeded' : 'Response cancelled', timedOut ? 504 : 499);
    let rejectAbort: (reason: unknown) => void = () => {};
    const aborted = new Promise<never>((_resolve, reject) => { rejectAbort = reject; });
    void aborted.catch(() => {});
    const onAbort = () => rejectAbort(cancelled()), cancel = () => abort.abort();
    abort.signal.addEventListener('abort', onAbort, { once: true });
    options.signal?.addEventListener('abort', cancel, { once: true });
    if (options.signal?.aborted) cancel();
    const check = () => {
      options.assertCurrent();
      if (options.signal?.aborted || abort.signal.aborted) throw cancelled();
      if (new URL(deps.origin()).origin !== origin) throw errors.failure('JSON destination changed', 409);
    };
    const release = () => { reader?.releaseLock(); if (ownership.active === owner) ownership.active = undefined; };
    try {
      check(); timer = deps.setTimer(() => { timedOut = true; abort.abort(); }, limits.deadlineMs);
      fetchWork = Promise.resolve().then(() => {
        check();
        return deps.fetch(target.href, { method: 'GET', credentials: expectedAccount === undefined ? options.credentials : 'include', cache: 'no-store', redirect: 'error', signal: abort.signal, headers: expectedAccount === undefined ? { Accept: 'application/json' } : { Accept: 'application/json', 'X-WeebDex-Expected-Account': expectedAccount } });
      }).then(value => { response = value; return value; });
      response = await Promise.race([fetchWork, aborted]); check();
      // Error bodies are not presentation data. Still own and settle their body.
      if (response.status !== 200) return { status: response.status, body: null };
      if (response.headers.get('content-type')?.split(';', 1)[0].trim().toLowerCase() !== 'application/json') throw errors.failure('Non-JSON response', 502);
      const declared = response.headers.get('content-length');
      if (declared !== null && (!/^(0|[1-9][0-9]*)$/.test(declared) || Number(declared) > limits.bytes)) throw errors.failure('Response byte budget exceeded', 502);
      reader = response.body?.getReader(); if (!reader) throw errors.failure('Missing response body', 502);
      const bytes = new Uint8Array(limits.bytes); let length = 0, fragments = 0, empty = 0;
      for (;;) {
        check(); if (++fragments > limits.fragments) throw errors.failure('Response fragment budget exceeded', 502);
        readWork = Promise.resolve().then(() => { check(); return reader!.read(); }).then(part => { if (part.done) eof = true; return part; });
        const part = await Promise.race([readWork, aborted]); check(); if (part.done) break;
        if (!(part.value instanceof Uint8Array) || part.value.byteLength > limits.bytes - length) throw errors.failure('Response byte budget exceeded', 502);
        if (!part.value.byteLength && ++empty > limits.emptyFragments) throw errors.failure('Response empty-fragment budget exceeded', 502);
        options.consume?.(part.value.byteLength);
        bytes.set(part.value, length); length += part.value.byteLength;
      }
      const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes.subarray(0, length)); check();
      const body: unknown = JSON.parse(text); check(); return { status: response.status, body };
    } finally {
      if (timer !== undefined) deps.clearTimer(timer);
      options.signal?.removeEventListener('abort', cancel); abort.signal.removeEventListener('abort', onAbort);
      if (eof) {
        try { release(); } catch { throw unsettled(); }
      } else {
        const preserveAbort = response?.status === noAbortStatus && noAbortStatus !== undefined;
        if (!preserveAbort) abort.abort();
        const cleanup = (async () => {
          if (fetchWork) await fetchWork.catch(() => undefined);
          if (!reader && response?.body) reader = response.body.getReader();
          if (reader) await Promise.allSettled([Promise.resolve().then(() => reader!.cancel()), ...(readWork ? [readWork] : [])]);
          release();
        })();
        const settled = cleanup.then(() => true, () => false);
        let cleanupTimer: ReturnType<typeof setTimeout> | undefined;
        try {
          if (!await Promise.race([settled, new Promise<boolean>(resolve => { cleanupTimer = deps.setTimer(() => resolve(false), limits.cleanupMs); })])) throw unsettled(timedOut ? 504 : 499);
          if (preserveAbort) check();
        } finally { if (cleanupTimer !== undefined) deps.clearTimer(cleanupTimer); }
      }
    }
  };
}

// This credentialed authority is deliberately not a general-purpose URL reader.
// The endpoint getter must be the build-configured native endpoint, not UI input.
export type NativeCompletionEndpoint = Readonly<{ origin: string; base: string; profile: string }>;
export type NativeCompletionInput = Readonly<{
  mangaId: string; ids: readonly string[]; accountId: string; accountEpoch: number;
  catalogNamespace: string; serviceKey: string | null;
}>;
export type NativeCompletionDependencies = Pick<OwnedJsonDependencies, 'fetch' | 'setTimer' | 'clearTimer'>;
export const NATIVE_COMPLETION_LIMITS = Object.freeze({ bytes: 16 * 1024, fragments: 4096, emptyFragments: 128, deadlineMs: 10000, cleanupMs: 250 });
// Shared by all factory instances, including replacement mounts and endpoints.
// A finite caller timeout is not evidence of fetch/read/cancel settlement.
const nativeCompletionOwnership: Owner = {};
// Continue has a separate envelope policy, not a wider completion/forum limit.
export const NATIVE_CONTINUE_LIMITS = Object.freeze({ bytes: 32 * 1024, fragments: 4096, emptyFragments: 128, deadlineMs: 10000, cleanupMs: 250 });
export type NativeContinueInput = Readonly<Omit<NativeCompletionInput, 'ids'> & { requestId: string; filters: string }>;
export function nativeContinueFilters(input: string): URLSearchParams {
  if (typeof input !== 'string' || input.length > 16 * 1024) throw Error('Native Continue filter budget exceeded');
  const sets = new Map<string, string[]>();
  for (const [raw, value] of new URLSearchParams(input)) {
    const key = raw.replace(/\[\]$/, '');
    if (!['tlang', 'tlangx', 'uploader', 'uploaderx', 'group', 'groupx', 'id', 'manga'].includes(key)) throw Error('Unsupported native Continue filter');
    if (key === 'tlang' || key === 'tlangx') {
      // Syntax/size only. The native server remains the language-enum authority.
      if (value.length < 1 || value.length > 48 || /[^A-Za-z0-9_-]/.test(value)) throw Error('Invalid native Continue language');
    } else nativeId(value);
    const values = sets.get(key) ?? []; if (values.length >= 100) throw Error('Native Continue filter value budget exceeded');
    values.push(value); sets.set(key, values);
  }
  const query = new URLSearchParams();
  for (const key of [...sets.keys()].sort()) for (const value of [...new Set(sets.get(key)!)].sort()) query.append(key, value);
  return query;
}
function nativeId(value: unknown): string {
  if (typeof value !== 'string' || !/^[A-Za-z0-9_:-]{1,128}$/.test(value)) throw Error('Invalid native completion identifier');
  return value;
}
function nativeCompletionEndpoint(endpoint: () => NativeCompletionEndpoint): NativeCompletionEndpoint {
  const { origin, base, profile } = endpoint();
  if (profile !== 'native-aggregate-v1' || typeof origin !== 'string' || origin.length > 2048 || typeof base !== 'string' || base.length > 2048) throw Error('Native completion requires a configured native endpoint');
  const parsed = new URL(origin);
  if (!['http:', 'https:'].includes(parsed.protocol) || parsed.origin !== origin || parsed.username || parsed.password || parsed.search || parsed.hash ||
      !/^(?:\/[A-Za-z0-9._~-]+)*$/.test(base) || new URL(base || '/', origin).pathname !== (base || '/')) throw Error('Invalid configured native completion endpoint');
  return Object.freeze({ origin, base, profile });
}
export function createNativeCompletionJsonReader(endpoint: () => NativeCompletionEndpoint, injected: Partial<NativeCompletionDependencies> = {}) {
  // Pick only the advertised injection points. In particular, a runtime extra
  // origin/headers/credentials property cannot replace configured authority.
  const deps = { fetch: injected.fetch ?? defaults.fetch, setTimer: injected.setTimer ?? defaults.setTimer, clearTimer: injected.clearTimer ?? defaults.clearTimer };
  return async (input: NativeCompletionInput, options: { signal?: AbortSignal; assertCurrent(): void; consume?(bytes: number): void }): Promise<{ status: number; body: unknown }> => {
    options.assertCurrent();
    const approved = nativeCompletionEndpoint(endpoint);
    const mangaId = nativeId(input.mangaId), accountId = nativeId(input.accountId);
    if (!Array.isArray(input.ids) || input.ids.length < 1 || input.ids.length > 50) throw Error('Request 1..50 distinct native chapter IDs');
    const ids = Array.from(input.ids, nativeId);
    if (new Set(ids).size !== ids.length) throw Error('Request 1..50 distinct native chapter IDs');
    const { accountEpoch, catalogNamespace, serviceKey } = input;
    if (!Number.isSafeInteger(accountEpoch) || accountEpoch < 0 || Object.is(accountEpoch, -0)) throw Error('Invalid native completion account epoch');
    if (typeof catalogNamespace !== 'string' || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(catalogNamespace) ||
        serviceKey !== null && (typeof serviceKey !== 'string' || !/^[0-9a-f]{64}$/.test(serviceKey))) throw Error('Invalid native completion catalogue identity');
    const query = new URLSearchParams({ accountEpoch: String(accountEpoch), catalogNamespace, serviceKey: serviceKey === null ? 'null' : serviceKey });
    for (const id of ids) query.append('id', id);
    if (query.toString().length > 24 * 1024) throw Error('Native completion query budget exceeded');
    const url = new URL(approved.base + '/user/history/manga/' + encodeURIComponent(mangaId) + '/completion', approved.origin);
    url.search = query.toString();
    const check = () => {
      options.assertCurrent();
      const now = nativeCompletionEndpoint(endpoint);
      if (now.origin !== approved.origin || now.base !== approved.base || now.profile !== approved.profile) throw Error('Native completion endpoint changed');
    };
    check();
    const read = createJsonOwner(NATIVE_COMPLETION_LIMITS, { ...deps, origin: () => approved.origin }, defaultErrors, undefined, nativeCompletionOwnership, accountId);
    const response = await read(url.href, { credentials: 'omit', signal: options.signal, assertCurrent: check, consume: options.consume });
    check(); if (options.signal?.aborted) throw Error('Native completion cancelled');
    return response;
  };
}

// Only this fixed native route can use the 32 KiB cookie-authorized policy.
// The shared native slot also excludes a still-unsettled completion request.
export function createNativeContinueJsonReader(endpoint: () => NativeCompletionEndpoint, injected: Partial<NativeCompletionDependencies> = {}) {
  const deps = { fetch: injected.fetch ?? defaults.fetch, setTimer: injected.setTimer ?? defaults.setTimer, clearTimer: injected.clearTimer ?? defaults.clearTimer };
  return async (input: NativeContinueInput, options: { signal?: AbortSignal; assertCurrent(): void; consume?(bytes: number): void }): Promise<{ status: number; body: unknown }> => {
    options.assertCurrent(); const approved = nativeCompletionEndpoint(endpoint);
    const mangaId = nativeId(input.mangaId), accountId = nativeId(input.accountId);
    const { accountEpoch, catalogNamespace, serviceKey, requestId } = input;
    if (!Number.isSafeInteger(accountEpoch) || accountEpoch < 0 || Object.is(accountEpoch, -0) ||
        typeof requestId !== 'string' || requestId.length < 1 || requestId.length > 64 || /[^A-Za-z0-9_-]/.test(requestId)) throw Error('Invalid native Continue generation');
    if (typeof catalogNamespace !== 'string' || catalogNamespace.length !== 36 || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(catalogNamespace) ||
        serviceKey !== null && (typeof serviceKey !== 'string' || serviceKey.length !== 64 || !/^[0-9a-f]{64}$/.test(serviceKey))) throw Error('Invalid native Continue catalogue identity');
    const query = nativeContinueFilters(input.filters);
    for (const [key, value] of Object.entries({ accountEpoch: String(accountEpoch), catalogNamespace, serviceKey: serviceKey === null ? 'null' : serviceKey, requestId })) query.set(key, value);
    if (query.toString().length > 16 * 1024) throw Error('Native Continue query budget exceeded');
    const url = new URL(approved.base + '/user/history/manga/' + encodeURIComponent(mangaId) + '/continue', approved.origin); url.search = query.toString();
    const check = () => { options.assertCurrent(); const now = nativeCompletionEndpoint(endpoint); if (now.origin !== approved.origin || now.base !== approved.base || now.profile !== approved.profile) throw Error('Native Continue endpoint changed'); };
    check(); const read = createJsonOwner(NATIVE_CONTINUE_LIMITS, { ...deps, origin: () => approved.origin }, defaultErrors, undefined, nativeCompletionOwnership, accountId);
    const response = await read(url.href, { credentials: 'omit', signal: options.signal, assertCurrent: check, consume: options.consume });
    check(); if (options.signal?.aborted) throw Error('Native Continue cancelled'); return response;
  };
}
