// Textual inventory regression only; no browser, server or product-coverage claim.
import test from 'node:test';
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const script=fileURLToPath(new URL('../../scripts/audit-public-api-frontend.mjs',import.meta.url));
test('original call inventory recognizes implemented recommendations without pretending raw dispatch is visible-control proof',()=>{
 const report=JSON.parse(execFileSync(process.execPath,[script],{encoding:'utf8'}));
 const rows=report.calls.filter(row=>row.family==='Manga'&&row.method==='Recommendations');
 assert.equal(rows.length,1);assert.equal(rows[0].classification,'staged_public_read');
 assert.ok(report.calls.filter(row=>row.method==='Trending').every(row=>row.classification==='explicit_unavailable_adapter'));
 assert.ok(report.limitations.some(value=>value.includes('later overlay may replace the entire visible control')));
 assert.equal(Object.values(report.classifications).reduce((sum,value)=>sum+value,0),report.call_sites);
});
