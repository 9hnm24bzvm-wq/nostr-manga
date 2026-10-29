import {readReaderUxAssociation,readReaderUxFixpassAssociation,restoreReaderUxFixpassPredecessor,restoreReaderUxPredecessor} from './reviewed-card-spinner-source-successor.mjs';
import {nativeSiteClientBindingStage,nativeSiteClientBindingPath,restoreNativeSiteClientBindingSource} from './reviewed-native-source-successors.mjs';
import {restoreBoundedReaderSource} from './reviewed-bounded-reader-source-successors.mjs';
import {restoreModeratorTransportSource} from './reviewed-moderator-transport-source-successors.mjs';
import {readReportTransportAssociation,restoreReportTransportSource} from './reviewed-report-transport-source-successors.mjs';
import {integrationSuccessorPaths,restoreIntegratedSource} from './reviewed-integration-source-successors.mjs';
import {historySuccessorPaths,restoreHistorySource} from './reviewed-history-source-successors.mjs';
// Finite source-only controls. No app, browser, compiler, socket or database.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {fileURLToPath} from 'node:url';
import test from 'node:test';
import {readNativeSourceAssociation,nativeReadSuccessorPaths,restoreNativeReadSource,restoreNativeTransportSource} from './reviewed-native-source-successors.mjs';
import {stripUploadRefusalQaSource} from '../../scripts/public-upload-refusal-qa.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../../..');
const bytes=fs.readFileSync(new URL('../../docs/fixtures/native-catalogue-source-associations.json',import.meta.url));
const association=readNativeSourceAssociation(bytes);
const hash=value=>createHash('sha256').update(value).digest('hex');
// Historical comparisons only. Runtime composition elsewhere reads physical current bytes.
// Source-only historical inputs: first undo the exact second pass, then the
// exact published first pass where applicable. Runtime tests read live files.
const firstReaderPaths = new Set(readReaderUxAssociation().rows.map(row => row.path));
const secondReaderPaths = new Set(readReaderUxFixpassAssociation().rows.map(row => row.path));
function beforeReaderUx(file, value) {
 return firstReaderPaths.has(file) ? restoreReaderUxPredecessor(file, value)
  : secondReaderPaths.has(file) ? restoreReaderUxFixpassPredecessor(file, value) : value;
}
const source=file=>{const live=fs.readFileSync(path.join(root,file),'utf8'),physical=beforeReaderUx(file,live);return file===nativeSiteClientBindingPath?restoreNativeSiteClientBindingSource(nativeSiteClientBindingStage,file,physical):file==='weebdex-platform/platform-integrations/local-library/library.ts'?restoreBoundedReaderSource('bounded-reader-to-frozen-main:platform-integrations/local-library/library.ts',file,physical):file===readReportTransportAssociation().path?restoreReportTransportSource(file,restoreModeratorTransportSource(file,physical,'raw'),'raw'):integrationSuccessorPaths.includes(file)?restoreIntegratedSource(file,physical):physical;};
const encoded=value=>Buffer.from(JSON.stringify(value,null,2)+'\n');

test('exact six native/catalogue read rows preserve the immutable40 historical association and scope',()=>{
 assert.equal(association.product_freeze,'11ea3ea9f593c134f2ba478bb84fa683f4b14149');
 assert.equal(hash(fs.readFileSync(new URL('../../docs/fixtures/native-user-source-associations.json',import.meta.url))),association.prior_association_sha256);
 assert.equal(association.prior_association_sha256,'9d54b6b8a05d920621a3b72fba31f7b4a85c2a81a5d45ff3399075f68b6ea780');
 assert.equal(association.historical_main,'8f878a6c189d2c6e0e5fe3c711724809d39e2ab6');
 assert.equal(association.browser_executed,false);assert.equal(association.http_executed,false);
 assert.match(association.read_fanout.scope,/do not establish native-user discussion fanout or load acceptance/);
 assert.deepEqual(nativeReadSuccessorPaths,['anchors.json','client.ts','site-client.ts','query.ts','normalize.ts','PublicLanguageFilter.svelte'].map(name=>'weebdex-platform/platform-integrations/public-api/'+name));
 const historicalBytes=fs.readFileSync(new URL('../../docs/fixtures/integrated-read-fanout-current-source.json',import.meta.url));
 assert.equal(hash(historicalBytes),association.read_fanout.historical_fixture_sha256);
 const historical=JSON.parse(historicalBytes);assert.equal(Object.keys(historical.sourcePins).length,40);
 let restored=0;
 for(const [file,pin]of Object.entries(historical.sourcePins)){
  const current=source(file);
  const previous=nativeReadSuccessorPaths.includes(file)?(restored++,restoreNativeReadSource(file,current)):historySuccessorPaths.includes(file)?restoreHistorySource(file,current):current;
  assert.equal(hash(previous),pin,file);
 }
 assert.equal(restored,6);
});

