// Actual source-comparison helper controls. No runtime source normalization.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {createHash} from 'node:crypto';
import test from 'node:test';
import {nativeSiteClientBindingAssociationSha256,nativeSiteClientBindingStage,nativeSiteClientBindingPath,readNativeSiteClientBindingAssociation,restoreNativeSiteClientBindingSource,readNativeSourceAssociation,restoreNativeReadSource} from './reviewed-native-source-successors.mjs';
const metadata = fs.readFileSync(new URL('../../docs/fixtures/native-site-client-binding-source-successor.json', import.meta.url));
const physical = fs.readFileSync(new URL('../../../'+nativeSiteClientBindingPath, import.meta.url));
const hash = value => createHash('sha256').update(value).digest('hex');
const data = readNativeSiteClientBindingAssociation();
const restore = (source, bytes = metadata) => restoreNativeSiteClientBindingSource(nativeSiteClientBindingStage,nativeSiteClientBindingPath,source,bytes);
const encoded = value => Buffer.from(JSON.stringify(value,null,2)+'\n');

test('exact metadata binds one response-bound successor without runtime or execution claims',()=>{
 assert.equal(hash(metadata),nativeSiteClientBindingAssociationSha256);
 assert.equal(data.current_bytes,20036);assert.equal(data.current_sha256,'fc0270dd2e3e0bb3b77967c045f07e88555103fbf969a2887c4b25803d6ab935');
 assert.equal(data.previous_bytes,19944);assert.equal(data.previous_sha256,'6c49b67a4c994ea15746708bd5d0e8136abf6fe30d26c1e24adbf34829bc24df');
 assert.equal(data.literal_reversals.length,1);assert.equal(data.source_only,true);
 for(const key of ['runtime_normalization','tests_executed','build_executed','browser_executed','http_executed','capacity_qualified'])assert.equal(data[key],false);
 assert.deepEqual(data.required_comparison_order,[nativeSiteClientBindingStage,'native-catalogue-read-fanout-to-historical40']);
});
test('actual helper restores the exact frozen predecessor for both Buffer and string inputs',()=>{
 const previous=restore(physical);assert(Buffer.isBuffer(previous));assert.equal(previous.length,data.previous_bytes);assert.equal(hash(previous),data.previous_sha256);
 assert.equal(restore(physical.toString('utf8')),previous.toString('utf8'));
 assert.equal(data.current_bytes-data.previous_bytes,92);
});
test('actual binding and native helpers compose to the full immutable native predecessor',()=>{
 const native=readNativeSourceAssociation().read_fanout.rows.find(row=>row.path===nativeSiteClientBindingPath);
 assert.equal(data.previous_sha256,native.current_sha256);assert.equal(data.previous_bytes,native.current_bytes);
 const previous=restoreNativeReadSource(nativeSiteClientBindingPath,restore(physical));
 assert.equal(Buffer.byteLength(previous),18831);assert.equal(hash(previous),'cd6379824dfa6218c5b5b373a36be80129f611bb235f81225bc62aab6f67ed00');
 assert.equal(hash(previous),data.native_previous_sha256);assert.equal(Buffer.byteLength(previous),data.native_previous_bytes);
});
test('unknown, partial, edited, duplicate and historical source bytes are rejected by the actual helper',()=>{
 const edited=Buffer.from(physical);edited[0]^=1;const literal=data.literal_reversals[0];
 for(const bad of [edited,physical.subarray(1),physical.subarray(0,-1),Buffer.concat([physical,Buffer.from(' ')]),Buffer.concat([physical,Buffer.from(literal.current)]),physical.toString().replace(literal.current,literal.previous),restore(physical)])assert.throws(()=>restore(bad));
});
test('wrong stage, wrong path and malformed source types never enter reconstruction',()=>{
 for(const stage of ['',null,'unknown',nativeSiteClientBindingStage+'unknown','native-catalogue-read-fanout-to-historical40'])assert.throws(()=>restoreNativeSiteClientBindingSource(stage,nativeSiteClientBindingPath,physical));
 for(const path of ['',null,'unknown','../'+nativeSiteClientBindingPath,nativeSiteClientBindingPath+'unknown'])assert.throws(()=>restoreNativeSiteClientBindingSource(nativeSiteClientBindingStage,path,physical));
 for(const source of [null,undefined,{},[],0,new String(physical.toString()),new Uint8Array(physical)])assert.throws(()=>restore(source));
});
test('every metadata field, literal, ordering and encoding remains indivisibly hash-bound',()=>{
 const candidates=[metadata.toString(),new Uint8Array(metadata),null,Buffer.concat([metadata,Buffer.from(' ')]),metadata.subarray(1),encoded(Object.fromEntries(Object.entries(data).reverse()))];
 for(const key of Object.keys(data)){const value=structuredClone(data);delete value[key];candidates.push(encoded(value));}
 for(const key of Object.keys(data.literal_reversals[0])){const value=structuredClone(data);delete value.literal_reversals[0][key];candidates.push(encoded(value));}
 for(const mutate of [value=>value.literal_reversals[0].ordinal=2,value=>value.literal_reversals[0].current+=' ',value=>value.literal_reversals[0].previous+=' ',value=>value.literal_reversals.push(value.literal_reversals[0]),value=>value.required_comparison_order.reverse(),value=>value.current_sha256='0'.repeat(64),value=>value.previous_sha256='0'.repeat(64),value=>value.native_previous_sha256='0'.repeat(64),value=>value.runtime_normalization=true]){const value=structuredClone(data);mutate(value);candidates.push(encoded(value));}
 for(const unknown of candidates){assert.throws(()=>readNativeSiteClientBindingAssociation(unknown));assert.throws(()=>restore(physical,unknown));}
});
test('omitted, repeated and reversed comparison stages fail through actual helper calls',()=>{
 const intermediate=restore(physical),previous=restoreNativeReadSource(nativeSiteClientBindingPath,intermediate);
 assert.throws(()=>restoreNativeReadSource(nativeSiteClientBindingPath,physical),'Binding layer cannot be skipped');
 assert.throws(()=>restore(intermediate),'Binding layer cannot be applied twice');
 assert.throws(()=>restore(previous),'Binding layer cannot follow the historical native result');
 assert.throws(()=>restoreNativeReadSource(nativeSiteClientBindingPath,previous),'Native layer cannot be applied twice');
});
test('the physical current runtime retains chapterPage and is not replaced by comparison bytes',()=>{
 const previous=restore(physical),literal=data.literal_reversals[0];
 assert.equal(physical.toString().split(literal.current).length,2);assert.equal(previous.toString().split(literal.previous).length,2);
 assert.match(physical.toString(),/chapterPage:response\.body\.chapterPage/);assert.doesNotMatch(previous.toString(),/chapterPage:response\.body\.chapterPage/);
 assert.deepEqual(fs.readFileSync(new URL('../../../'+nativeSiteClientBindingPath,import.meta.url)),physical);
 assert.equal(hash(physical),data.current_sha256);
});
