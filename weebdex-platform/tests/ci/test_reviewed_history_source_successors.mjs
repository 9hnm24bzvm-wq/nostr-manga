import {localPageDriverPath,restoreLocalPageDriverSource} from './reviewed-local-page-driver-source-successor.mjs';
import {restoreBoundedReaderSource} from './reviewed-bounded-reader-source-successors.mjs';
import {integrationSuccessorPaths,restoreIntegratedSource} from './reviewed-integration-source-successors.mjs';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import {createHash} from 'node:crypto';
import {readHistoryAssociation,historySuccessorPaths,restoreHistorySource} from './reviewed-history-source-successors.mjs';
const root=new URL('../../../',import.meta.url),metadata=fs.readFileSync(new URL('../../docs/fixtures/device-history-source-association.json',import.meta.url));
const hash=x=>createHash('sha256').update(x).digest('hex');
test('history successor pins three product and one comparison-driver exact reviewed comparisons without execution claims',()=>{
 const data=readHistoryAssociation();assert.equal(data.reviewed_product,'eeafdcdad2abcef4cc96bb11086c89da774942d3');assert.equal(data.browser_executed,false);assert.equal(data.capacity_qualified,false);
 assert.deepEqual(historySuccessorPaths,['overlays.mjs','library.ts','ChapterHistory.ts'].map(x=>'weebdex-platform/platform-integrations/local-library/'+x).concat('weebdex-platform/tests/ci/test_signed_feature_source_pins.py'));
 for(const row of data.rows){const physical=fs.readFileSync(new URL(row.path,root)),associated=row.path===localPageDriverPath?restoreLocalPageDriverSource(row.path,physical):physical,source=row.path==='weebdex-platform/platform-integrations/local-library/library.ts'?restoreBoundedReaderSource('bounded-reader-to-frozen-main:platform-integrations/local-library/library.ts',row.path,associated):integrationSuccessorPaths.includes(row.path)?restoreIntegratedSource(row.path,associated):associated;const restored=restoreHistorySource(row.path,source);assert.equal(hash(restored),row.previous_sha256);assert.equal(Buffer.byteLength(restored),row.previous_bytes);
  for(const bad of [Buffer.concat([source,Buffer.from(' ')]),Buffer.from(restored),source.subarray(1)])assert.throws(()=>restoreHistorySource(row.path,bad));
  for(const edit of row.literal_reversals)assert.throws(()=>restoreHistorySource(row.path,source+edit.current));
  assert.throws(()=>restoreHistorySource(row.path+'unknown',source));
  assert.deepEqual(fs.readFileSync(new URL(row.path,root)),physical);
 }
});
test('all association bytes and claims fail closed on mutation',()=>{
 assert.throws(()=>readHistoryAssociation(Buffer.concat([metadata,Buffer.from(' ')])));
 for(const key of Object.keys(readHistoryAssociation())){const changed=readHistoryAssociation();delete changed[key];assert.throws(()=>readHistoryAssociation(Buffer.from(JSON.stringify(changed))));}
 for(const mutate of [x=>x.rows.reverse(),x=>x.rows.pop(),x=>x.rows[0].literal_reversals.reverse(),x=>x.browser_executed=true,x=>x.capacity_qualified=true]){const changed=readHistoryAssociation();mutate(changed);assert.throws(()=>readHistoryAssociation(Buffer.from(JSON.stringify(changed))));}
});
