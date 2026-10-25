// Bounded source contracts and exact frozen pre-feature adapter counterfactual.
// These are not a Rust compiler, native HTTP or concurrency execution claim.
import fs from 'node:fs';import path from 'node:path';import vm from 'node:vm';import {stripTypeScriptTypes} from 'node:module';import {createHash} from 'node:crypto';import test from 'node:test';import assert from 'node:assert/strict';
import {root,fixture,id,loadSignedFixtureModule} from './report-draft-fixture.mjs';
const shared=await loadSignedFixtureModule('native-user-forum/signed-client');
const current=await loadSignedFixtureModule('native-forum/signed-client');
const oldSource="// Native catalogue discussions use the reviewed common account/request/CAS core.\n// A kind/UUID/actual-thread tuple is not protocol publication or cookie authority.\nimport {createSignedDiscussion,type NativeDiscussionKind} from '../native-user-forum/signed-client';\nimport type {TitleDependencies} from '../title-client';\nexport type {DiscussionIntent,SignedDiscussionView,NativeDiscussionKind} from '../native-user-forum/signed-client';\nexport {discussionBody} from '../native-user-forum/signed-client';\nexport async function createSignedNativeDiscussion(kind:NativeDiscussionKind,targetId:string,threadId:string,assertCurrent=()=>{},injected:Partial<TitleDependencies>={},signal?:AbortSignal){\n const {reports:_reports,...client}=await createSignedDiscussion(kind,targetId,threadId,assertCurrent,injected,signal);\n return client;\n}\n";
assert.equal(createHash('sha256').update(oldSource).digest('hex'),'1bd85edd18a36817b2183d271c388b6e8a9b69634f9d82b33b30d49da143262e');
for(const kind of ['manga','chapter','group'])test('frozen pre-feature '+kind+' adapter cannot satisfy native report controller acceptance',async()=>{
 const context=vm.createContext({}),module=new vm.SourceTextModule(stripTypeScriptTypes(oldSource,{mode:'transform'}),{context});
 const imports=new vm.SyntheticModule(['createSignedDiscussion','discussionBody'],function(){this.setExport('createSignedDiscussion',shared.createSignedDiscussion);this.setExport('discussionBody',shared.discussionBody);},{context});
 await module.link(()=>imports);await module.evaluate();const f=fixture(),old=await module.namespace.createSignedNativeDiscussion(kind,id,'ActualStoredThread',f.check,f.deps),next=await current.createSignedNativeDiscussion(kind,id,'ActualStoredThread',f.check,f.deps);
 assert.equal(old.reports,undefined);assert.throws(()=>assert.equal(typeof old.reports?.read,'function'),{code:'ERR_ASSERTION'});assert.equal(typeof next.reports.read,'function');old.dispose();next.dispose();assert.equal(f.calls.filter(row=>row.method==='POST').length,0);
});
test('backend source keeps actor upgrade before actual typed target/reply/reason locks and private atomic core',()=>{
 const source=fs.readFileSync(path.join(root,'backend/src/public_api/native_discussion_reports.rs'),'utf8');
 const create=source.slice(source.indexOf('async fn create('));
 assert.ok(create.indexOf('bound_account_write')<create.indexOf('actor.require_permission("report.create")'));
 assert.ok(create.indexOf('actor.require_permission("report.create")')<create.indexOf('FOR UPDATE'));
 assert.ok(create.indexOf('FOR UPDATE')<create.indexOf('target(&tx, kind, &id'));
 assert.ok(create.indexOf('target(&tx, kind, &id')<create.indexOf('report_subject(&tx'));
 assert.ok(create.indexOf('report_subject(&tx')<create.indexOf('SELECT version FROM platform.report_reasons'));
 assert.ok(create.indexOf('expected_reason_version')<create.indexOf('moderation::create_report'));
 assert.ok(create.indexOf('moderation::create_report')<create.indexOf('tx.commit()'));
 assert.match(source,/require_context_budget\(&result\)/);assert.doesNotMatch(source,/Cookie|session_actor|protocol.*publish|accept_report|refuse_report/);
});
test('generic report source uses only existing visibility witnesses and exact private route exceptions',()=>{
 const target=fs.readFileSync(path.join(root,'backend/src/public_api/native_discussion.rs'),'utf8');
 for(const witness of ['public.events','public.blob_intents','platform.catalog_manga','platform.catalog_chapters','platform.catalog_threads','platform.catalog_groups','platform.catalog_chapter_groups'])assert.ok(target.includes(witness));
 assert.match(target,/FOR SHARE OF cg NOWAIT/);assert.match(target,/FOR SHARE NOWAIT/);assert.match(target,/source_fences\(tx, sources\)/);
 const outer=fs.readFileSync(path.join(root,'backend/src/router.rs'),'utf8');assert.match(outer,/matches!\(kind, "user" \| "manga" \| "chapter" \| "group"\)/);assert.match(outer,/suffix == "discussion\/reports"/);assert.match(outer,/suffix == "discussion\/reports\/context"/);
});
