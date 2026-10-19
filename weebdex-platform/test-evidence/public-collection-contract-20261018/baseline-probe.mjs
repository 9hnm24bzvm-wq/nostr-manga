// Read-only regression witness against exact published baseline client bytes.
import fs from 'node:fs';import path from 'node:path';import {execFileSync} from 'node:child_process';import {fileURLToPath,pathToFileURL} from 'node:url';import {createHash} from 'node:crypto';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../../..'),relative='weebdex-platform/platform-integrations/public-api/client.ts',target=path.join(root,relative);
const baseline=execFileSync('git',['show','be283b664f699e8c7175f6eac0200cd22e212c8a:'+relative],{cwd:root,encoding:'utf8'});
console.log(JSON.stringify({scope:'baseline-regression-only',source:'be283b664f699e8c7175f6eac0200cd22e212c8a',clientSha256:createHash('sha256').update(baseline).digest('hex'),productFilesEdited:false}));
const original=fs.readFileSync;fs.readFileSync=function(file,...args){return typeof file==='string'&&path.resolve(file)===target?baseline:original.call(this,file,...args);};
await import(pathToFileURL(path.join(root,'weebdex-platform/tests/ci/test_public_collection_contract.mjs')).href);
