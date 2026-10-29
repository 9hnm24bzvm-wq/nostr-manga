import fs from 'node:fs';import path from 'node:path';import {fileURLToPath} from 'node:url';import {createHash} from 'node:crypto';import test from 'node:test';import assert from 'node:assert/strict';
import {titleSpinnerPatches,titleSpinnerManifest,assertTitleSpinnerScope,applyTitleSpinnerOutput,restoreTitleSpinnerOutput} from '../../platform-integrations/local-library/title-spinner-overlays.mjs';
import {readReaderFollowupSourceAssociation,restoreReaderFollowupSourcePredecessor,readTitleSpinnerSourceAssociation,restoreTitleSpinnerSourcePredecessor,readReaderUxFixpassAssociation,restoreReaderUxFixpassPredecessor} from './reviewed-six-persona-source-successor.mjs';
import {composed} from './composed-account-library-fixture.mjs';
const root=fileURLToPath(new URL('../../../',import.meta.url)),hash=bytes=>createHash('sha256').update(bytes).digest('hex');
test('title spinner exact successor only wraps two pending statuses; every script and visual class stays identical',async()=>{
 const entry=titleSpinnerManifest[0],current=await composed(entry.path),previous=restoreTitleSpinnerOutput(current,entry.path);
 assert.equal(hash(previous),entry.predecessor_sha256);assert.equal(hash(current),entry.output_sha256);assert.equal(applyTitleSpinnerOutput(previous,entry.path),current);
 assert.deepEqual(current.match(/<script\b[^>]*>[\s\S]*?<\/script>/g),previous.match(/<script\b[^>]*>[\s\S]*?<\/script>/g));
 assert.deepEqual([...current.matchAll(/class=([^\r\n]+)/g)].map(x=>x[0]),[...previous.matchAll(/class=([^\r\n]+)/g)].map(x=>x[0]));
 for(const patch of titleSpinnerPatches)assert.equal(current.split(patch.after).length,2);
 for(const value of [previous,current+' ',current.slice(1)])assert.throws(()=>restoreTitleSpinnerOutput(value,entry.path));
 assert.throws(()=>applyTitleSpinnerOutput(current,entry.path));assert.equal(applyTitleSpinnerOutput('unrelated','other.svelte'),'unrelated');
});
test('title spinner registry rejects omitted, extra, edited and reordered literal exceptions',()=>{
 const old=structuredClone(titleSpinnerPatches),restore=()=>titleSpinnerPatches.splice(0,titleSpinnerPatches.length,...structuredClone(old));try{
 for(const mutate of [()=>titleSpinnerPatches.pop(),()=>titleSpinnerPatches.push(old[0]),()=>titleSpinnerPatches.reverse(),()=>titleSpinnerPatches[0].after+='x']){mutate();assert.throws(assertTitleSpinnerScope);restore();}
 }finally{restore();}assertTitleSpinnerScope();
});
test('presentation source successor preserves immutable 157-row association and restores exact frozen inputs without inherited qualification',()=>{
 const a=readTitleSpinnerSourceAssociation(),prior=readReaderUxFixpassAssociation();assert.equal(a.predecessor_association_sha256,'860ec694f57f712e1f3ce10753340ff03b3dd11c92cbf2dd6dc19fb85fae95e6');assert.equal(prior.rows.length,157);
 assert.equal(a.browser_executed,false);assert.equal(a.http_executed,false);assert.equal(hash(fs.readFileSync(path.join(root,a.predecessor_association_path))),a.predecessor_association_sha256);
 for(const row of a.rows){const source=fs.readFileSync(path.join(root,row.path)),previous=restoreTitleSpinnerSourcePredecessor(row.path,source);assert.equal(hash(readReaderFollowupSourceAssociation().rows.some(next=>next.path===row.path)?restoreReaderFollowupSourcePredecessor(row.path,source):source),row.current_sha256);if(row.added)assert.equal(previous,null);else{assert.equal(hash(previous),row.previous_sha256);const frozen=prior.rows.find(x=>x.path===row.path);if(frozen)assert.equal(hash(previous),frozen.current_sha256);}for(const bad of [source.subarray(1),Buffer.concat([source,Buffer.from(' ')]),null,{}])assert.throws(()=>restoreTitleSpinnerSourcePredecessor(row.path,bad));}
 for(const row of prior.rows)restoreReaderUxFixpassPredecessor(row.path,fs.readFileSync(path.join(root,row.path)));
 const changed=structuredClone(a);changed.rows.reverse();assert.throws(()=>readTitleSpinnerSourceAssociation(Buffer.from(JSON.stringify(changed))));assert.throws(()=>restoreTitleSpinnerSourcePredecessor('unknown',Buffer.from('x')));
});
