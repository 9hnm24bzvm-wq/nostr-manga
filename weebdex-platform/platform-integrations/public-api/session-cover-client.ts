// Exact native-account, instance-local covers. No signing fallback and
// no automatic replay after an unknown mutation outcome.
import { createNativeContributionScope, type NativeContributionDependencies } from './native-contribution-scope';

import { hash } from '../local-library/protocol/codec.js';
import {
  PublicUploadRequestError,
} from './nostr-upload-request';
import {
  type TitleAccount,
  titleAwaitingProjection,
} from './title-client';
import {
  coverMetadata,
  coverMetadataPatch,
  serializeCoverMultipart,
  type CoverMetadata,
} from './cover-multipart';
import type { UploadImageFile } from './upload-multipart';
import type { MangaView } from './types';
import {
  PublicApiContractError,
  publicUuid,
  record,
  resource,
} from './validation';
export type CoverRecord = {
  id: string;
  mangaId: string;
  version: number;
  locale: string | null;
  volume: string;
  description: string;
  main: boolean;
  sourceEventId: string | null;
  original: {
    sha256: string;
    mime: string;
    bytes: number;
    dimensions: [number, number];
  } | null;
};
export type CoverPage = { data: CoverRecord[]; total: number; limit: 100; offset: number };
export interface SignedCoverController {
  readonly account: TitleAccount;
  assert(): void;
  dispose(): void;
  pending(): boolean;
  read(offset?: number): Promise<CoverPage>;
  upload(file: UploadImageFile, input: CoverMetadata): Promise<CoverRecord>;
  update(cover: CoverRecord, input: CoverMetadata): Promise<CoverRecord>;
  remove(cover: CoverRecord): Promise<void>;
  preview(cover: CoverRecord): Promise<Blob>;
}
// Match the cover-specific native authority rather than the unrelated title
// metadata grant. The server still rechecks current authority under its locks.
export function coverCommandCapabilities(
  account: TitleAccount | undefined,
  title: MangaView,
): { edit: boolean; remove: boolean } {
  if (!account || titleAwaitingProjection(title))
    return { edit: false, remove: false };
  const staff = account.roles.some((role) =>
      ['moderator', 'staff', 'sysop'].includes(role),
    ),
    owns = title.public_api.resource.relationships.some(
      (row) => row.type === 'creator' && row.id === account.id,
    ),
    coverEdit = account.permissions.includes('manga.cover.edit');
  return {
    edit:
      (staff && coverEdit) ||
      (!title.locked &&
        (title.state === 'published'
          ? coverEdit
          : owns && account.permissions.includes('manga.create'))),
    remove: staff && account.permissions.includes('cover.delete'),
  };
}
const encoder = new TextEncoder();
function fail(message: string, status = 502): never {
  throw new PublicApiContractError(message, status);
}
const positive = (value: unknown) => {
  if (
    !Number.isSafeInteger(value) ||
    Number(value) < 1 ||
    Number(value) > 2147483647
  )
    fail('Invalid current cover/title version');
  return Number(value);
};
const strings = (value: unknown) => {
  if (!Array.isArray(value) || value.some((row) => typeof row !== 'string'))
    fail('Invalid current cover account grants');
  return [...value] as string[];
};
export async function createSessionCoverController(nativeAccount:string,assertCurrent:()=>void,title:()=>MangaView,expectedAccount:string,injected:NativeContributionDependencies={}):Promise<SignedCoverController>{
 const access=await createNativeContributionScope(nativeAccount,assertCurrent,injected),deps={fences:access.fences},captured={origin:access.origin},account=access.account,mangaId=publicUuid(title().id);
 if(account.id!==publicUuid(expectedAccount)){access.dispose();fail('Cover account changed',409);}
 let disposed=false,busy=false,reading=false,knownParentVersion:number|undefined;const known=new Map<string,CoverRecord>();
 const check=()=>{access.check();if(disposed||publicUuid(title().id)!==mangaId)fail('Cover editor or parent changed',409);assertCurrent();};
 const request=access.request,mutationRequest=async(input:Parameters<typeof request>[0])=>{check();const fresh=await access.context();check();if(fresh.account.id!==account.id)throw new PublicUploadRequestError('Cover account changed','unsent',409);if(titleAwaitingProjection(title()))throw new PublicUploadRequestError('Accepted original title remains read-only','unsent',409);return request(input);};
 const key=access.key('cover-pending')+':'+access.catalogNamespace+':'+mangaId,pending=()=>deps.fences.get(key)!==null;
  function cover(input: unknown): CoverRecord {
    const row = resource(input, 'cover_art'),
      a = row.attributes,
      n = record(a.nostr, 'cover provenance');
    if (
      n.metadataScope !== 'instance-local' ||
      n.originalEventRewritten !== false ||
      typeof n.main !== 'boolean' ||
      (n.sourceEventId !== null &&
        (typeof n.sourceEventId !== 'string' ||
          !/^[0-9a-f]{64}$/.test(n.sourceEventId)))
    )
      fail('Invalid cover instance/original provenance');
    const parents = row.relationships.filter((r) => r.type === 'manga');
    if (parents.length !== 1 || publicUuid(parents[0].id) !== mangaId)
      fail('Cover changed its exact parent');
    if (
      (a.locale !== null && typeof a.locale !== 'string') ||
      (a.volume !== null && typeof a.volume !== 'string') ||
      (a.description !== null && typeof a.description !== 'string')
    )
      fail('Invalid cover metadata receipt');
    let original: CoverRecord['original'] = null;
    if (n.original !== null) {
      const o = record(n.original);
      if (
        typeof o.sha256 !== 'string' ||
        !/^[0-9a-f]{64}$/.test(o.sha256) ||
        !['image/png', 'image/jpeg', 'image/webp'].includes(String(o.mime)) ||
        !Number.isSafeInteger(o.bytes) ||
        Number(o.bytes) < 1 ||
        Number(o.bytes) > 32 * 1024 * 1024 ||
        !Array.isArray(o.dimensions) ||
        o.dimensions.length !== 2 ||
        o.dimensions.some((v) => !Number.isSafeInteger(v) || Number(v) < 1)
      )
        fail('Invalid actual original cover descriptor');
      original = {
        sha256: o.sha256,
        mime: String(o.mime),
        bytes: Number(o.bytes),
        dimensions: o.dimensions as [number, number],
      };
    }
    return {
      id: publicUuid(row.id),
      mangaId,
      version: positive(a.version),
      locale: a.locale as string | null,
      volume: (a.volume as string) ?? '',
      description: (a.description as string) ?? '',
      main: n.main,
      sourceEventId: n.sourceEventId as string | null,
      original,
    };
  }
  const read = async (offset = 0): Promise<CoverPage> => {
    check();
    if (busy || reading) fail('A cover operation is in progress', 409);
    if (!Number.isSafeInteger(offset) || offset < 0)
      fail('Cover offset must be a non-negative exact integer', 422);
    const parentVersion = positive(title().version);
    // A failed or delayed replacement page must never leave editable stale rows.
    known.clear();
    knownParentVersion = undefined;
    reading = true;
    try {
      // The retained layout prefixes URLSearchParams.toString() with '?'.
      const query = new URLSearchParams({limit: '100', offset: String(offset), expectedAccountId: account.id}).toString().replace(/^\?/, '');
      const response = await request({
        url: `${captured.origin}/api/v1/account/manga/${mangaId}/covers?${query}`,
        method: 'GET',
      });
      check();
      if (positive(title().version) !== parentVersion)
        fail('Title changed while reading covers; reload this page', 409);
      if (response.status !== 200)
        fail(`Covers could not be loaded (HTTP ${response.status})`, response.status);
      const body = record(response.body);
      if (
        body.result !== 'ok' || body.response !== 'collection' ||
        !Array.isArray(body.data) || body.limit !== 100 || body.offset !== offset ||
        !Number.isSafeInteger(body.total) || Number(body.total) < 0 ||
        body.data.length !== Math.min(100, Math.max(0, Number(body.total) - offset))
      ) fail('Invalid bounded cover collection');
      const rows = body.data.map(cover);
      if (new Set(rows.map((row) => row.id)).size !== rows.length)
        fail('Duplicate cover receipt');
      for (const row of rows) known.set(row.id, row);
      knownParentVersion = parentVersion;
      return { data: rows, total: Number(body.total), limit: 100, offset };
    } finally {
      reading = false;
    }
  };
  const current = (row: CoverRecord) => {
    check();
    const saved = known.get(publicUuid(row.id));
    if (reading || knownParentVersion !== positive(title().version) || !saved || saved.version !== row.version || row.mangaId !== mangaId)
      fail('Read the exact current cover before editing', 409);
    return saved;
  };
  const write = async (
    action: 'upload' | 'update' | 'remove',
    row: CoverRecord | undefined,
    input?: CoverMetadata,
    file?: UploadImageFile,
  ) => {
    check();
    if (busy || reading || pending() || titleAwaitingProjection(title()))
      fail(
        'This cover/title command is unconfirmed or read-only. Nothing was replayed.',
        409,
      );
    if (row) row = current(row);
    const parentVersion = positive(title().version),
      previous = row,
      changed = action === 'update' ? coverMetadataPatch(input, row!) : undefined,
      desired = action === 'remove' ? undefined : action === 'update' ? {locale:row!.locale,volume:row!.volume,description:row!.description,main:row!.main,...changed} : coverMetadata(input);
    busy = true;
    let installed = false;
    try {
      let body: Uint8Array, contentType: string, fileHash: string | undefined;
      if (action === 'upload') {
        const multipart = await serializeCoverMultipart(
          file!,
          parentVersion,
          desired! as CoverMetadata,
          account.id,
        );
        check();
        body = multipart.body;
        contentType = multipart.contentType;
        fileHash = hash(multipart.fileBytes);
      } else {
        const payload: Record<string, unknown> = {
          version: row!.version,
          expectedAccountId: account.id,
        };
        if (action !== 'remove') {
          for (const field of [
            'locale',
            'volume',
            'description',
            'main',
          ] as const)
            if (Object.hasOwn(changed!, field))
              payload[field] = desired![field];
          if (Object.keys(payload).length === 2)
            fail('No cover metadata changes to save', 422);
        }
        body = encoder.encode(JSON.stringify(payload));
        contentType = 'application/json';
      }
      const marker = JSON.stringify({
        action,
        target: row?.id ?? mangaId,
        version: row?.version ?? parentVersion,
        fileHash,
      });
      deps.fences.set(key, marker);
      installed = true;
      if (deps.fences.get(key) !== marker)
        fail('Cover recovery fence was not persisted', 409);
      const response = await mutationRequest({
        url: `${captured.origin}/api/v1/account/cover/${row?.id ?? mangaId}?expectedAccountId=${account.id}`,
        method:
          action === 'upload' ? 'POST' : action === 'update' ? 'PUT' : 'DELETE',
        body,
        contentType,
        requestLimit: body.length,
        timeoutMs: 60000,
      });
      check();
      if (response.status >= 400 && response.status < 500) {
        deps.fences.delete(key);
        installed = false;
        fail(
          `Cover command refused (HTTP ${response.status})`,
          response.status,
        );
      }
      if (action === 'remove') {
        if (response.status !== 204 || response.body !== undefined)
          fail('Cover removal outcome is unconfirmed');
        known.delete(row!.id);
        deps.fences.delete(key);
        installed = false;
        return;
      }
      if (response.status !== (action === 'upload' ? 201 : 200))
        fail(`Cover command outcome is unconfirmed (HTTP ${response.status})`);
      const envelope = record(response.body);
      if (envelope.result !== 'ok' || envelope.response !== 'entity')
        fail('The saved cover could not be confirmed');
      const next = cover(envelope.data);
      if (
        next.version !== (previous ? previous.version + 1 : 1) ||
        (previous && next.id !== previous.id) ||
        (action === 'upload' &&
          (!next.original ||
            next.original.sha256 !== fileHash ||
            next.original.bytes !== file!.size ||
            next.original.mime !== file!.type)) ||
        (previous &&
          (JSON.stringify(next.original) !==
            JSON.stringify(previous.original) ||
            next.sourceEventId !== previous.sourceEventId)) ||
        (action === 'upload' && known.has(next.id))
      )
        fail('Cover receipt changed its exact original/version');
      if (
        desired &&
        (['locale', 'volume', 'description', 'main'] as const).some(
          (field) => next[field] !== desired[field],
        )
      )
        fail('Cover receipt did not acknowledge the requested metadata');
      deps.fences.delete(key);
      installed = false;
      known.clear();
      known.set(next.id, next);
      knownParentVersion = parentVersion;
      return next;
    } catch (error) {
      if (
        installed &&
        error instanceof PublicUploadRequestError &&
        error.outcome === 'unsent'
      )
        deps.fences.delete(key);
      throw error;
    } finally {
      busy = false;
    }
  };
  return {
    account,
    assert: check,
    dispose: () => {
      disposed = true; access.dispose();
      known.clear();
    },
    pending,
    read,
    upload: async (file, input) =>
      (await write('upload', undefined, input, file))!,
    update: async (row, input) => (await write('update', row, input))!,
    remove: async (row) => {
      await write('remove', row);
    },
    preview: async (row) => {
      row = current(row);
      if (!row.original)
        fail('This source cover has no instance-local original preview', 404);
      const expected = row.original;
      const response = await request({
        url: `${captured.origin}/api/v1/account/covers/${row.id}/original?expectedAccountId=${account.id}`,
        method: 'GET',
        responseType: 'image',
        responseLimit: expected.bytes,
      });
      check();
      if (
        response.status !== 200 ||
        !(response.body instanceof Uint8Array) ||
        response.body.length !== expected.bytes ||
        response.headers['content-type']?.split(';', 1)[0] !== expected.mime ||
        hash(response.body) !== expected.sha256 ||
        response.headers['cache-control'] !== 'private, no-store' ||
        !response.headers.vary
          ?.split(',')
          .some((value) => value.trim().toLowerCase() === 'authorization') ||
        response.headers['x-content-type-options']?.toLowerCase() !== 'nosniff'
      )
        fail('Cover preview does not match the exact acknowledged original');
      return new Blob([response.body as Uint8Array<ArrayBuffer>], {
        type: expected.mime,
      });
    },
  };
}
