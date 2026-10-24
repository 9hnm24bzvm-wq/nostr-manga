// Actual serializer/controller/transport. HTTP and signatures are finite stand-ins.
// This explicit White_Space table is an independent fixture oracle, not proof of
// the real Rust/HTTP path (which must be exercised separately).
import test from 'node:test';
import assert from 'node:assert/strict';
import { multipart, fixture, metadata, image, row, entity, plain, second, bytes } from './cover-command-fixture.mjs';
const whiteSpace = new Set([9,10,11,12,13,32,0x85,0xa0,0x1680,0x2000,0x2001,0x2002,0x2003,0x2004,0x2005,0x2006,0x2007,0x2008,0x2009,0x200a,0x2028,0x2029,0x202f,0x205f,0x3000]);
function serverText(text) {
  const chars = [...text];
  while (chars.length && whiteSpace.has(chars[0].codePointAt(0))) chars.shift();
  while (chars.length && whiteSpace.has(chars.at(-1).codePointAt(0))) chars.pop();
  return chars.join('');
}
test('all Unicode White_Space boundaries normalize, while BOM and non-whitespace remain',()=>{
  for(const code of whiteSpace){const mark=String.fromCodePoint(code);assert.equal(multipart.coverMetadata({...metadata,volume:mark+'2'+mark,description:mark+'A'+mark}).description,'A');}
  for(const mark of ['\ufeff','\u180e','\u200b'])assert.equal(multipart.coverMetadata({...metadata,description:mark+'A'+mark}).description,mark+'A'+mark);
});
const examples = [
  ['NEL', '\u00852\u0085', '\u0085Annotation\u0085'],
  ['BOM', '\ufeff2\ufeff', '\ufeffAnnotation\ufeff'],
  ['ordinary', ' \t2 \n', ' \tAnnotation \n'],
  ['empty', '\u0085 \t', '\u0085 \n'],
  ['BOM-only', '\ufeff', '\ufeff'],
  ['internal whitespace', '2\u00853', 'A\u0085B\ufeffC'],
];
function requestFields(request) {
  if (request.init.method === 'PUT') return JSON.parse(new TextDecoder().decode(request.body));
  const body = new TextDecoder().decode(request.body), result = {};
  for (const key of ['volume','description']) {
    const match = body.match(new RegExp('name="'+key+'"\\r\\n\\r\\n([\\s\\S]*?)\\r\\n--'));
    assert.ok(match, key+' multipart field'); result[key] = match[1];
  }
  assert.ok(Buffer.from(request.body).includes(Buffer.from(bytes)), 'original bytes unchanged');
  return result;
}
for (const method of ['POST','PUT']) for (const [name,volume,description] of examples) {
  test(method+' accepts independently normalized '+name+' receipt without uncertainty', async () => {
    const f = fixture({handler(request,current) {
      if (request.init.method === 'GET') {
        // PUT empty must be an explicit clear rather than a no-op.
        current.attributes.volume = 'old'; current.attributes.description = 'old';
        return;
      }
      if (request.init.method !== method) return;
      const fields = requestFields(request), saved = method === 'POST' ? row(second) : plain(current);
      if (method === 'PUT') saved.attributes.version++;
      for (const key of ['volume','description']) saved.attributes[key] = key === 'volume' && serverText(fields[key]) === '' ? null : serverText(fields[key]);
      return entity(saved, method === 'POST' ? 201 : 200);
    }}), c = await f.open(), input = {...metadata,volume,description};
    const accepted = method === 'POST' ? await c.upload(image,input) : await c.update((await c.read()).data[0],input);
    assert.equal(accepted.volume,serverText(volume)); assert.equal(accepted.description,serverText(description));
    const fields = requestFields(f.calls.findLast(call=>call.init.method===method));
    assert.equal(fields.volume,serverText(volume)); assert.equal(fields.description,serverText(description));
    assert.equal(c.pending(),false); assert.equal(f.fences.size,0);
  });
}
for (const locale of [null,'tam','und','zh-hant','EN']) test('unchanged imported '+locale+' metadata is compared before normalization',()=>{
  const previous = {locale,volume:'\u0085 2 ',description:'\u0085 Original ',main:true};
  assert.deepEqual(plain(multipart.coverMetadataPatch({...previous,locale:locale??''},previous)),{});
  const patch = plain(multipart.coverMetadataPatch({...previous,locale:locale??'',description:'\u0085 Changed \u0085'},previous));
  assert.deepEqual(patch,{description:'Changed'});
});
test('BOM-only edit is not silently dropped as no-op; NEL-only edit is',async()=>{
  const f=fixture(),c=await f.open(),saved=(await c.read()).data[0];
  assert.deepEqual(plain(multipart.coverMetadataPatch({description:'\ufeff'},saved)),{description:'\ufeff'});
  await assert.rejects(()=>c.update(saved,{description:'\u0085'}),/No cover metadata changes/);
  assert.equal(c.pending(),false);assert.ok(f.calls.every(call=>call.init.method==='GET'));
});
for (const method of ['POST','PUT']) for (const contradiction of ['metadata','source','original']) test(method+' '+contradiction+' contradiction stays fenced after normalization',async()=>{
  let writes=0;
  const f=fixture({handler(request,current){
    if(request.init.method!==method)return;
    writes++;const fields=requestFields(request),saved=method==='POST'?row(second):plain(current);
    if(method==='PUT')saved.attributes.version++;
    for(const key of ['volume','description'])saved.attributes[key]=key==='volume'&&serverText(fields[key])===''?null:serverText(fields[key]);
    if(contradiction==='metadata')saved.attributes.description='Contradictory';
    if(contradiction==='source'){if(method==='PUT')saved.attributes.nostr.sourceEventId='f'.repeat(64);else saved.attributes.nostr.originalEventRewritten=true;}
    if(contradiction==='original')saved.attributes.nostr.original.sha256='0'.repeat(64);
    return entity(saved,method==='POST'?201:200);
  }}),c=await f.open(),input={...metadata,volume:'\u00852\u0085',description:'\u0085Annotation\u0085'},saved=(await c.read()).data[0];
  await assert.rejects(()=>method==='POST'?c.upload(image,input):c.update(saved,input));assert.equal(c.pending(),true);
  await c.read();assert.equal(c.pending(),true);const reopened=await f.open();
  await assert.rejects(()=>method==='POST'?reopened.upload(image,input):reopened.update(saved,input));assert.equal(writes,1);
});
