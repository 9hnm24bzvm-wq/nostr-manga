// Read-only native moderation history. No write fence access, native-cookie
// fallback, target expansion, or inference about an uncertain submission.
import { createSignedAccountScope } from './signed-account-scope';
import { publicUuid, record, PublicApiContractError } from './validation';
import type { TitleDependencies } from './title-client';
export const nativeReportCategories = ['thread', 'reply'] as const;
export const nativeReportStatuses = ['waiting', 'accepted', 'refused', 'autoresolved'] as const;
export const nativeReportPageSize = 5, nativeReportMaxPage = 2001;
export interface NativeInspectedReport {
 id: string; category: typeof nativeReportCategories[number]; objectId: string;
 status: typeof nativeReportStatuses[number]; createdAt: string; details: string;
 reason: {id: string; reason: string; version: number; detailsRequired: boolean};
}
export interface NativeReportSnapshot {data: NativeInspectedReport[]; total: number; page: number; limit: number; offset: number; detail: boolean;}
function fail(message: string, status = 502): never { throw new PublicApiContractError(message, status); }
function nativeId(value: unknown): string {
 if (typeof value !== 'string' || !/^[A-Za-z0-9_:-]{1,128}$/.test(value)) fail('Invalid native report identifier', 422);
 return value;
}
function keys(value: unknown, expected: string[], label: string): Record<string, unknown> {
 const row = record(value, label);
 if (Object.keys(row).length !== expected.length || expected.some(key => !Object.hasOwn(row, key))) fail('Invalid native report response');
 return row;
}
export function nativeReportInspectionQuery(input: URLSearchParams, accountId?: string) {
 const allowed = ['page', 'category', 'status', 'objectId', 'reasonId', 'reportId'];
 for (const key of input.keys()) if (!allowed.includes(key) || input.getAll(key).length !== 1) fail('Unsupported or repeated native report filter', 422);
 const reportId = input.get('reportId');
 if (reportId !== null && input.size !== 1) fail('A report detail cannot also select a list filter', 422);
 const page = input.get('page') ?? '1';
 if (!/^[1-9]\d*$/.test(page) || !Number.isSafeInteger(Number(page)) || Number(page) > nativeReportMaxPage) fail('Invalid bounded native report page', 422);
 const category = input.get('category'), status = input.get('status');
 if (category !== null && !nativeReportCategories.includes(category as NativeInspectedReport['category']) || status !== null && !nativeReportStatuses.includes(status as NativeInspectedReport['status'])) fail('Invalid native report category or status', 422);
 const query = new URLSearchParams();
 if (accountId !== undefined) query.set('expectedAccountId', publicUuid(accountId));
 if (reportId !== null) return {path: '/nostr/account/reports/native/' + encodeURIComponent(nativeId(reportId)), query, reportId, page: 1};
 query.set('page', page);
 for (const key of ['category', 'status', 'objectId', 'reasonId']) {
  const value = input.get(key);
  if (value !== null) query.set(key, key === 'objectId' || key === 'reasonId' ? nativeId(value) : value);
 }
 return {path: '/nostr/account/reports/native', query, reportId: null, page: Number(page)};
}
export function nativeReportInspectionHref(category?: 'thread' | 'reply', objectId?: string): string {
 if (category === undefined && objectId === undefined) return '/reports/signed/native';
 if (!nativeReportCategories.includes(category!) || objectId === undefined) fail('Choose a native thread or reply', 422);
 return '/reports/signed/native?' + new URLSearchParams({category: category!, objectId: nativeId(objectId)}).toString().replace(/^\?/, '');
}
export function nativeReportDetailHref(id: string): string { return '/reports/signed/native?' + new URLSearchParams({reportId: nativeId(id)}).toString().replace(/^\?/, ''); }
function decodeRow(input: unknown, selection: ReturnType<typeof nativeReportInspectionQuery>): NativeInspectedReport {
 const row = keys(input, ['id', 'category', 'objectId', 'status', 'createdAt', 'details', 'reason'], 'native report');
 const reason = keys(row.reason, ['id', 'reason', 'version', 'detailsRequired'], 'native report reason');
 const id = nativeId(row.id), objectId = nativeId(row.objectId), reasonId = nativeId(reason.id);
 if (!nativeReportCategories.includes(row.category as NativeInspectedReport['category']) || !nativeReportStatuses.includes(row.status as NativeInspectedReport['status'])) fail('Invalid native report category or status');
 if (typeof row.details !== 'string' || row.details.includes('\0') || [...row.details].length > 20000 || new TextEncoder().encode(row.details).byteLength > 20000) fail('Invalid native report details');
 if (typeof reason.reason !== 'string' || !reason.reason || reason.reason.includes('\0') || new TextEncoder().encode(reason.reason).byteLength > 8000 || !Number.isSafeInteger(reason.version) || Number(reason.version) < 1 || typeof reason.detailsRequired !== 'boolean') fail('Invalid current native report reason');
 if (typeof row.createdAt !== 'string' || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{6}Z$/.test(row.createdAt) || !Number.isFinite(Date.parse(row.createdAt)) || new Date(row.createdAt.slice(0, 23) + 'Z').toISOString() !== row.createdAt.slice(0, 23) + 'Z') fail('Invalid native report creation time');
 if (selection.reportId !== null && selection.reportId !== id) fail('Native report detail escaped the requested identity');
 for (const [key, value] of [['category', row.category], ['status', row.status], ['objectId', objectId], ['reasonId', reasonId]]) if (selection.query.has(key as string) && selection.query.get(key as string) !== value) fail('Native report page escaped the requested filter');
 return {id, objectId, category: row.category as NativeInspectedReport['category'], status: row.status as NativeInspectedReport['status'], createdAt: row.createdAt, details: row.details, reason: {id: reasonId, reason: reason.reason, version: Number(reason.version), detailsRequired: reason.detailsRequired}};
}
function decode(input: unknown, selection: ReturnType<typeof nativeReportInspectionQuery>, accountId: string): NativeReportSnapshot {
 const detail = selection.reportId !== null;
 const body = keys(input, detail ? ['result', 'response', 'nostr', 'data'] : ['result', 'response', 'nostr', 'data', 'total', 'limit', 'page', 'offset'], 'native report response');
 const meta = keys(body.nostr, ['scope', 'source', 'accountId', 'nativeSessionAuthority'], 'native report authority');
 if (body.result !== 'ok' || body.response !== (detail ? 'entity' : 'collection') || meta.scope !== 'instance-account' || meta.source !== 'persisted-native-personal-reports' || publicUuid(meta.accountId) !== accountId || meta.nativeSessionAuthority !== false) fail('Native report response escaped the captured account');
 if (detail) return {data: [decodeRow(body.data, selection)], total: 1, page: 1, limit: 1, offset: 0, detail};
 const offset = (selection.page - 1) * nativeReportPageSize;
 if (!Array.isArray(body.data) || body.page !== selection.page || body.limit !== nativeReportPageSize || body.offset !== offset || !Number.isSafeInteger(body.total) || Number(body.total) < 0 || body.data.length !== Math.min(nativeReportPageSize, Math.max(0, Number(body.total) - offset))) fail('Invalid bounded native report page');
 const data = body.data.map(row => decodeRow(row, selection));
 if (new Set(data.map(row => row.id)).size !== data.length || data.some((row, index) => index > 0 && (row.createdAt > data[index - 1].createdAt || row.createdAt === data[index - 1].createdAt && row.id > data[index - 1].id))) fail('Invalid native report ordering');
 return {data, total: Number(body.total), page: selection.page, limit: nativeReportPageSize, offset, detail};
}
// UI never echoes an exception from a signer, transport, or response body.
export function nativeReportInspectionError(error: unknown): string {
 if (error instanceof PublicApiContractError) {
  if (error.status === 422) return 'The native report link or filters are invalid.';
  if (error.status === 401 || error.status === 403) return 'Signed access to this account is unavailable. No browser-login fallback was used.';
  if (error.status === 404) return 'This report is not available to the selected signed account.';
  if (error.status === 409) return 'The captured account or request changed. Reopen inspection before reading again.';
 }
 return 'Native reports could not be verified. No private snapshot was retained; reopen inspection to try a new read.';
}
export async function createNativeReportReader(assertCurrent: () => void, injected: Partial<TitleDependencies> = {}, signal?: AbortSignal) {
 const scope = await createSignedAccountScope(assertCurrent, injected, signal);
 let busy = false;
 async function read(input: URLSearchParams): Promise<NativeReportSnapshot> {
  scope.check();
  if (busy) fail('A native report inspection is already running', 409);
  const selection = nativeReportInspectionQuery(input, scope.account.id);
  busy = true;
  try {
   const response = await scope.send(selection.path + '?' + selection.query.toString().replace(/^\?/, ''));
   if (response.status !== 200) fail('Native report inspection refused', response.status);
   // Empty pages are private results too. Never install one after rebinding.
   await scope.verify();
   const result = decode(response.body, selection, scope.account.id);
   scope.check();
   return result;
  } finally { busy = false; }
 }
 return {account: scope.account, check: scope.check, dispose: scope.dispose, read};
}
