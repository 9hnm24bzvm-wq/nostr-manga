import fs from 'node:fs';import path from 'node:path';import {fileURLToPath} from 'node:url';import {createHash} from 'node:crypto';import test from 'node:test';import assert from 'node:assert/strict';
import {readCompletionManifest,readCompletionPatches,applyReadCompletionOutput,restoreReadCompletionOutput,assertReadCompletionScope} from '../../platform-integrations/local-library/reader-completion-overlays.mjs';
import {readSixPersonaSourceAssociation,restoreSixPersonaSourcePredecessor,readCardSpinnerSourceAssociation,restoreCardSpinnerSourcePredecessor,readReaderFollowupSourceAssociation,restoreReaderFollowupSourcePredecessor,readTitleSpinnerSourceAssociation,restoreTitleSpinnerSourcePredecessor,readReaderUxFixpassAssociation,restoreReaderUxFixpassPredecessor} from './reviewed-six-persona-source-successor.mjs';
import {composed} from './composed-account-library-fixture.mjs';
const root=fileURLToPath(new URL('../../../',import.meta.url)),hash=bytes=>createHash('sha256').update(bytes).digest('hex');
test('bounded badge state precedence changes one exact expression and preserves guarded fallback',async()=>{
 const entry=readCompletionManifest[0],current=await composed(entry.path),previous=restoreReadCompletionOutput(current,entry.path);
 assert.equal(hash(previous),entry.predecessor_sha256);assert.equal(hash(current),entry.output_sha256);assert.equal(applyReadCompletionOutput(previous,entry.path),current);
 assert.equal(current.replace(readCompletionPatches[0].after,readCompletionPatches[0].before),previous);
 assert.match(current,/visibleBadges \? visibleBadges.value\(\$badgeState, chapter.id\) : \$localLibraryState.mode === "account" && historyObservationSession !== \$Session \? undefined :/);
 for(const value of [previous,current+' ',current.slice(1)])assert.throws(()=>restoreReadCompletionOutput(value,entry.path));
 assert.throws(()=>applyReadCompletionOutput(current,entry.path));assert.equal(applyReadCompletionOutput('unrelated','other.svelte'),'unrelated');
});
test('read state registry rejects changed or extra exact exceptions',()=>{
 const old=structuredClone(readCompletionPatches),restore=()=>readCompletionPatches.splice(0,readCompletionPatches.length,...structuredClone(old));try{for(const mutate of [()=>readCompletionPatches.pop(),()=>readCompletionPatches.push(old[0]),()=>readCompletionPatches[0].after+='x']){mutate();assert.throws(assertReadCompletionScope);restore();}}finally{restore();}assertReadCompletionScope();
});
test('reader followup binds exact restored baseline sources and restores every immutable predecessor without inherited qualification',()=>{
 const a=readReaderFollowupSourceAssociation(),prior=readTitleSpinnerSourceAssociation();assert.equal(a.predecessor_association_sha256,'db72f81d232b9ab4da1a7f9b2f314bda81ac1ff95342e0d9bd36448ed8704979');assert.equal(prior.rows.length,25);assert.equal(readReaderUxFixpassAssociation().rows.length,157);assert.equal(a.browser_executed,false);assert.equal(a.http_executed,false);assert.equal(hash(fs.readFileSync(path.join(root,a.predecessor_association_path))),a.predecessor_association_sha256);
 for(const row of a.rows){const source=fs.readFileSync(path.join(root,row.path)),previous=restoreReaderFollowupSourcePredecessor(row.path,source);assert.equal(hash(readCardSpinnerSourceAssociation().rows.some(next=>next.path===row.path)?restoreCardSpinnerSourcePredecessor(row.path,source):readSixPersonaSourceAssociation().rows.some(next=>next.path===row.path)?restoreSixPersonaSourcePredecessor(row.path,source):source),row.current_sha256);if(row.added)assert.equal(previous,null);else{assert.equal(hash(previous),row.previous_sha256);const frozen=prior.rows.find(x=>x.path===row.path);if(frozen)assert.equal(hash(previous),frozen.current_sha256);}for(const bad of [source.subarray(1),Buffer.concat([source,Buffer.from(' ')]),null,{}])assert.throws(()=>restoreReaderFollowupSourcePredecessor(row.path,bad));}
 for(const row of prior.rows)restoreTitleSpinnerSourcePredecessor(row.path,fs.readFileSync(path.join(root,row.path)));
 for(const row of readReaderUxFixpassAssociation().rows)restoreReaderUxFixpassPredecessor(row.path,fs.readFileSync(path.join(root,row.path)));
 const changed=structuredClone(a);changed.rows.reverse();assert.throws(()=>readReaderFollowupSourceAssociation(Buffer.from(JSON.stringify(changed))));assert.throws(()=>restoreReaderFollowupSourcePredecessor('unknown',Buffer.from('x')));
});
