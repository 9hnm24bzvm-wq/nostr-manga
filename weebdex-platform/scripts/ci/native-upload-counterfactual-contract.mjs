// Standalone diagnostic contract. Does not change product or observer semantics.
export const CASES = Object.freeze(['complete-eof', 'truncated-valid-json', 'owner-dispose-checkpoint']);
export const PREFLIGHT_STAGES=Object.freeze(['not-run','identity','command-introspection','sandbox','protocol','complete']);
export const VERSIONS = Object.freeze(['156.0.8078.47', '157.0.8103.6']);
export const LIMITS = Object.freeze({ aggregate: 300000, case: 20000, reserve: 4000, report: 65536, observations: 32 });
export const BODY = '{"result":"error"}';
export const ACTOR='b'.repeat(64), SERVICE='a'.repeat(64), USER='33333333-3333-8333-8333-333333333333', NAMESPACE='66666666-6666-8666-8666-666666666666';
export const account = Object.freeze({result:'ok',user:{id:USER,username:'synthetic',version:7,roles:[],permissions:['chapter.create','self.chapter.update']},nostr:{pubkey:ACTOR,serviceKey:SERVICE,catalogNamespace:NAMESPACE,authentication:'NIP-98',requestScoped:true,nativeSessionAuthority:false,catalogueScope:'instance-local'},upload:{canBegin:true,canManage:true,conditional:true,publication:{configured:true,requiresOperatorOrAuthorizedGroup:true,scope:'original-protocol'},limits:{maxFiles:500,maxFileBytes:26214400}}});
export const insist = value => { if (!value) throw Error('Counterfactual contract failed'); };
export const errorClass = value => value==='net::ERR_ABORTED'?'aborted':value==='net::ERR_CONTENT_LENGTH_MISMATCH'?'length-mismatch':'other';
// Pinned Linux chrome://sandbox uses ordinary two-cell table rows. Require
// the exact seccomp row, not the separate TSYNC capability row, and Chromium's
// complete first-layer plus second-layer evaluation. Unknown layouts fail shut.
export function sandboxStatusOkay(text) {
  if(typeof text!=='string'||text.length>8192)return false;
  const lines=text.replace(/\r\n/g,'\n').split('\n').map(line=>line.trim());
  if(lines.length>64)return false;
  const evaluations=lines.filter(line=>line.startsWith('You are '));
  const seccomp=lines.filter(line=>/^Seccomp-BPF sandbox(?:[ \t]|$)/.test(line)&&!line.startsWith('Seccomp-BPF sandbox supports TSYNC'));
  return evaluations.length===1&&evaluations[0]==='You are adequately sandboxed.'&&seccomp.length===1&&/^Seccomp-BPF sandbox[ \t]+Yes$/.test(seccomp[0]);
}
export function expectedCase(row) {
  if (!row.infrastructure || !row.cleanup || !row.correlated || !row.observer_valid || !row.account_finished || !row.server_terminal_before_cleanup || row.mutations!==0) return false;
  if (row.case==='owner-dispose-checkpoint') return row.application==='rejected' && !row.eof && row.bytes===18 && row.phases===1 && !row.server_finish && row.server_premature && row.cancellation && row.abort_before_read_error && row.native==='failed' && row.error==='aborted' && !row.observer_healthy;
  // The upstream fix also preserves errored-body classification. The old arm
  // may expose that known aborted classification; never rewrite the raw enum.
  if (row.case==='truncated-valid-json') return row.application==='rejected' && !row.eof && row.bytes===18 && row.phases===2 && row.server_finish && !row.server_premature && !row.abort_before_read_error && row.native==='failed' && (row.error==='length-mismatch'||(row.version===VERSIONS[0]&&row.error==='aborted')) && !row.observer_healthy;
  return row.application==='missing' && row.eof && row.body_match && row.body_hash_match && row.bytes===18 && row.phases===2 && row.server_finish && !row.server_premature && !row.cancellation && (row.version===VERSIONS[0] ? row.native==='failed' && row.error==='aborted' && !row.observer_healthy : row.native==='finished' && row.error==='none' && row.observer_healthy);
}
export function emptyCase(version,kind,schema=4) { return {...(schema===4?{account_native:'not-run',account_error:'none',observer_failure_counts:{signed_request_invalid:0,duplicate_native_terminal:0,signed_native_failed:0}}:{}),...(schema>=3?{operation:'not-entered',completed_operation:'not-entered',failure_reason:'none',deadline_elapsed:false}:{}),version,case:kind,application:'not-run',native:'not-run',error:'none',eof:false,body_match:false,body_hash_match:false,bytes:0,reads:0,phases:0,cancellation:false,abort_before_read_error:false,observer_healthy:false,observer_valid:false,correlated:false,account_finished:false,mutations:0,server_finish:false,server_premature:false,server_terminal_before_cleanup:false,infrastructure:false,cleanup:false,expectation:false}; }
export function emptyReport(schema=4) { return {schema,kind:'native-upload-browser-counterfactual',driver:'1.64.1',source_commit:'0'.repeat(40),source_manifest_sha256:'0'.repeat(64),acquisition_manifest_sha256:'0'.repeat(64),sandbox:false,protocol:false,outer_cleanup:false,completed:false,qualified:false,binaries:VERSIONS.map((version,index)=>({version,generation:['1792451271847937','1792024099984701'][index],size:[200206486,202676171][index],archive_sha256:'0'.repeat(64),executable_sha256:'0'.repeat(64),identity:false,sandbox:false,protocol:false,preflight_stage:'not-run'})),cases:VERSIONS.flatMap(version=>CASES.map(kind=>emptyCase(version,kind,schema)))}; }
const sameKeys=(value,keys)=>value&&typeof value==='object'&&!Array.isArray(value)&&Object.keys(value).sort().join('|')===keys.sort().join('|');
export function canonicalReport(report) {
  insist([2,3,4].includes(report.schema));const skeleton=emptyReport(report.schema); insist(sameKeys(report,Object.keys(skeleton)));
  insist(report.kind===skeleton.kind&&report.driver==='1.64.1');
  insist(/^[0-9a-f]{40}$/.test(report.source_commit));
  for (const k of ['source_manifest_sha256','acquisition_manifest_sha256']) insist(/^[0-9a-f]{64}$/.test(report[k]));
  for (const k of ['sandbox','protocol','outer_cleanup','completed','qualified']) insist(typeof report[k]==='boolean');
  insist(Array.isArray(report.binaries)&&report.binaries.length===2);for(let i=0;i<2;i++){const row=report.binaries[i],shape=skeleton.binaries[i];insist(sameKeys(row,Object.keys(shape))&&row.version===shape.version&&row.generation===shape.generation&&row.size===shape.size);for(const k of ['archive_sha256','executable_sha256'])insist(/^[0-9a-f]{64}$/.test(row[k]));for(const k of ['identity','sandbox','protocol'])insist(typeof row[k]==='boolean');insist(PREFLIGHT_STAGES.includes(row.preflight_stage));insist(row.identity===!['not-run','identity'].includes(row.preflight_stage));insist(row.sandbox===['protocol','complete'].includes(row.preflight_stage));if(row.protocol)insist(row.sandbox);if(row.preflight_stage==='complete')insist(row.protocol);}insist(report.sandbox===report.binaries.every(r=>r.sandbox)&&report.protocol===report.binaries.every(r=>r.protocol));insist(Array.isArray(report.cases)&&report.cases.length===6);
  for (let i=0;i<6;i++) { const row=report.cases[i],shape=skeleton.cases[i]; insist(sameKeys(row,Object.keys(shape))&&row.version===shape.version&&row.case===shape.case);
    for (const [k,v] of Object.entries(shape)) if(typeof v==='boolean') insist(typeof row[k]==='boolean');
    for(const k of ['bytes','reads','phases','mutations']) insist(Number.isInteger(row[k])&&row[k]>=0&&row[k]<=32);
    insist(['not-run','pending','missing','rejected'].includes(row.application)); insist(['not-run','pending','finished','failed'].includes(row.native)); insist(['none','aborted','length-mismatch','other'].includes(row.error));
    if(report.schema>=3){
      insist(OPERATIONS.includes(row.operation)&&OPERATIONS.includes(row.completed_operation)&&FAILURE_REASONS.includes(row.failure_reason));
      insist(OPERATIONS.indexOf(row.completed_operation)<=OPERATIONS.indexOf(row.operation));
      if(row.operation==='not-entered')insist(row.completed_operation==='not-entered'&&row.failure_reason==='none'&&!row.deadline_elapsed&&!row.infrastructure);
      if(row.infrastructure)insist(row.operation==='complete'&&row.completed_operation==='complete'&&row.failure_reason==='none'&&!row.deadline_elapsed);
      if(row.failure_reason!=='none')insist(!row.infrastructure&&row.operation!=='not-entered');
    }
    if(report.schema===4){
      insist(['not-run','pending','finished','failed'].includes(row.account_native)&&['none','aborted','length-mismatch','other'].includes(row.account_error));
      insist((row.account_native==='failed')===(row.account_error!=='none'));
      insist(row.account_finished===(row.account_native==='finished'));
      insist(validObserverFailureCounts(row.observer_failure_counts));
    }
    insist(row.expectation===expectedCase(row));
  }
  insist(report.completed===(report.outer_cleanup&&report.cases.every(r=>r.cleanup&&r.infrastructure)));insist(report.qualified===(report.completed&&report.sandbox&&report.protocol&&report.binaries.every(r=>r.identity&&r.preflight_stage==='complete')&&report.cases.every(r=>r.expectation)));
  const bytes=JSON.stringify(report)+'\n'; insist(Buffer.byteLength(bytes)<=LIMITS.report); return bytes;
}

