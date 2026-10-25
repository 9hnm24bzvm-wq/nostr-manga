import { publicQuery } from '../public-api/query';
import { createOwnedJsonReader, type OwnedJsonDependencies } from '../public-api/owned-json';
import { reference, selectorPage, type Destination, type Profile, type Request, type SelectorPage } from './contract';
export const READER_LIMITS = Object.freeze({ bytes: 256 * 1024, fragments: 16384, emptyFragments: 128, deadlineMs: 10000, cleanupMs: 250, requests: 8, rows: 400, actionBytes: 1024 * 1024, page: 50, recentKeys: 100 });
const controls = new Set(['operation', 'limit', 'direction', 'current', 'groupChapter', 'after']);
export function readerFilters(profile: Profile, input: URLSearchParams): string {
  // The public chapter translator preserves its own language/UUID vocabulary.
  // Remove only its generated pagination default, not a caller-supplied one.
  if ([...input.keys()].some(key => controls.has(key) || ['page', 'offset', 'sort', 'order', 'manga', 'includes', 'includes[]'].includes(key) || key.startsWith('order['))) throw Error('Unsupported reader filter control');
  const query = profile === 'public-retained-v1' ? publicQuery('chapter', input) : new URLSearchParams(input);
  if (profile === 'public-retained-v1') query.delete('limit');
  else if ([...query.keys()].some(key => !['tlang', 'tlangx', 'uploader', 'uploaderx', 'group', 'groupx', 'id'].includes(key.replace(/\[\]$/, '')))) throw Error('Unsupported native reader filter');
  if (query.toString().length > 12000) throw Error('Reader filters exceed their query budget');
  return query.toString();
}
export class ReaderBudget {
  readonly abort = new AbortController(); readonly signal = this.abort.signal;
  requests = 0; rows = 0; bytes = 0;
  private readonly timer: ReturnType<typeof setTimeout>;
  private handedToRouter = false;
  private readonly expires = Date.now() + READER_LIMITS.deadlineMs;
  constructor(readonly assertCurrent: () => void) { this.timer = setTimeout(() => this.cancel(), READER_LIMITS.deadlineMs); }
  check = () => { this.assertCurrent(); if (this.signal.aborted || !this.handedToRouter && Date.now() >= this.expires) throw Error('Reader action expired or cancelled'); };
  fits(requests: number, rows: number, bytes: number) {
    this.check(); if (![requests, rows, bytes].every(value => Number.isSafeInteger(value) && value >= 0)) throw Error('Invalid reader budget reservation');
    return this.requests + requests <= READER_LIMITS.requests && this.rows + rows <= READER_LIMITS.rows && this.bytes + bytes <= READER_LIMITS.actionBytes;
  }
  request() { this.check(); if (++this.requests > READER_LIMITS.requests) throw Error('Reader request budget exceeded'); }
  consume = (bytes: number) => { this.check(); if (!Number.isSafeInteger(bytes) || bytes < 0 || (this.bytes += bytes) > READER_LIMITS.actionBytes) throw Error('Reader action byte budget exceeded'); };
  accept(rows: number) { this.check(); if ((this.rows += rows) > READER_LIMITS.rows) throw Error('Reader action row budget exceeded'); }
  handoffToRouter() { this.check(); this.handedToRouter = true; this.close(); }
  cancel() { this.abort.abort(); }
  close() { clearTimeout(this.timer); }
}
export type ReaderRead = (destination: Destination, request: Request, budget: ReaderBudget) => Promise<SelectorPage>;
export type ReaderEndpoint = Readonly<{ origin: string; base: string; profile: Profile }>;
export type ReaderIdentity = Readonly<{ origin: string; catalogNamespace: string; serviceKey: string | null }>;
export function createReaderClient(deps: Partial<OwnedJsonDependencies> = {}, endpoint?: () => ReaderEndpoint) {
  // One slot survives disposal and remount, including unsettled cleanup.
  const read = createOwnedJsonReader(READER_LIMITS, deps);
  const readSelector: ReaderRead = async (destination, request, budget) => {
    budget.request(); reference(destination.mangaId, destination.profile);
    if (!Number.isSafeInteger(request.limit) || request.limit < 1 || request.limit > 100) throw Error('Invalid reader page size');
    const approved = endpoint?.();
    const expectedBase = approved?.base ?? (destination.profile === 'public-retained-v1' ? '/api/v1' : '/api');
    if (destination.base !== expectedBase || approved && (destination.origin !== approved.origin || destination.profile !== approved.profile)) throw Error('Reader profile and pinned endpoint disagree');
    const query = new URLSearchParams(destination.filters);
    for (const [key, value] of Object.entries(request)) if (value !== undefined) {
      if (!controls.has(key) || query.has(key)) throw Error('Reader controls overlap filters');
      query.set(key, String(value));
    }
    if (query.toString().length > 16384) throw Error('Reader query exceeds 16 KiB');
    const url = new URL(destination.base + '/manga/' + encodeURIComponent(destination.mangaId) + '/reader', destination.origin);
    url.search = query.toString();
    const response = await read(url.href, { credentials: destination.profile === 'public-retained-v1' ? 'omit' : 'same-origin', signal: budget.signal, assertCurrent: budget.check, consume: budget.consume });
    budget.check(); if (response.status !== 200) throw Error('Reader unavailable (HTTP ' + response.status + '). No boundary was inferred.');
    const page = selectorPage(response.body, destination, request); budget.accept(page.data.length + Number(!!page.current) + Number(!!page.groupAnchor) + Number(!!page.winner) + Number(!!page.next) + Number(!!page.previous));
    return page;
  };
  const verifyIdentity = async (budget: ReaderBudget): Promise<ReaderIdentity> => {
    const approved = endpoint?.(); if (!approved) throw Error('An explicitly pinned reader endpoint is required for identity discovery');
    budget.request(); let bytes = 0;
    const base = approved.profile === 'public-retained-v1' ? '/api' : approved.base;
    const response = await read(new URL(base + '/protocol/capabilities', approved.origin).href, { credentials: 'omit', signal: budget.signal, assertCurrent: budget.check, consume: size => { budget.consume(size); if ((bytes += size) > 65536) throw Error('Catalogue identity byte budget exceeded'); } });
    budget.check(); if (response.status !== 200) throw Error('Catalogue identity could not be verified (HTTP ' + response.status + ')');
    const body = response.body;
    if (typeof body !== 'object' || body === null || Array.isArray(body) || Object.getPrototypeOf(body) !== Object.prototype || !('catalog_namespace' in body) || !('service_key' in body)) throw Error('Invalid catalogue identity response');
    const namespace = body.catalog_namespace, serviceKey = body.service_key;
    if (typeof namespace !== 'string' || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(namespace) || serviceKey !== null && (typeof serviceKey !== 'string' || !/^[0-9a-f]{64}$/.test(serviceKey))) throw Error('Invalid catalogue namespace or service key');
    return Object.freeze({ origin: approved.origin, catalogNamespace: namespace, serviceKey });
  };
  return { read: readSelector, verifyIdentity };
}
export function createReaderRead(deps: Partial<OwnedJsonDependencies> = {}): ReaderRead { return createReaderClient(deps).read; }
export const readReader = createReaderRead();
