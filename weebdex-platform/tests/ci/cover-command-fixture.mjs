// Finite evaluation of the actual cover controller, multipart serializer and
// exact NIP-98 transport. SHA-256 is real; signatures, HTTP and image decoding
// are explicit stand-ins. No browser, network, database or real keys are used.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { createHash, webcrypto } from 'node:crypto';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
const root = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../..',
);
const directory = path.join(root, 'platform-integrations/public-api');
const context = vm.createContext({
  Error,
  URL,
  URLSearchParams,
  TextEncoder,
  TextDecoder,
  Uint8Array,
  ArrayBuffer,
  Blob,
  AbortController,
  Response,
  crypto: webcrypto,
  setTimeout,
  clearTimeout,
  performance,
  console,
});
const modules = new Map(),
  digest = (bytes) => createHash('sha256').update(bytes).digest('hex');
function stub(id, exports) {
  if (!modules.has(id))
    modules.set(
      id,
      new vm.SyntheticModule(
        Object.keys(exports),
        function () {
          for (const [key, value] of Object.entries(exports))
            this.setExport(key, value);
        },
        { context, identifier: id },
      ),
    );
  return modules.get(id);
}
async function load(name) {
  const file = path.join(directory, name + '.ts');
  const sourceFile = name==='cover-multipart'&&process.env.WEEBDEX_COVER_UNICODE_CONTROL_SOURCE?path.join(process.env.WEEBDEX_COVER_UNICODE_CONTROL_SOURCE,name+'.ts'):name==='cover-client'&&process.env.WEEBDEX_COVER_PAGINATION_CONTROL_SOURCE?path.join(process.env.WEEBDEX_COVER_PAGINATION_CONTROL_SOURCE,name+'.ts'):file;
  if (!modules.has(file)) {
    const module = new vm.SourceTextModule(
      stripTypeScriptTypes(fs.readFileSync(sourceFile, 'utf8'), {
        mode: 'transform',
        sourceUrl: file,
      }),
      { context, identifier: file },
    );
    modules.set(file, module);
    await module.link(async (specifier, importer) => {
      if (specifier.includes('local-library/publication'))
        return stub('publication', {
          capabilities: () => {
            throw Error('Real capability request forbidden');
          },
          selectedSigner: () => {
            throw Error('Real signer forbidden');
          },
        });
      if (specifier.includes('protocol/nostr.js'))
        return stub('nostr', {
          origin: (value) => new URL(value).origin,
          proveService: () => {
            throw Error('Real proof forbidden');
          },
          signExact: () => {
            throw Error('Real signature forbidden');
          },
        });
      if (specifier.includes('protocol/codec.js'))
        return stub('codec', {
          hash: digest,
          b64: (bytes) => Buffer.from(bytes).toString('base64'),
        });
      if (specifier === './title-client')
        return stub('title', {
          titleAwaitingProjection: (value) =>
            value?.public_api?.resource?.attributes?.nostr
              ?.publicationStatus === 'accepted_pending_projection',
        });
      return load(
        path
          .relative(
            directory,
            path.resolve(path.dirname(importer.identifier), specifier),
          )
          .replace(/\.ts$/, ''),
      );
    });
  }
  return modules.get(file);
}
const module = await load('cover-client');
await module.evaluate();
const api = module.namespace;
const multipart = (await load('cover-multipart')).namespace,
  transport = (await load('nostr-upload-request')).namespace;
const manga = '11111111-1111-8111-8111-111111111111',
  cover = '22222222-2222-8222-8222-222222222222',
  actor = '33333333-3333-8333-8333-333333333333',
  namespace = '44444444-4444-8444-4444-444444444444',
  second = '55555555-5555-8555-8555-555555555555';
const owner = 'a'.repeat(64),
  service = 'b'.repeat(64),
  origin = 'https://fixture.invalid',
  plain = (value) => JSON.parse(JSON.stringify(value));
const bytes = Uint8Array.from([137, 80, 78, 71, 13, 10, 26, 10, 0, 1, 2, 3]);
const image = {
  name: 'original.png',
  type: 'image/png',
  size: bytes.length,
  arrayBuffer: async () => bytes.slice().buffer,
};
const metadata = { locale: 'ja', volume: '', description: '', main: true };
function row(id = cover, version = 1) {
  return {
    id,
    type: 'cover_art',
    attributes: {
      locale: 'ja',
      volume: null,
      description: null,
      version,
      nostr: {
        metadataScope: 'instance-local',
        main: true,
        sourceEventId: null,
        originalEventRewritten: false,
        original: {
          sha256: digest(bytes),
          mime: 'image/png',
          bytes: bytes.length,
          dimensions: [2, 2],
        },
      },
    },
    relationships: [{ type: 'manga', id: manga }],
  };
}
const json = (body, status = 200) =>
  new Response(JSON.stringify(body), {
    status,
    headers: {
      'content-type': 'application/json',
      'cache-control': 'private, no-store',
      vary: 'Origin, Authorization',
      'x-content-type-options': 'nosniff',
    },
  });
