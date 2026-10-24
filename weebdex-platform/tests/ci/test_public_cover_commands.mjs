import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import assert from 'node:assert/strict';
import { api, multipart, transport, directory, root, bytes, digest, plain, image, metadata, row, fixture, json, entity, manga, cover, actor, namespace, second, owner, service, origin } from './cover-command-fixture.mjs';
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

function capabilityTitle({
  state = 'published',
  locked = false,
  owns = false,
  pending = false,
} = {}) {
  return {
    state,
    locked,
    public_api: {
      resource: {
        attributes: {
          nostr: pending
            ? { publicationStatus: 'accepted_pending_projection' }
            : {},
        },
        relationships: owns ? [{ type: 'creator', id: actor }] : [],
      },
    },
  };
}
for (const [name, roles, permissions, title, expected] of [
  [
    'published cover-only editor needs no manga.update',
    ['user'],
    ['manga.cover.edit'],
    {},
    { edit: true, remove: false },
  ],
  [
    'title-metadata grant alone does not grant cover editing',
    ['user'],
    ['manga.update'],
    {},
    { edit: false, remove: false },
  ],
  [
    'ordinary reader has no cover write capability',
    ['user'],
    [],
    {},
    { edit: false, remove: false },
  ],
  [
    'moderator removal remains independent of metadata and cover-edit grants',
    ['moderator'],
    ['cover.delete'],
    { locked: true },
    { edit: false, remove: true },
  ],
  [
    'locked ordinary cover editor stays read-only',
    ['user'],
    ['manga.cover.edit'],
    { locked: true },
    { edit: false, remove: false },
  ],
  [
    'locked moderator cover editor needs no title-metadata grant',
    ['moderator'],
    ['manga.cover.edit'],
    { locked: true },
    { edit: true, remove: false },
  ],
  [
    'draft owner can edit covers with the native create grant',
    ['user'],
    ['manga.create'],
    { state: 'draft', owns: true },
    { edit: true, remove: false },
  ],
  [
    'another draft owner is not inferred from the create grant',
    ['user'],
    ['manga.create'],
    { state: 'draft' },
    { edit: false, remove: false },
  ],
  [
    'submitted owner retains the native cover-edit policy',
    ['user'],
    ['manga.create'],
    { state: 'submitted', owns: true },
    { edit: true, remove: false },
  ],
  [
    'accepted original projection fences all cover commands',
    ['moderator'],
    ['manga.cover.edit', 'cover.delete'],
    { pending: true },
    { edit: false, remove: false },
  ],
])
  test(`actual cover capability: ${name}`, () => {
    assert.deepEqual(
      plain(
        api.coverCommandCapabilities(
          { id: actor, roles, permissions },
          capabilityTitle(title),
        ),
      ),
      expected,
    );
  });
test('no captured account exposes no cover command capability', () => {
  assert.deepEqual(
    plain(api.coverCommandCapabilities(undefined, capabilityTitle())),
    { edit: false, remove: false },
  );
});
test('rendered cover controls use separate native capabilities and preserve shared command fences', () => {
  const coverSource = fs.readFileSync(
      path.join(directory, 'PublicCoverEditor.svelte'),
      'utf8',
    ),
    titleSource = fs.readFileSync(
      path.join(directory, 'PublicTitleEditor.svelte'),
      'utf8',
    ),
    mount = titleSource.match(/<PublicCoverEditor\b[\s\S]*?\/>/)?.[0];
  assert.ok(mount);
  assert.doesNotMatch(mount, /!canWrite/);
  for (const guard of [
    'busy',
    'pending',
    'dispositionBusy',
    'dispositionPending',
    'removed',
    'metadataDirty',
    'awaitingProjection',
  ])
    assert.ok(mount.includes(guard), `Parent cover guard retained: ${guard}`);
  assert.match(
    coverSource,
    /coverCommandCapabilities\(controller\?\.account,title\)/,
  );
  assert.match(
    coverSource,
    /if\(!controller\|\|!file\|\|!canEdit\|\|disabled\|\|pending\)return/,
  );
  assert.match(
    coverSource,
    /if\(!controller\|\|!selected\|\|!canEdit\|\|disabled\|\|pending\)return/,
  );
  assert.match(coverSource, /<fieldset disabled=\{!canEdit\}/);
  assert.match(coverSource, /<\/fieldset>\s*\{#if selected&&canRemove\}/);
});

for(const locale of ['tam','und','zh-hant','EN',null])test('description-only public cover update does not rewrite imported '+locale,async()=>{
 const f=fixture({handler:(request,current)=>{if(request.init.method==='GET'&&request.path.endsWith('/covers'))current.attributes.locale=locale;}}),c=await f.open();
 const existing=(await c.read()).data[0];const updated=await c.update(existing,{locale:locale??'',volume:existing.volume,description:'Changed only',main:existing.main});
 const request=f.calls.findLast(call=>call.init.method==='PUT'),body=JSON.parse(new TextDecoder().decode(request.body));assert.equal(Object.hasOwn(body,'locale'),false);assert.equal(body.description,'Changed only');assert.equal(updated.locale,locale);assert.equal(c.pending(),false);
});

test('description-only receipt cannot silently change an untouched imported cover locale',async()=>{
 const f=fixture({handler:(request,current)=>{if(request.init.method==='GET'&&request.path.endsWith('/covers'))current.attributes.locale='tam';if(request.init.method==='PUT')return entity({...current,attributes:{...current.attributes,version:current.attributes.version+1,description:'Edited',locale:'ta'}});}}),c=await f.open(),existing=(await c.read()).data[0];
 await assert.rejects(()=>c.update(existing,{locale:'tam',volume:existing.volume,description:'Edited',main:existing.main}),/requested metadata/);assert.equal(c.pending(),true);
});