// Closed, sanitized diagnostics. Never carry raw errors, URLs or rejection values.
export const OPERATIONS=Object.freeze(['not-entered','context','binding','page','observer','cdp-session','network-enable','navigation','readiness','readiness-dispose','start','application-terminal','native-terminal','observer-terminal','server-terminal','validation','complete']);
export const FAILURE_REASONS=Object.freeze(['none','operation-failed','snapshot-invalid','instrumentation-invalid','ledger-invalid','bindings-pending','phase-mismatch']);
const boundedCount=value=>Number.isInteger(value)&&value>=0&&value<=32;
function invalidSnapshot(row){if(row.failure_reason==='none')row.failure_reason='snapshot-invalid';insist(false);}
export function recordApplicationSnapshot(row,value){
  const numbers=['bytes','reads','pending','phases','controller','signal','cancel','release','bindings','owned'];
  const booleans=['eof','body_match','body_hash_match','invalid','read_error','abort_before_read_error','ledger_valid'];
  if(!sameKeys(value,[...numbers,...booleans,'application'])||!numbers.every(k=>boundedCount(value[k]))||!booleans.every(k=>typeof value[k]==='boolean')||!['pending','missing','rejected'].includes(value.application))invalidSnapshot(row);
  for(const key of ['application','bytes','reads','eof','body_match','body_hash_match','abort_before_read_error'])row[key]=value[key];
  row.cancellation=value.controller>0||value.signal>0||value.cancel>0;
  return value;
}
export function recordTerminalSnapshot(row,value){
  const accountFields=Object.hasOwn(row,'account_native')?['account_native','account_error','observer_failure_counts']:[];
  const booleans=['account_finished','server_finish','server_premature','server_terminal_before_cleanup','correlated','observer_valid','observer_healthy'];
  if(!sameKeys(value,[...booleans,'native','error','mutations','phases',...accountFields])||!booleans.every(k=>typeof value[k]==='boolean')||!boundedCount(value.mutations)||!boundedCount(value.phases)||!['not-run','pending','finished','failed'].includes(value.native)||!['none','aborted','length-mismatch','other'].includes(value.error))invalidSnapshot(row);
  if(accountFields.length&&(!['not-run','pending','finished','failed'].includes(value.account_native)||!['none','aborted','length-mismatch','other'].includes(value.account_error)||((value.account_native==='failed')!==(value.account_error!=='none'))||value.account_finished!==(value.account_native==='finished')||!validObserverFailureCounts(value.observer_failure_counts)))invalidSnapshot(row);
  Object.assign(row,value);
  if(accountFields.length)row.observer_failure_counts=Object.freeze({...value.observer_failure_counts});
}
export function validateCaseObservation(row,last,bindings,phaseCount,kind){
  for(const [okay,reason]of [[!last.invalid,'instrumentation-invalid'],[last.ledger_valid,'ledger-invalid'],[bindings===0,'bindings-pending'],[phaseCount===(kind==='owner-dispose-checkpoint'?1:2),'phase-mismatch']])if(!okay){row.failure_reason=reason;insist(false);}
}

// Failure observations precede cleanup; owned late work must still settle, but
// cannot overwrite the captured failure stage or make infrastructure successful.
export function preserveCaseFailure(row){const {cleanup,expectation,...values}=row;if(Object.hasOwn(values,'observer_failure_counts'))values.observer_failure_counts=Object.freeze({...values.observer_failure_counts});const snapshot=Object.freeze(values);return()=>Object.assign(row,snapshot);}

export const OBSERVER_FAILURE_KINDS=Object.freeze(['signed_request_invalid','duplicate_native_terminal','signed_native_failed']);
function validObserverFailureCounts(value){return sameKeys(value,[...OBSERVER_FAILURE_KINDS])&&OBSERVER_FAILURE_KINDS.every(key=>Number.isInteger(value[key])&&value[key]>=0&&value[key]<=16)&&OBSERVER_FAILURE_KINDS.reduce((sum,key)=>sum+value[key],0)<=16;}
export function countObserverFailures(failures){
  insist(Array.isArray(failures)&&failures.length<=16&&failures.every(value=>OBSERVER_FAILURE_KINDS.includes(value)));
  const counts={signed_request_invalid:0,duplicate_native_terminal:0,signed_native_failed:0};
  for(const value of failures)counts[value]++;
  return counts;
}