const entity = (data, status = 200) =>
  json({ result: 'ok', response: 'entity', data }, status);
function fixture(options = {}) {
  const calls = [],
    proofs = [],
    fences = new Map(),
    state = { active: true, identity: true },
    policy = {
      available: true,
      schema_ready: true,
      origin,
      service_key: service,
      catalog_namespace: namespace,
    };
  const title = {
    id: manga,
    version: 7,
    public_api: { resource: { attributes: { nostr: {} } } },
  };
  const account = {
    result: 'ok',
    user: {
      id: actor,
      username: 'Bound account',
      version: 2,
      roles: ['moderator'],
      permissions: ['manga.create', 'manga.cover.edit', 'cover.delete'],
    },
    nostr: {
      pubkey: owner,
      serviceKey: service,
      catalogNamespace: namespace,
      authentication: 'NIP-98',
      requestScoped: true,
      nativeSessionAuthority: false,
      catalogueScope: 'instance-local',
    },
  };
  options.account?.(account);
  let current = row(),
    counter = 0;
  const primitives = {
    hash: digest,
    b64: (bytes) => Buffer.from(bytes).toString('base64'),
    nonce: () => `synthetic_nonce_${String(++counter).padStart(16, '0')}`,
    now: () => 1700000000,
    clock: () => performance.now(),
    setTimer: setTimeout,
    clearTimer: clearTimeout,
    signExact: async (signer, key, event) => {
      proofs.push(plain(event));
      return signer.signEvent({ ...event, pubkey: key });
    },
    fetch: async (url, init) => {
      const request = {
        url,
        init,
        body: init.body && new Uint8Array(init.body),
        path: new URL(url).pathname,
      };
      calls.push(request);
      if (request.path.endsWith('/account-context')) return json(account);
      if (options.handler) {
        const response = await options.handler(request, current);
        if (response !== undefined) return response;
      }
      if (init.method === 'GET' && request.path.endsWith('/covers'))
        return json({
          result: 'ok',
          response: 'collection',
          data: [current],
          limit: 100,
          offset: 0,
          total: 1,
        });
      if (init.method === 'GET' && request.path.endsWith('/original'))
        return new Response(bytes, {
          headers: {
            'content-type': 'image/png',
            'cache-control': 'private, no-store',
            vary: 'Origin, Authorization',
            'x-content-type-options': 'nosniff',
          },
        });
      if (init.method === 'POST') {
        current = row(second);
        return entity(current, 201);
      }
      if (init.method === 'PUT') {
        const patch = JSON.parse(new TextDecoder().decode(init.body));
        current = plain(current);
        current.attributes.version++;
        for (const key of ['locale', 'volume', 'description'])
          if (key in patch) current.attributes[key] = patch[key];
        if ('main' in patch) current.attributes.nostr.main = patch.main;
        return entity(current);
      }
      if (init.method === 'DELETE') return new Response(null, { status: 204 });
      throw Error('Unreviewed finite route ' + request.path);
    },
  };
  const deps = {
    capabilities: async () => ({ ...policy }),
    selectedSigner: () => ({
      owner,
      generation: 3,
      check: () => {
        if (!state.identity) throw Error('Signer changed');
      },
      signer: {
        signEvent: async (event) => ({
          ...event,
          id: 'synthetic',
          sig: 'synthetic',
        }),
      },
    }),
    proveService: async () => ({ origin, serviceKey: service }),
    publicOrigin: () => origin,
    fences: {
      get: (key) => fences.get(key) ?? null,
      set: (key, value) => fences.set(key, value),
      delete: (key) => fences.delete(key),
    },
    createRequest: (context) =>
      transport.createNostrUploadRequest(context, primitives),
  };
  return {
    calls,
    proofs,
    fences,
    state,
    policy,
    title,
    account,
    deps,
    open: () =>
      api.createSignedCoverController(
        () => {
          if (!state.active) throw Error('Route changed');
        },
        () => title,
        actor,
        deps,
      ),
  };
}

export { api, multipart, transport, directory, root, bytes, digest, plain, image, metadata, row, fixture, json, entity, manga, cover, actor, namespace, second, owner, service, origin };