for(const row of association.read_fanout.rows)test('exact read source and each literal refuse partial, duplicate, historical or unknown input: '+row.path,()=>{
 const current=source(row.path),previous=restoreNativeReadSource(row.path,current);
 assert.equal(hash(current),row.current_sha256);assert.equal(hash(previous),row.previous_sha256);
 for(const changed of [current+' ',previous,current.slice(0,-1)])assert.throws(()=>restoreNativeReadSource(row.path,changed));
 for(const literal of row.literal_reversals){
  assert.equal(current.split(literal.current).length,2);
  assert.throws(()=>restoreNativeReadSource(row.path,current.replace(literal.current,()=>literal.previous)));
  assert.throws(()=>restoreNativeReadSource(row.path,current+literal.current));
 }
 assert.throws(()=>restoreNativeReadSource(row.path+'?unknown',current));
});

for(const mode of ['raw','qa_stripped'])test('exact three transport budget reversals preserve generic and QA baseline: '+mode,()=>{
 const raw=source(association.transport.path),current=mode==='raw'?raw:stripUploadRefusalQaSource(raw);
 const row=association.transport[mode],previous=restoreNativeTransportSource(current,mode);
 assert.equal(row.literal_reversals.length,3);assert.equal(hash(current),row.current_sha256);assert.equal(hash(previous),row.previous_sha256);
 assert.match(current,/NATIVE_REPORT_JSON_LIMIT = 131072/);
 assert(!previous.includes('NATIVE_REPORT_JSON_LIMIT'));
 for(const changed of [current+' ',previous,current.slice(0,-1)])assert.throws(()=>restoreNativeTransportSource(changed,mode));
 for(const literal of row.literal_reversals){
  assert.equal(current.split(literal.current).length,2);
  assert.throws(()=>restoreNativeTransportSource(current.replace(literal.current,()=>literal.previous),mode));
  assert.throws(()=>restoreNativeTransportSource(current+literal.current,mode));
 }
 assert.throws(()=>restoreNativeTransportSource(current,'unknown'));
 assert.equal(stripUploadRefusalQaSource(restoreNativeTransportSource(raw,'raw')),restoreNativeTransportSource(stripUploadRefusalQaSource(raw),'qa_stripped'));
});

test('metadata bytes, every field, row order and missing/unknown paths fail closed',()=>{
 assert.throws(()=>readNativeSourceAssociation(Buffer.concat([bytes,Buffer.from(' ')])));
 for(const key of Object.keys(association)){const value=structuredClone(association);delete value[key];assert.throws(()=>readNativeSourceAssociation(encoded(value)),key);}
 const mutations=[value=>value.browser_executed=true,value=>value.product_freeze='0'.repeat(40),value=>value.unknown=true,
  value=>value.read_fanout.rows.reverse(),value=>value.read_fanout.rows.pop(),
  value=>value.read_fanout.rows[0].path='unknown',value=>value.read_fanout.rows.push(value.read_fanout.rows[0]),
  value=>value.transport.raw.literal_reversals.reverse(),value=>value.transport.raw.literal_reversals.pop(),
  value=>value.transport.qa_stripped.current_sha256='0'.repeat(64)];
 for(const mutate of mutations){const value=structuredClone(association);mutate(value);const unknown=encoded(value);assert.throws(()=>readNativeSourceAssociation(unknown));assert.throws(()=>restoreNativeReadSource(nativeReadSuccessorPaths[0],source(nativeReadSuccessorPaths[0]),unknown));assert.throws(()=>restoreNativeTransportSource(source(association.transport.path),'raw',unknown));}
});
