import assert from 'node:assert/strict';
import test from 'node:test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { errorFacts, emptyDiagnostic, collectHostFacts, readBounded, MESSAGE_LIMIT } from '../../scripts/ci/sandbox-launch-diagnostics.mjs';
import { probe, initialRecord } from '../../scripts/ci/sandbox-preflight.mjs';
const privateText = 'https://synthetic.invalid/private?token=never-export';
test('schema2 adds mandatory closed diagnostics without raw values', () => {
  const value=initialRecord(); assert.equal(value.schema,2); assert.deepEqual(value.launch_diagnostic,emptyDiagnostic());
  assert.doesNotMatch(JSON.stringify(value),/synthetic|token|\/proc\//);
});
test('known launch error families remain fixed and combined evidence stays visible', () => {
  const rows=[['Chromium sandboxing failed!',undefined,'sandbox-refused'],['error while loading shared libraries: '+privateText,undefined,'missing-runtime-library'],['ordinary', 'ENOENT','spawn-not-found'],['ordinary','EACCES','permission-refused'],['ordinary','EPERM','permission-refused'],['ordinary','ENOEXEC','executable-format'],['ordinary','ENOMEM','resource-unavailable'],['ordinary','ENOSPC','resource-unavailable'],['ordinary','ETIMEDOUT','launch-timeout'],['ordinary','EPIPE','transport-closed'],['ordinary','ECONNRESET','transport-closed'],['Failed to launch the browser process.',undefined,'launch-closed-before-ready'],['spawn '+privateText+' ENOENT',undefined,'spawn-not-found']];
  for(const [message,code,reason]of rows){const value=errorFacts('launching-browser',{message,code});assert.equal(value.reason,reason);assert(!JSON.stringify(value).includes(privateText));}
  const mixed=errorFacts('launching-browser',{message:'Chromium sandboxing failed!\nFailed to launch the browser process.'});assert.equal(mixed.reason,'multiple-known-families');assert.equal(mixed.signatures.sandbox,true);assert.equal(mixed.signatures.premature_exit,true);
  const conflict=errorFacts('launching-browser',{message:'spawn '+privateText+' ENOENT',code:'EACCES'});assert.equal(conflict.code_source,'conflict');assert.equal(conflict.code,'unknown');assert.equal(conflict.reason,'multiple-known-families');
});
test('code and message getters are captured once including the actual probe catch', async()=>{
  const reads={message:0,name:0,code:0};const error={get message(){reads.message++;return reads.message===1?'Chromium sandboxing failed!':privateText;},get name(){reads.name++;return 'Error';},get code(){reads.code++;return undefined;}};
  const value=await probe({executablePath:'/synthetic',chromium:{async launch(){throw error;}}});
  assert.deepEqual(reads,{message:1,name:1,code:1});assert.equal(value.caught_category,'sandbox-refused');assert.equal(value.launch_diagnostic.error.signatures.sandbox,true);assert(!JSON.stringify(value).includes(privateText));
});
test('hostile getters, nonstrings and near-miss codes never stringify or leak',()=>{
  const hostile=new Proxy({}, {get(){throw Error(privateText);}});assert.equal(errorFacts('launching-browser',hostile).reason,'unavailable');
  for(const code of ['enoent','ENOENT '+privateText,{},1,true]){const value=errorFacts('launching-browser',{message:privateText,code});assert.equal(value.code,'unknown');assert.equal(value.reason,'unknown');assert(!JSON.stringify(value).includes(privateText));}
  let touched=0;errorFacts('context-created',{get message(){touched++;throw Error(privateText);}});assert.equal(touched,0);
});
test('scanned messages are capped by actual UTF8 bytes including astral text',()=>{
  for(const text of ['x'.repeat(MESSAGE_LIMIT),'🙂'.repeat(MESSAGE_LIMIT/4)])assert.equal(errorFacts('launching-browser',{message:text}).message_state,'bounded');
  for(const text of ['x'.repeat(MESSAGE_LIMIT+1),'🙂'.repeat(MESSAGE_LIMIT/4+1),'界'.repeat(22000)]){const value=errorFacts('launching-browser',{message:text});assert.equal(value.message_state,'unavailable');assert.equal(value.reason,'unavailable');assert(Object.values(value.signatures).every(x=>x===null));}
});
test('timeout identity and unknown facts are explicit instead of fabricated causes',()=>{
  assert.equal(errorFacts('launching-browser',{name:'TimeoutError'}).reason,'launch-timeout');assert.equal(errorFacts('launching-browser',{message:'ordinary private failure'}).reason,'unknown');assert.equal(errorFacts('launching-browser',null).reason,'unavailable');
});
test('fixed host reads export only closed facts and preserve unavailable',()=>{
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'closed-host-'));try{
    const exe=path.join(root,'exe');fs.writeFileSync(exe,'inert fixture',{mode:0o700});
    const files=new Map([['/proc/sys/kernel/unprivileged_userns_clone','1\n'],['/proc/sys/user/max_user_namespaces','0\n'],['/proc/sys/kernel/apparmor_restrict_unprivileged_userns','1\n'],['/proc/self/status','Name:\tPRIVATE\nSeccomp:\t2\n']]);
    const mapping=new Map();for(const [name,content]of files){const file=path.join(root,'policy'+mapping.size);fs.writeFileSync(file,content);mapping.set(name,file);}
    const io=Object.create(fs);io.openSync=(file,...args)=>fs.openSync(mapping.get(file)??file,...args);
    const value=collectHostFacts(exe,root,io);assert.deepEqual(value,{executable:'ready',tmp:'ready',userns_clone:'enabled',userns_max:'zero',apparmor_userns:'enabled',seccomp:'filter'});assert(!JSON.stringify(value).includes(root));assert(!JSON.stringify(value).includes('PRIVATE'));
    fs.writeFileSync(mapping.get('/proc/self/status'),'Seccomp:\n2\n');assert.equal(collectHostFacts(exe,root,io).seccomp,'unavailable');
    io.openSync=()=>{throw Error(privateText);};const refused=collectHostFacts(exe,root,io);assert.equal(refused.userns_clone,'unavailable');assert.equal(refused.seccomp,'unavailable');
  }finally{fs.rmSync(root,{recursive:true});}
});
test('bounded host reader refuses symlinks nonregular and oversize data',()=>{
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'closed-reader-'));try{
    const file=path.join(root,'policy');fs.writeFileSync(file,'x'.repeat(65));assert.throws(()=>readBounded(file,64));
    const link=path.join(root,'link');fs.symlinkSync(file,link);assert.throws(()=>readBounded(link,64));assert.throws(()=>readBounded(root,64));
  }finally{fs.rmSync(root,{recursive:true});}
});
