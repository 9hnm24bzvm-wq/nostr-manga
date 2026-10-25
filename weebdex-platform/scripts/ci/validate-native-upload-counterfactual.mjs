import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {canonicalReport,LIMITS,insist} from './native-upload-counterfactual-contract.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
try{
  insist(process.argv.length===2);
  const directory=path.join(root,'ci-native-upload-counterfactual'),input=path.join(directory,'report.json');
  const stat=fs.lstatSync(input);insist(stat.isFile()&&!stat.isSymbolicLink()&&stat.size<=LIMITS.report);
  const bytes=fs.readFileSync(input);insist(bytes.length===stat.size);
  const canonical=canonicalReport(JSON.parse(bytes.toString('utf8')));
  fs.writeFileSync(path.join(directory,'validated-report.json'),canonical,{flag:'wx'});
  console.log('Counterfactual report validated');
}catch{console.error('Counterfactual report rejected');process.exitCode=1;}
