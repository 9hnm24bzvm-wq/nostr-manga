// Exact finite-test association only. Historical counterfactual pins stay immutable.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
const sha = value => createHash('sha256').update(value).digest('hex');
const predecessor = 'f211b70bc0b9089e79ed48a46291a88204b3b60212cf604db4fb15c7dd8f7340';
const successor = '96a8ddb2cbb5fc86822d84eadd458518fae5e2d3d08607a8e1879829207d0d72';
const edits = [
  [
    "  const parsed = new URL(url); assert.equal(parsed.origin, origin); assert.equal(parsed.href, url); assert.ok(!url.includes('#') && route.test(parsed.pathname));\n  // The real client binds discovery GET and exact-session cancellation DELETE\n  // to its verified account. Admit only their canonical URL extension, never a\n  // decoded/rebuilt substitute for the full URL covered by the signed u tag.\n  if (url.includes('?')) {\n    const discovery = method === 'GET' && parsed.pathname === '/api/v1/upload';\n    const cancellation = method === 'DELETE' && parsed.pathname.startsWith('/api/v1/upload/') && accountIdPattern.test(parsed.pathname.slice('/api/v1/upload/'.length));\n    assert.ok(discovery || cancellation); assert.equal(body?.length ?? 0, 0);\n    assert.equal(typeof expectedAccountId, 'string'); assert.match(expectedAccountId, accountIdPattern);\n",
    "  const parsed = new URL(url); assert.equal(parsed.origin, origin); assert.equal(parsed.href, url); assert.ok(!url.includes('#') && route.test(parsed.pathname));\n  // The real upload client binds its initial discovery GET to the account it\n  // just verified. Admit only that exact canonical URL extension, never a\n  // decoded/rebuilt substitute for the full URL covered by the signed u tag.\n  if (url.includes('?')) {\n    assert.equal(method, 'GET'); assert.equal(parsed.pathname, '/api/v1/upload');\n    assert.equal(typeof expectedAccountId, 'string'); assert.match(expectedAccountId, accountIdPattern);\n"
  ],
  [
    "  if (body !== null && method !== 'GET') assert.equal(tags.get('payload'), uploadDigest(body));\n  if (method === 'DELETE' && url.includes('?')) assert.equal(tags.get('payload'), uploadDigest(Buffer.alloc(0)));\n  if (method === 'GET') assert.equal(body?.length ?? 0, 0);\n",
    "  if (body !== null && method !== 'GET') assert.equal(tags.get('payload'), uploadDigest(body));\n  if (method === 'GET') assert.equal(body?.length ?? 0, 0);\n"
  ]
];
export function restoreCancellationObserver(value) {
  if (sha(value) === predecessor) return value;
  assert.equal(sha(value), successor, 'Unknown cancellation observer successor');
  for (const [current, prior] of edits) {
    assert.equal(value.split(current).length - 1, 1);
    value = value.replace(current, prior);
  }
  assert.equal(sha(value), predecessor);
  return value;
}

