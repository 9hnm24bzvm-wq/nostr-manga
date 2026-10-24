// Minimal selector DTOs only. No Chapter casts, aggregate compatibility object,
// page arrays, relationship dictionaries or catalogue-wide retained IDs.
export type Profile = 'native-aggregate-v1' | 'public-retained-v1';
export type Operation = 'groups' | 'editions' | 'first' | 'last' | 'around' | 'next' | 'previous' | 'resolve-group';
export type CatalogIdentity = Readonly<{ catalogNamespace: string; serviceKey: string | null }>;
export class ReaderIdentityChanged extends Error {}
export type SelectorState = 'ready' | 'boundary' | 'excluded-current' | 'stale-anchor';
export type Row = Readonly<{
  id: string; volume: string | null; chapter: string | null; language: string;
  publishedAt: string; version: number; groupIds: readonly string[]; sharedGroupScore: number; witness: string;
}>;
export type SelectorPage = Readonly<{
  operation: Operation; scope: string; catalogIdentity: CatalogIdentity; state: SelectorState; data: readonly Row[];
  nextCursor: string | null; current?: Row; groupAnchor?: Row; winner?: Row | null;
  next?: Row | null; previous?: Row | null; hasMultipleEditions?: boolean;
}>;
export type Request = Readonly<{ operation: Operation; limit: number; direction?: 'newest' | 'oldest'; current?: string; groupChapter?: string; after?: string }>;
export type Destination = Readonly<{ origin: string; base: string; profile: Profile; mangaId: string; filters: string; identity?: CatalogIdentity }>;
const digest = /^[0-9a-f]{64}$/;
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
function fail(): never { throw Error('Invalid bounded reader response. Refresh before another action.'); }
function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value) && Object.getPrototypeOf(value) === Object.prototype;
}
function object(value: unknown): Record<string, unknown> { if (!isObject(value)) fail(); return value; }
function string(value: unknown, max: number, empty = false): string {
  if (typeof value !== 'string' || value.length > max || (!empty && !value.length) || /\u0000/.test(value)) fail();
  return value;
}
function number(value: unknown, max: number): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0 || value > max) fail();
  return value;
}
export function reference(value: unknown, profile: Profile): string {
  const id = string(value, 128);
  if (!(profile === 'public-retained-v1' ? uuid : /^[a-zA-Z0-9_:-]+$/).test(id)) fail();
  return id;
}
const nullable = (value: unknown) => { if (value === null) return null; const text = string(value, 64, true); if ([...text].length > 32) fail(); return text; };
const rowKeys = new Set(['id', 'volume', 'chapter', 'language', 'publishedAt', 'version', 'groupIds', 'sharedGroupScore', 'witness']);
export function chapterRow(value: unknown, profile: Profile): Row {
  const row = object(value);
  if (Object.keys(row).length !== rowKeys.size || Object.keys(row).some(key => !rowKeys.has(key))) fail();
  if (!Array.isArray(row.groupIds) || row.groupIds.length > 100) fail();
  const groupIds = row.groupIds.map(id => reference(id, profile));
  if (new Set(groupIds).size !== groupIds.length) fail();
  const publishedAt = string(row.publishedAt, 64), witness = string(row.witness, 64);
  if (!/^\d{4,6}-\d\d-\d\d \d\d:\d\d:\d\d(?:\.\d{1,6})?\+00$/.test(publishedAt) || !digest.test(witness)) fail();
  const language = string(row.language, 48); if (new TextEncoder().encode(language).byteLength > 48) fail();
  const version = number(row.version, 2147483647); if (version < 1) fail();
  return Object.freeze({ id: reference(row.id, profile), volume: nullable(row.volume), chapter: nullable(row.chapter), language,
    publishedAt, version, groupIds: Object.freeze(groupIds), sharedGroupScore: number(row.sharedGroupScore, 100), witness });
}
export const sameGroup = (a: Row, b: Row) => a.volume === b.volume && a.chapter === b.chapter;
export const sameRow = (a: Row | null | undefined, b: Row | null | undefined) => !a || !b ? a === b :
  a.id === b.id && a.witness === b.witness && sameGroup(a, b) && a.version === b.version && a.language === b.language && a.publishedAt === b.publishedAt &&
  a.groupIds.length === b.groupIds.length && a.groupIds.every((id, i) => id === b.groupIds[i]);
