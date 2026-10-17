// Finite evaluation of the actual cover controller, multipart serializer and
// exact NIP-98 transport. SHA-256 is real; signatures, HTTP and image decoding
// are explicit stand-ins. No browser, network, database or real keys are used.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { createHash, webcrypto } from 'node:crypto';
import { stripTypeScriptTypes } from 'node:module';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import assert from 'node:assert/strict';
const root = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../..',
);
const directory = path.join(root, 'platform-integrations/public-api');
const context = vm.createContext({
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
  if (!modules.has(file)) {
    const module = new vm.SourceTextModule(
      stripTypeScriptTypes(fs.readFileSync(file, 'utf8'), {
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
test('multipart preserves exact original bytes with first canonical parent version and four explicit fields', async () => {
  const value = await multipart.serializeCoverMultipart(
      image,
      7,
      metadata,
      actor,
      {
        boundary: 'FiniteCoverBoundary0123',
      },
    ),
    text = new TextDecoder().decode(value.body);
  assert.match(
    text,
    /name="expectedVersion"\r\n\r\n7\r\n--FiniteCoverBoundary0123\r\nContent-Disposition: form-data; name="expectedAccountId"/,
  );
  assert.ok(Buffer.from(value.body).includes(Buffer.from(bytes)));
  assert.equal(text.match(/name="file"/g).length, 1);
  assert.equal(text.match(/name="expectedAccountId"/g).length, 1);
  assert.ok(
    text.includes(
      `name="expectedAccountId"\r\n\r\n${actor}\r\n--FiniteCoverBoundary0123\r\nContent-Disposition: form-data; name="file"`,
    ),
  );
  for (const key of Object.keys(metadata))
    assert.equal(text.match(new RegExp(`name="${key}"`, 'g')).length, 1);
  assert.deepEqual([...value.fileBytes], [...bytes]);
});
for (const [name, file, input] of [
  ['GIF', { ...image, type: 'image/gif', name: 'original.gif' }, metadata],
  ['filename mismatch', { ...image, name: 'original.jpg' }, metadata],
  ['unsafe filename', { ...image, name: '../original.png' }, metadata],
  ['wrong size', { ...image, size: 100 }, metadata],
  ['extra metadata', image, { ...metadata, fileName: 'other.png' }],
  ['NUL metadata', image, { ...metadata, description: 'bad\0' }],
  ['invalid locale', image, { ...metadata, locale: '?' }],
  [
    'oversized UTF-8 metadata',
    image,
    { ...metadata, description: '界'.repeat(7000) },
  ],
])
  test('serializer rejects ' + name + ' before signing', async () => {
    await assert.rejects(() =>
      multipart.serializeCoverMultipart(file, 7, input, actor),
    );
  });
test('metadata boundary collision rejects without corrupting the signed multipart', async () => {
  await assert.rejects(
    () =>
      multipart.serializeCoverMultipart(
        image,
        7,
        { ...metadata, description: '--FiniteCoverBoundary0123' },
        actor,
        { boundary: 'FiniteCoverBoundary0123' },
      ),
    /collides/,
  );
});
test('actual transport signs the same once-serialized multipart it dispatches with no cookies', async () => {
  const f = fixture(),
    c = await f.open(),
    accepted = await c.upload(image, metadata),
    call = f.calls.at(-1),
    proof = f.proofs.findLast((event) =>
      event.tags.some((tag) => tag[0] === 'method' && tag[1] === 'POST'),
    );
  assert.equal(accepted.id, second);
  assert.equal(call.init.method, 'POST');
  assert.equal(call.url, origin + '/api/v1/cover/' + manga);
  assert.equal(call.init.credentials, 'omit');
  assert.equal(call.init.redirect, 'error');
  assert.ok(
    new TextDecoder()
      .decode(call.body)
      .includes(`name="expectedAccountId"\r\n\r\n${actor}`),
  );
  assert.equal(
    proof.tags.find((tag) => tag[0] === 'payload')[1],
    digest(call.body),
  );
  assert.equal(proof.tags.find((tag) => tag[0] === 'u')[1], call.url);
  assert.equal(c.pending(), false);
});
test('update needs the exact list read, sends only changed metadata and preserves original provenance', async () => {
  const f = fixture(),
    c = await f.open();
  await assert.rejects(() =>
    c.update({ id: cover, version: 1, mangaId: manga }, metadata),
  );
  const saved = (await c.read()).data[0],
    accepted = await c.update(saved, {
      ...metadata,
      description: 'Annotation',
    });
  assert.equal(accepted.version, 2);
  assert.deepEqual(JSON.parse(new TextDecoder().decode(f.calls.at(-1).body)), {
    version: 1,
    expectedAccountId: actor,
    description: 'Annotation',
  });
  assert.equal(accepted.original.sha256, digest(bytes));
  await assert.rejects(() => c.update(saved, { ...metadata, volume: '3' }));
});
test('moderator removal uses an exact version JSON proof and validates actual204', async () => {
  const f = fixture(),
    c = await f.open(),
    saved = (await c.read()).data[0];
  await c.remove(saved);
  assert.equal(f.calls.at(-1).init.method, 'DELETE');
  assert.deepEqual(JSON.parse(new TextDecoder().decode(f.calls.at(-1).body)), {
    version: 1,
    expectedAccountId: actor,
  });
  assert.equal(c.pending(), false);
});
for (const field of [
  'pubkey',
  'serviceKey',
  'catalogNamespace',
  'authentication',
  'requestScoped',
  'nativeSessionAuthority',
  'catalogueScope',
])
  test('account scope rejects changed ' + field, async () => {
    const f = fixture({
      account: (account) => {
        account.nostr[field] = 'wrong';
      },
    });
    await assert.rejects(() => f.open());
    assert.equal(f.calls.length, 1);
  });
test('fresh changed account binding stops mutation before network dispatch and clears proven-unsent fence', async () => {
  const f = fixture(),
    c = await f.open();
  f.account.user.id = second;
  await assert.rejects(() => c.upload(image, metadata));
  assert.ok(f.calls.every((row) => row.init.method === 'GET'));
  assert.equal(c.pending(), false);
});
for (const what of [
  'identity',
  'route',
  'namespace',
  'parent',
  'accepted projection',
  'closed editor',
])
  test('captured ' + what + ' fences new cover commands', async () => {
    const f = fixture(),
      c = await f.open();
    if (what === 'identity') f.state.identity = false;
    if (what === 'route') f.state.active = false;
    if (what === 'namespace') f.policy.catalog_namespace = second;
    if (what === 'parent') f.title.id = second;
    if (what === 'accepted projection')
      f.title.public_api.resource.attributes.nostr.publicationStatus =
        'accepted_pending_projection';
    if (what === 'closed editor') c.dispose();
    await assert.rejects(() => c.upload(image, metadata));
    assert.ok(f.calls.every((row) => row.init.method === 'GET'));
  });
for (const kind of [
  '503',
  'network',
  'wrong parent',
  'wrong version',
  'wrong original',
  'ignored metadata',
])
  test(
    'unconfirmed ' +
      kind +
      ' stays durably fenced across reads and remount, with one dispatch',
    async () => {
      let writes = 0;
      const f = fixture({
          handler: (request) => {
            if (request.init.method !== 'POST') return;
            writes++;
            if (kind === '503') return json({ result: 'error' }, 503);
            if (kind === 'network') throw Error('Finite connection loss');
            const value = row(second);
            if (kind === 'wrong parent') value.relationships[0].id = second;
            if (kind === 'wrong version') value.attributes.version = 2;
            if (kind === 'wrong original')
              value.attributes.nostr.original.sha256 = '0'.repeat(64);
            if (kind === 'ignored metadata')
              value.attributes.description = 'Ignored requested text';
            return entity(value, 201);
          },
        }),
        c = await f.open();
      await assert.rejects(() => c.upload(image, metadata));
      assert.equal(c.pending(), true);
      await c.read();
      assert.equal(c.pending(), true);
      const reopened = await f.open();
      await assert.rejects(() => reopened.upload(image, metadata));
      assert.equal(writes, 1);
    },
  );
test('known409 rejection clears the pending fence without automatic retry', async () => {
  const f = fixture({
      handler: (request) =>
        request.init.method === 'POST'
          ? json({ result: 'error' }, 409)
          : undefined,
    }),
    c = await f.open();
  await assert.rejects(
    () => c.upload(image, metadata),
    (error) => error.status === 409,
  );
  assert.equal(c.pending(), false);
  assert.equal(f.calls.filter((row) => row.init.method === 'POST').length, 1);
});
test('exact signed original preview checks bytes, hash, MIME and private Authorization variation', async () => {
  const f = fixture(),
    c = await f.open(),
    saved = (await c.read()).data[0],
    blob = await c.preview(saved);
  assert.equal(blob.type, 'image/png');
  assert.deepEqual([...new Uint8Array(await blob.arrayBuffer())], [...bytes]);
  assert.equal(f.calls.at(-1).init.credentials, 'omit');
});
for (const mode of ['hash', 'mime', 'private', 'vary', 'nosniff'])
  test('preview refuses invalid ' + mode + ' evidence', async () => {
    const f = fixture({
        handler: (request) => {
          if (!request.path.endsWith('/original')) return;
          const headers = {
            'content-type': 'image/png',
            'cache-control': 'private, no-store',
            vary: 'Origin, Authorization',
            'x-content-type-options': 'nosniff',
          };
          if (mode === 'mime') headers['content-type'] = 'image/jpeg';
          if (mode === 'private') headers['cache-control'] = 'no-store';
          if (mode === 'vary') headers.vary = 'Origin';
          if (mode === 'nosniff') delete headers['x-content-type-options'];
          return new Response(
            mode === 'hash' ? bytes.map((value) => value ^ 1) : bytes,
            { headers },
          );
        },
      }),
      c = await f.open(),
      saved = (await c.read()).data[0];
    await assert.rejects(() => c.preview(saved));
  });
test('actual cover controls remain a dedicated original interface and title commands share pending guards', () => {
  const cover = fs.readFileSync(
      path.join(directory, 'PublicCoverEditor.svelte'),
      'utf8',
    ),
    title = fs.readFileSync(
      path.join(directory, 'PublicTitleEditor.svelte'),
      'utf8',
    );
  assert.match(cover, /createSignedCoverController/);
  assert.match(cover, /URL\.revokeObjectURL/);
  assert.match(cover, /Approve this instance-local cover removal now/);
  assert.doesNotMatch(cover, /API\.|FormData|canvas|toDataURL/);
  assert.match(title, /coverBusy\|\|coverPending/);
  assert.match(title, /<PublicCoverEditor/);
  assert.match(title, /titleAwaitingProjection/);
});
test('metadata uses the same native whitespace normalization before signing and receipt checks', () => {
  assert.deepEqual(
    plain(
      multipart.coverMetadata({
        ...metadata,
        volume: ' 2 ',
        description: ' Annotation ',
      }),
    ),
    { ...metadata, volume: '2', description: 'Annotation' },
  );
});
test('caller metadata changes during original file loading cannot alter the captured command', async () => {
  let release;
  const held = new Promise((resolve) => {
      release = resolve;
    }),
    file = {
      ...image,
      arrayBuffer: async () => {
        await held;
        return bytes.slice().buffer;
      },
    },
    input = { ...metadata },
    f = fixture(),
    c = await f.open(),
    write = c.upload(file, input);
  input.description = 'Changed later';
  release();
  await write;
  const body = new TextDecoder().decode(f.calls.at(-1).body);
  assert.equal(body.includes('Changed later'), false);
});
test('unchanged metadata is refused before pending marker or mutation dispatch', async () => {
  const f = fixture(),
    c = await f.open(),
    saved = (await c.read()).data[0];
  await assert.rejects(
    () => c.update(saved, metadata),
    /No cover metadata changes/,
  );
  assert.equal(c.pending(), false);
  assert.ok(f.calls.every((row) => row.init.method === 'GET'));
});
test('unavailable pending-marker storage prevents any new cover signature or upload dispatch', async () => {
  const f = fixture(),
    c = await api.createSignedCoverController(
      () => {},
      () => f.title,
      actor,
      {
        ...f.deps,
        fences: {
          ...f.deps.fences,
          set: () => {
            throw Error('Storage full');
          },
        },
      },
    );
  const signed = f.proofs.length;
  await assert.rejects(() => c.upload(image, metadata), /Storage full/);
  assert.equal(f.proofs.length, signed);
  assert.ok(f.calls.every((row) => row.init.method === 'GET'));
});
test('frontend preparation explicitly declares rendered cover control and pins its controller and exact serializer', () => {
  const source = fs.readFileSync(
    path.join(root, 'scripts/prepare-frontend-integration.mjs'),
    'utf8',
  );
  assert.match(source, /"public-api\/PublicCoverEditor\.svelte"/);
  assert.match(source, /public_signed_cover_client_sha256/);
  assert.match(source, /public_signed_cover_multipart_sha256/);
  assert.match(source, /public_signed_original_covers_enabled: publicApiGate/);
});
test('a missing or native-shaped account intent fails before original file access or signature', async () => {
  for (const account of [undefined, 'NativeUser1']) {
    let reads = 0;
    await assert.rejects(() =>
      multipart.serializeCoverMultipart(
        {
          ...image,
          arrayBuffer: async () => {
            reads++;
            return bytes.slice().buffer;
          },
        },
        7,
        metadata,
        account,
      ),
    );
    assert.equal(reads, 0);
  }
});