// This diagnostic test is itself one historical frozen payload. Its own exact
// source-only successor must be reversed too; no historical runtime is relabeled.
export function restoreCancellationCounterfactualTest(value) {
  assert.equal(sha(value), 'f01f6a6c9c80617d0c2acc080f1dd41fa26f262122e174c23ea7b5edb593015e', 'Unknown cancellation counterfactual test');
  const edits = [
  [
    "import { restoreCancellationObserver, restoreCancellationCounterfactualTest } from './upload-cancel-observer-association.mjs';\n// No browser, server, socket, dependency install, or external network is used.\n",
    "// No browser, server, socket, dependency install, or external network is used.\n"
  ],
  [
    "test('CRC32C implementation matches standard finite vector',()=>{assert.equal((crc32c(Buffer.from('123456789'))^0xffffffff)>>>0,0xe3069283);});\n// A comparison-only temporary tree restores the exact prior observer bytes.\n// The executable source verifier still refuses the changed live tree.\nfunction historicalFreezeComparison() {\n const location=path.join(repo,'.ci/native-upload-counterfactual/source-freeze-current.json');\n const bytes=fs.readFileSync(location),manifest=JSON.parse(bytes),temporary=fs.mkdtempSync(path.join(os.tmpdir(),'cancel-freeze-comparison-'));\n try {\n  for(const row of manifest.files){\n   const base=row.origin==='payload'?repo:path.dirname(source);\n   let data=fs.readFileSync(path.join(base,row.path));\n   if(row.path==='weebdex-platform/scripts/public-upload-recovery-observer.mjs')data=Buffer.from(restoreCancellationObserver(data.toString('utf8')));\n   if(row.path==='weebdex-platform/tests/ci/test_native_upload_counterfactual.mjs')data=Buffer.from(restoreCancellationCounterfactualTest(data.toString('utf8')));\n   const target=path.join(temporary,row.path);fs.mkdirSync(path.dirname(target),{recursive:true});fs.writeFileSync(target,data);\n  }\n  const target=path.join(temporary,'.ci/native-upload-counterfactual/source-freeze-current.json');fs.mkdirSync(path.dirname(target),{recursive:true});fs.writeFileSync(target,bytes);\n  return verifySources(temporary,target);\n } finally {fs.rmSync(temporary,{recursive:true,force:true});}\n}\ntest('historical frozen files match through only the exact cancellation observer association',()=>{assert.match(historicalFreezeComparison(),/^[0-9a-f]{64}$/);assert.throws(()=>verifySources(path.dirname(source),path.join(repo,'.ci/native-upload-counterfactual/source-freeze-current.json')),/Counterfactual contract failed/);});\ntest('current actual closed browser graph links without fabricated application exports',async()=>{const assets=await browserModules(source);const context=vm.createContext({});const modules=new Map();for(const [name,code]of assets)modules.set(name,new vm.SourceTextModule(code,{context,identifier:name}));await modules.get('/entry.js').link(specifier=>{assert.ok(modules.has(specifier),specifier);return modules.get(specifier);});assert.equal(modules.get('/entry.js').status,'linked');assert.ok(assets.has('/modules/schedules.js'));});\n",
    "test('CRC32C implementation matches standard finite vector',()=>{assert.equal((crc32c(Buffer.from('123456789'))^0xffffffff)>>>0,0xe3069283);});\ntest('exact frozen product, observer and candidate files match',()=>{assert.match(verifySources(path.dirname(source),path.join(repo,'.ci/native-upload-counterfactual/source-freeze-current.json')),/^[0-9a-f]{64}$/);});\ntest('current actual closed browser graph links without fabricated application exports',async()=>{const assets=await browserModules(source);const context=vm.createContext({});const modules=new Map();for(const [name,code]of assets)modules.set(name,new vm.SourceTextModule(code,{context,identifier:name}));await modules.get('/entry.js').link(specifier=>{assert.ok(modules.has(specifier),specifier);return modules.get(specifier);});assert.equal(modules.get('/entry.js').status,'linked');assert.ok(assets.has('/modules/schedules.js'));});\n"
  ],
  [
    "  assert.equal(row.sha256,expected[row.path]??before.sha256);\n  const bytes=fs.readFileSync(path.join(path.dirname(source),row.path));\n  assert.equal(hash(row.path.endsWith('/public-upload-recovery-observer.mjs')?restoreCancellationObserver(bytes.toString('utf8')):bytes),row.sha256);\n }\n assert.equal(historicalFreezeComparison(),hash(currentBytes));\n assert.throws(()=>verifySources(path.dirname(source),path.join(repo,'.ci/native-upload-counterfactual/source-freeze.json')),/Counterfactual contract failed/);\n",
    "  assert.equal(row.sha256,expected[row.path]??before.sha256);\n  assert.equal(hash(fs.readFileSync(path.join(path.dirname(source),row.path))),row.sha256);\n }\n assert.equal(verifySources(path.dirname(source),path.join(repo,'.ci/native-upload-counterfactual/source-freeze-current.json')),hash(currentBytes));\n assert.throws(()=>verifySources(path.dirname(source),path.join(repo,'.ci/native-upload-counterfactual/source-freeze.json')),/Counterfactual contract failed/);\n"
  ]
];
  for (const [current, prior] of edits) {
    assert.equal(value.split(current).length - 1, 1);
    value = value.replace(current, prior);
  }
  assert.equal(sha(value), 'bb1140e9eac0942068b63bc9b80d3718cd6bf9fd62d41222e08420e3ef51467f');
  return value;
}
