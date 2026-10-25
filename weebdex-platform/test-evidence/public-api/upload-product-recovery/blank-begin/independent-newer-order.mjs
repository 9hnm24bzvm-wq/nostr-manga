// Finite seams from the actual recovered composition and snapshot/persist bodies.
// Response/save objects below are explicit inputs, not backend or E2E evidence.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {createHash} from 'node:crypto';
const root=new URL('file:///workspace/shared/nostr-manga-recovery-20261017/upload-product-worktree/weebdex-platform/'),read=path=>fs.readFileSync(new URL(path,root),'utf8');
const raw=read('platform-integrations/frontend-overlays.mjs'),start=raw.indexOf('const patches = ')+16,end=raw.indexOf('\n];\nconst hash',start)+2;
assert.ok(start>=16&&end>start);const patches=JSON.parse(raw.slice(start,end));
const helper=read('platform-integrations/frontend-behavior/upload-resume.ts');
const snapshots=helper.slice(helper.indexOf('export const snapshot ='),helper.indexOf('const sameDraft ='))
 .replace('export const snapshot = (draft: ChapterDraft, files: UploadSessionFile[]): UploadSnapshot','const snapshot = (draft, files)')
 .replace('key as keyof ChapterDraft','key').replace('export const signature = (value: UploadSnapshot)','const signature = (value)');
const acknowledge=helper.slice(helper.indexOf('export function acknowledge('),helper.indexOf('export function immutableCommit('))
 .replace('export function acknowledge(session: UploadSession, saved: UploadSession)','function acknowledge(session, saved)').replace(' as const','');
function compose(name){const path='src/lib/components/upload/'+name+'.svelte',original=fs.readFileSync(new URL('frontend/'+path,root));let code=original.toString();for(const patch of patches.filter(p=>p.path===path)){assert.equal(createHash('sha256').update(original).digest('hex'),patch.sha256);if(process.env.UPLOAD_AUTOSAVE_USE_RECOVERED_BASE==='1'&&patch.label.startsWith('recovered upload '))continue;assert.equal(code.split(patch.before).length-1,patch.count,patch.label);code=code.split(patch.before).join(patch.after);}return code;}
function harness({edit=false,local={},save=async(session,value)=>({...session,version:session.version+1,draft:value.draft,page_order:value.page_order})}={}){
 const code=compose(edit?'UploadChapterEdit':'UploadChapter'),beginStart=code.indexOf('\t\t\t\t\tuploadSession = res.body;'),beginEnd=code.indexOf('\r\n\t\t\t\t} else',beginStart);assert.ok(beginStart>=0&&beginEnd>beginStart);const begin=code.slice(beginStart,beginEnd);
 const persistStart=code.indexOf('const persistCurrentUpload = async () => {'),persistEnd=code.indexOf('\r\n\t};',persistStart)+6;assert.ok(persistStart>0&&persistEnd>persistStart);const persist=code.slice(persistStart,persistEnd);
 return new Function('input','save',`${snapshots}\n${acknowledge}\nlet uploadSession,lastSavedUpload='',resumeDisposed=false;const owner={};let draft=structuredClone(input),chapter=draft;const $state={snapshot:structuredClone};const calls=[];const saveResume=(session,value)=>{calls.push(structuredClone(value));return save(session,value);};${persist}\nreturn{begin(res){${begin}},persist:persistCurrentUpload,reorder(){uploadSession.relationships.files.reverse();},change(value){Object.assign(draft,value);},replaceSession(session){uploadSession=session;},dispose(){resumeDisposed=true;},get state(){return{session:structuredClone(uploadSession),draft:structuredClone(draft),lastSavedUpload,dirty:!!uploadSession&&signature(snapshot(draft,uploadSession.relationships.files??[]))!==lastSavedUpload,calls:structuredClone(calls)};}};`)(local,save);
}
const session=(draft={},id='Stage00001')=>({id,version:1,can_mutate:true,draft,relationships:{files:[]},page_order:[]});

for(const edit of [false,true])test('independent held-save retains newer actual file order '+edit,async()=>{let release;const pending=new Promise(r=>release=r),h=harness({edit,local:{language:'en'},save:async(s,v)=>{await pending;return{...s,version:2,draft:v.draft,page_order:v.page_order};}});const returned=session({language:'en'});returned.relationships.files=[{id:'File000001'},{id:'File000002'}];returned.page_order=['File000001','File000002'];h.begin({body:returned});assert.equal(h.state.dirty,false);h.reorder();assert.equal(h.state.dirty,true);const active=h.persist();h.reorder();release();await active;assert.deepEqual(h.state.calls[0].page_order,['File000002','File000001']);assert.deepEqual(h.state.session.page_order,['File000002','File000001']);assert.deepEqual(h.state.session.relationships.files.map(x=>x.id),['File000001','File000002']);assert.equal(h.state.dirty,true);assert.equal(h.state.session.version,2);});
