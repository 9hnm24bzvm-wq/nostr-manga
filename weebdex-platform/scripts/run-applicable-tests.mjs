// Sequential current-source suite. Historical exclusions are preserved verbatim.
import fs from 'node:fs';import path from 'node:path';import {fileURLToPath} from 'node:url';import {spawnSync} from 'node:child_process';import {createHash} from 'node:crypto';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const evidence=process.argv[2];if(!evidence)throw Error('Supply a new evidence directory for this run');
const output=path.resolve(evidence);if(fs.existsSync(output))throw Error('Refusing to overwrite an earlier suite receipt');fs.mkdirSync(output,{recursive:true});
const bytes=fs.readFileSync(path.join(root,'tests/fixtures/qualification/applicable-suite-exclusions-20261028.json'));
const expected='aec018fd7111169b2d7e0350dcc91b0e5e16fe2061d387bee99fc3629addf130';if(createHash('sha256').update(bytes).digest('hex')!==expected)throw Error('Historical suite exclusion inventory changed');
const original=JSON.parse(bytes),excluded=new Set(original.excluded_modules),skip=original.skipped_test_names;
const escape=text=>text.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
const files=fs.readdirSync(path.join(root,'tests/ci')).filter(name=>/^test.*\.mjs$/.test(name)&&!excluded.has(name)).sort().map(name=>'tests/ci/'+name);
const args=['--experimental-vm-modules','--conditions=browser','--test','--test-concurrency=1','--test-skip-pattern=^(?:'+skip.map(escape).join('|')+')$',...files];
const env={...process.env,WEEBDEX_FRONTEND_ROOT:path.join(root,'.build/frontend-ux-final'),WEEBDEX_BROWSER_DOM_NODE_MODULES:path.join(root,'.build/qualified-test-deps-20261028/node_modules'),WEEBDEX_MIGRATION_TEST_NODE_MODULES:path.join(root,'.build/qualified-test-deps-20261028/node_modules'),WEEBDEX_MIGRATION_TEST_NOBLE:path.join(root,'.build/local-library-noble-2.0.1/node_modules/@noble/hashes'),WEEBDEX_SVELTE_REVIEW_RUNTIME_ROOT:path.join(root,'.build/qualified-svelte-review-5.55.1/packages/svelte')};
fs.writeFileSync(path.join(output,'historical-exclusions.json'),bytes);
fs.writeFileSync(path.join(output,'selected-modules.json'),JSON.stringify({qualification:'All current standalone test modules, with the exact preceding qualified historical exclusions. CSS and positional CLI checks run separately. No new exclusion.',files},null,2)+'\n');
const result=spawnSync(process.execPath,args,{cwd:root,env,stdio:'inherit'});
fs.writeFileSync(path.join(output,'terminal.json'),JSON.stringify({exit_code:result.status,signal:result.signal,error:result.error?.message??null,finished_at:new Date().toISOString(),runtime_claim:'Node module/mounted suite only; not real browser or database coverage.'},null,2)+'\n');process.exitCode=result.status??1;