export function selectorPage(value: unknown, destination: Destination, request: Request): SelectorPage {
  const body = object(value), profile = destination.profile;
  const identity = object(body.catalogIdentity);
  if (Object.keys(identity).length !== 2 || typeof identity.catalogNamespace !== 'string' || !uuid.test(identity.catalogNamespace) || identity.serviceKey !== null && (typeof identity.serviceKey !== 'string' || !digest.test(identity.serviceKey))) fail();
  if (!destination.identity || identity.catalogNamespace !== destination.identity.catalogNamespace || identity.serviceKey !== destination.identity.serviceKey) throw new ReaderIdentityChanged('The catalogue namespace or service identity changed. Reload this page before navigating; saved history was not reinterpreted.');
  const catalogIdentity = Object.freeze({ catalogNamespace: identity.catalogNamespace, serviceKey: identity.serviceKey });
  if (body.version !== 1 || body.mangaId !== destination.mangaId || body.operation !== request.operation || body.orderingProfile !== profile ||
      body.consistency !== 'live-keyset-v1' || body.crossRequestSnapshot !== false || body.history !== 'device-local-only' || body.continueProof !== 'observed-only' ||
      typeof body.scope !== 'string' || !digest.test(body.scope) || body.limit !== request.limit || !Array.isArray(body.data)) fail();
  let state: SelectorState;
  if (body.state === 'ready' || body.state === 'boundary' || body.state === 'excluded-current' || body.state === 'stale-anchor') state = body.state; else fail();
  if (body.restartRequired !== (state === 'stale-anchor')) fail();
  const paged = request.operation === 'groups' || request.operation === 'editions';
  if (body.data.length > (paged ? request.limit : 1)) fail();
  const data = body.data.map(row => chapterRow(row, profile));
  if (new Set(data.map(row => row.id)).size !== data.length || request.operation === 'groups' && new Set(data.map(row => JSON.stringify([row.volume, row.chapter]))).size !== data.length) fail();
  const nextCursor = body.nextCursor === null ? null : string(body.nextCursor, 4096);
  if (nextCursor && (!paged || !data.length || !/^[A-Za-z0-9_-]+$/.test(nextCursor))) fail();
  if (state !== 'ready' && data.length || state === 'ready' && !data.length || state !== 'ready' && nextCursor !== null) fail();
  if (state === 'excluded-current' || state === 'stale-anchor') return Object.freeze({ operation: request.operation, scope: body.scope, catalogIdentity, state, data: Object.freeze([]), nextCursor: null });
  const current = body.current === undefined ? undefined : chapterRow(body.current, profile);
  if (request.current ? current?.id !== request.current : current !== undefined) fail();
  const result: { operation: Operation; scope: string; catalogIdentity: CatalogIdentity; state: SelectorState; data: readonly Row[]; nextCursor: string | null; current?: Row; groupAnchor?: Row; winner?: Row | null; next?: Row | null; previous?: Row | null; hasMultipleEditions?: boolean } =
    { operation: request.operation, scope: body.scope, catalogIdentity, state, data: Object.freeze(data), nextCursor, ...(current ? { current } : {}) };
  if (request.operation === 'editions' || request.operation === 'resolve-group') {
    const groupAnchor = chapterRow(body.groupAnchor, profile), winner = body.winner === null ? null : chapterRow(body.winner, profile);
    if (groupAnchor.id !== (request.groupChapter ?? request.current) || data.some(row => !sameGroup(row, groupAnchor)) || winner && !sameGroup(winner, groupAnchor) || typeof body.hasMultipleEditions !== 'boolean') fail();
    if (request.operation === 'resolve-group' && !sameRow(data[0] ?? null, winner)) fail();
    Object.assign(result, { groupAnchor, winner, hasMultipleEditions: body.hasMultipleEditions });
  }
  if ((request.operation === 'first' || request.operation === 'last') && data.length) {
    if (typeof body.hasMultipleEditions !== 'boolean') fail();
    result.hasMultipleEditions = body.hasMultipleEditions;
  }
  if (request.operation === 'around') {
    if (!current || !data[0] || !sameGroup(data[0], current)) fail();
    const next = body.next === null ? null : chapterRow(body.next, profile), previous = body.previous === null ? null : chapterRow(body.previous, profile);
    if (next && sameGroup(next, current) || previous && sameGroup(previous, current) || next && previous && sameGroup(next, previous)) fail();
    Object.assign(result, { next, previous });
  }
  return Object.freeze(result);
}
