// Exact signed, instance-local covers. No native cookie command fallback and
// no automatic replay after an unknown mutation outcome.
import { capabilities, selectedSigner } from '../local-library/publication';
import { proveService, origin } from '../local-library/protocol/nostr.js';
import { hash } from '../local-library/protocol/codec.js';
import {
  createNostrUploadRequest,
  PublicUploadRequestError,
} from './nostr-upload-request';
import {
  type TitleDependencies,
  type TitleAccount,
  titleAwaitingProjection,
} from './title-client';
import {
  coverMetadata,
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
export interface SignedCoverController {
  readonly account: TitleAccount;
  assert(): void;
  dispose(): void;
  pending(): boolean;
  read(): Promise<{ data: CoverRecord[]; total: number }>;
  upload(file: UploadImageFile, input: CoverMetadata): Promise<CoverRecord>;
  update(cover: CoverRecord, input: CoverMetadata): Promise<CoverRecord>;
  remove(cover: CoverRecord): Promise<void>;
  preview(cover: CoverRecord): Promise<Blob>;
}
const defaults: TitleDependencies = {
  capabilities,
  selectedSigner,
  proveService,
  publicOrigin: () => window.location.origin,
  createRequest: createNostrUploadRequest,
  fences: {
    get: (key) => window.sessionStorage.getItem(key),
    set: (key, value) => window.sessionStorage.setItem(key, value),
    delete: (key) => window.sessionStorage.removeItem(key),
  },
};
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
function scope(
  policy: Awaited<ReturnType<typeof capabilities>>,
  publicOrigin: string,
) {
  if (
    !policy.available ||
    !policy.schema_ready ||
    !policy.origin ||
    !policy.service_key ||
    !/^[0-9a-f]{64}$/.test(policy.service_key) ||
    !policy.catalog_namespace
  )
    fail('Signed cover service is unavailable', 503);
  return {
    origin: origin(publicOrigin),
    proofOrigin: origin(policy.origin),
    serviceKey: policy.service_key,
    catalogNamespace: publicUuid(policy.catalog_namespace),
  };
}
export async function createSignedCoverController(
  assertCurrent: () => void,
  title: () => MangaView,
  expectedAccount: string,
  injected: Partial<TitleDependencies> = {},
): Promise<SignedCoverController> {
  const deps = { ...defaults, ...injected },
    identity = deps.selectedSigner();
  identity.check();
  assertCurrent();
  const captured = scope(await deps.capabilities(), deps.publicOrigin());
  identity.check();
  assertCurrent();
  const proof = await deps.proveService(
    captured.proofOrigin,
    captured.serviceKey,
  );
  identity.check();
  assertCurrent();
  if (
    proof.origin !== captured.proofOrigin ||
    proof.serviceKey !== captured.serviceKey
  )
    fail('Cover service proof changed');
  const mangaId = publicUuid(title().id);
  expectedAccount = publicUuid(expectedAccount);
  let disposed = false,
    busy = false;
  const known = new Map<string, CoverRecord>();
  const check = () => {
    if (disposed) fail('Cover editor is closed', 409);
    identity.check();
    assertCurrent();
    if (publicUuid(title().id) !== mangaId) fail('Cover parent changed', 409);
  };
  const refresh = async () => {
    check();
    if (
      JSON.stringify(captured) !==
      JSON.stringify(scope(await deps.capabilities(), deps.publicOrigin()))
    )
      fail('Cover service/catalogue changed', 409);
    check();
  };
  const request = deps.createRequest({
    scope: captured,
    identity,
    assertCurrent: check,
    beforeSend: async (signal) => {
      if (signal.aborted) fail('Cover request cancelled', 409);
      await refresh();
    },
  });
  function accountFacts(response: {
    status: number;
    body: unknown;
  }): TitleAccount {
    if (response.status !== 200)
      fail(
        `Signed cover account refused (HTTP ${response.status})`,
        response.status,
      );
    const body = record(response.body),
      user = record(body.user),
      nostr = record(body.nostr);
    if (
      body.result !== 'ok' ||
      nostr.pubkey !== identity.owner ||
      nostr.serviceKey !== captured.serviceKey ||
      publicUuid(nostr.catalogNamespace) !== captured.catalogNamespace ||
      nostr.authentication !== 'NIP-98' ||
      nostr.requestScoped !== true ||
      nostr.nativeSessionAuthority !== false ||
      nostr.catalogueScope !== 'instance-local' ||
      publicUuid(user.id) !== expectedAccount
    )
      fail('Cover account does not match this exact signer/parent account');
    positive(user.version);
    return {
      id: publicUuid(user.id),
      name:
        typeof user.username === 'string'
          ? user.username
          : fail('Invalid bound cover username'),
      roles: strings(user.roles),
      permissions: strings(user.permissions),
    };
  }
  const account = accountFacts(
    await request({
      url: captured.origin + '/api/v1/nostr/account-context',
      method: 'GET',
    }),
  );
  check();
  const mutationRequest = deps.createRequest({
    scope: captured,
    identity,
    assertCurrent: check,
    beforeSend: async (signal) => {
      if (signal.aborted) fail('Cover request cancelled', 409);
      await refresh();
      accountFacts(
        await request({
          url: captured.origin + '/api/v1/nostr/account-context',
          method: 'GET',
        }),
      );
      check();
      if (titleAwaitingProjection(title()))
        fail(
          'Accepted original title is read-only while projection is pending',
          409,
        );
    },
  });
  const key = `weebdex.signed-cover-pending.v1:${captured.origin}:${captured.serviceKey}:${captured.catalogNamespace}:${identity.owner}:${account.id}:${mangaId}`,
    pending = () => deps.fences.get(key) !== null;
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
  const read = async () => {
    check();
    if (busy) fail('A cover command is in progress', 409);
    const response = await request({
      url: `${captured.origin}/api/v1/nostr/manga/${mangaId}/covers`,
      method: 'GET',
    });
    check();
    if (response.status !== 200)
      fail(
        `Signed cover list refused (HTTP ${response.status})`,
        response.status,
      );
    const body = record(response.body);
    if (
      body.result !== 'ok' ||
      body.response !== 'collection' ||
      !Array.isArray(body.data) ||
      body.data.length > 100 ||
      body.limit !== 100 ||
      body.offset !== 0 ||
      !Number.isSafeInteger(body.total) ||
      Number(body.total) < body.data.length
    )
      fail('Invalid bounded cover collection');
    const rows = body.data.map(cover);
    if (new Set(rows.map((row) => row.id)).size !== rows.length)
      fail('Duplicate cover receipt');
    known.clear();
    for (const row of rows) known.set(row.id, row);
    return { data: rows, total: Number(body.total) };
  };
  const current = (row: CoverRecord) => {
    check();
    const saved = known.get(publicUuid(row.id));
    if (!saved || saved.version !== row.version || row.mangaId !== mangaId)
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
    if (busy || pending() || titleAwaitingProjection(title()))
      fail(
        'This cover/title command is unconfirmed or read-only. Nothing was replayed.',
        409,
      );
    if (row) row = current(row);
    const parentVersion = positive(title().version),
      previous = row,
      desired = action === 'remove' ? undefined : coverMetadata(input);
    busy = true;
    let installed = false;
    try {
      let body: Uint8Array, contentType: string, fileHash: string | undefined;
      if (action === 'upload') {
        const multipart = await serializeCoverMultipart(
          file!,
          parentVersion,
          desired!,
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
            if (desired![field] !== row![field])
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
        url: `${captured.origin}/api/v1/cover/${row?.id ?? mangaId}`,
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
        fail('Invalid signed cover envelope');
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
      disposed = true;
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
        url: `${captured.origin}/api/v1/nostr/covers/${row.id}/original`,
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
