<script lang="ts">
 import {reviewFields,reviewTimeline} from './draft-feedback';
 import UnsavedEditorGuard from './UnsavedEditorGuard.svelte';
 import {allowEditorDiscard} from './editor-unsaved';
 import AccountActionGate from './AccountActionGate.svelte';
 import { Session } from '$lib/stores';
 import { Locale } from '$lib';
 import {canonicalLanguageOptions,languageLabel} from './language';
 import { nativeAccountIssue, type NativeAccountIssue } from './native-account-scope';
 import { onDestroy, onMount, untrack } from 'svelte';
 import { createSessionTitleController, titleAwaitingProjection, type SignedTitleController, type TitleAction } from './session-title-client';
 import PublicCoverEditor from './PublicCoverEditor.svelte';
 import PublicTitleDisposition from './PublicTitleDisposition.svelte';
 import PublicRelationEditor from './PublicRelationEditor.svelte';
 import type { MangaView } from './types';
 import PublicEntityPicker from './PublicEntityPicker.svelte';
 import {titleEditAffordances,richTitleFields,richTitlePayload,titleFieldsChanged,titleLinks,type RichTitleFields} from './editor-fields';
 let { data }: { data?: { manga?: { id:string }; covers?: unknown[] } } = $props();
 const captured=untrack(()=>data?.manga?.id),path=window.location.pathname;
 let owner=$state<SignedTitleController>();
 let disposed=false,generation=0;let capturedSession=$state.raw(untrack(()=>$Session)),issue=$state<NativeAccountIssue>();
 let rejectionReason=$state(''),affectedFields=$state<string[]>([]);
 let current=$state<MangaView>(),ready=$state(false),busy=$state(false),pending=$state(false),message=$state('');
 const history=$derived(reviewTimeline(current?.public_api.resource.attributes.reviewTimeline));
 const languages=canonicalLanguageOptions(Locale.Languages);
 let customLanguage=$state(false);
 let title=$state(''),language=$state('en'),description=$state(''),status=$state('ongoing'),rating=$state('safe');
 let fields=$state<RichTitleFields>(richTitleFields());
 let coverPending=$state(false),coverBusy=$state(false),dispositionBusy=$state(false),dispositionPending=$state(false),removed=$state(false);
 let relationBusy=$state(false),relationPending=$state(false),relationDirty=$state(false),coverDirty=$state(false);
 let permissions=$state<string[]>([]),roles=$state<string[]>([]);
 const active=()=>{if(disposed||$Session!==capturedSession||window.location.pathname!==path||data?.manga?.id!==captured)throw Error('The title editor changed. Reopen the current route.');};
 onDestroy(()=>{disposed=true;owner?.dispose();});
 const access=$derived(titleEditAffordances(current,{id:owner?.account.id??'',permissions,roles}));
 const canWrite=$derived(access.write);
 const canEditLinks=$derived(access.links);
 const canReview=$derived(access.review);
 const metadataDirty=$derived(current?(title!==originalText(current,'title')||description!==originalText(current,'description')||language!==current.language||status!==current.status||rating!==current.content_rating||titleFieldsChanged(fields,current)):(title!==''||description!==''||language!=='en'||status!=='ongoing'||rating!=='safe'||titleFieldsChanged(fields)));
 const awaitingProjection=$derived(titleAwaitingProjection(current));
 const hasCover=$derived(!!current?.relationships.cover&&current.relationships.cover.language===current.language);
 function originalText(value:MangaView,key:'title'|'description'){const map=value.public_api.resource.attributes[key];return map&&typeof map==='object'&&value.language in map&&typeof (map as Record<string,unknown>)[value.language]==='string'?(map as Record<string,string>)[value.language]:'';}
 function restore(value:MangaView){customLanguage=false;current=value;title=originalText(value,'title');language=value.language;description=originalText(value,'description');status=value.status;rating=value.content_rating;fields=richTitleFields(value);}
 function failure(error:unknown){return error instanceof Error?error.message:'The title command was not confirmed.';}
 function responseError(response:{status:number;body?:unknown}){const body=response.body as {errors?:{detail?:string}[]}|undefined;return body?.errors?.[0]?.detail??`Title request refused (HTTP ${response.status})`;}
 let initializing = $state(false);
 async function initialize(){if(initializing||disposed||!$Session?.identity?.id)return;capturedSession=$Session;const session=capturedSession,token=generation,check=()=>{active();if(generation!==token||$Session!==session)throw Error('This account view changed.');};initializing=true;issue=undefined;message='';try{const next=await createSessionTitleController(session.identity.id,check,captured);try{active();if(token!==generation)throw Error('This account view changed.');}catch(error){next.dispose();throw error;}owner=next;permissions=[...owner.account.permissions];roles=[...owner.account.roles];pending=owner.pending();if(captured){const response=await owner.read();active();if(response.status!==200||!response.body)throw Error(responseError(response));restore(response.body);}ready=true;if(pending)message='A previous title command has an unconfirmed outcome. It will not be replayed. Review the actual account drafts or contact the instance operator before further changes.';}catch(error){if(!disposed&&token===generation){issue=nativeAccountIssue(error);message=failure(error);}}finally{if(!disposed&&token===generation)initializing=false;}}
 $effect(()=>{const session=$Session;untrack(()=>{generation++;owner?.dispose();owner=undefined;current=undefined;ready=false;initializing=false;busy=false;pending=false;permissions=[];roles=[];customLanguage=false;language='en';title='';description='';status='ongoing';rating='safe';rejectionReason='';affectedFields=[];fields=richTitleFields();coverBusy=false;coverPending=false;relationBusy=false;relationPending=false;relationDirty=false;coverDirty=false;dispositionBusy=false;dispositionPending=false;removed=false;message='';issue=undefined;capturedSession=session;if(session?.identity?.id)void initialize();});});
 async function command(action:TitleAction){
  if(!owner||!ready||busy||pending||coverBusy||coverPending||coverDirty||dispositionBusy||dispositionPending||relationBusy||relationPending||relationDirty||removed)return;
  if(metadataDirty&&action!=='create'&&action!=='update'){message='Save metadata changes before reviewing this title.';return;}
  if(action==='create'||action==='update'){if(!canWrite){message='You do not have permission to edit this title.';return;}}
  else if(action==='submit'?!access.submit:!canReview){message='You do not have permission for this title transition.';return;}
  const operationOwner=owner,token=generation;busy=true;message='';
  try{
   let payload:Record<string,unknown>|undefined;
   if(action==='create')payload={title,language,description,status,content_rating:rating,...richTitlePayload(fields)};
   else if(action==='update'){
    if(!current)throw Error('Read the current title first.');
    payload={version:current.version,...richTitlePayload(fields,current)};
    if(title!==originalText(current,'title'))payload.title=title;
    if(description!==originalText(current,'description'))payload.description=description;
    if(language!==current.language)payload.language=language;
    if(status!==current.status)payload.status=status;
    if(rating!==current.content_rating)payload.content_rating=rating;
    if(Object.keys(payload).length===1){message='No metadata changes to save.';return;}
   }
   if(action==='reject')payload={reason:rejectionReason,affectedFields};
   if(payload){for(const [keys,allowed] of [[['title','alt_titles'],access.title],[['authors'],access.authors],[['artists'],access.artists],[['tags'],access.tags]] as const)if(!allowed&&keys.some(key=>Object.hasOwn(payload!,key)))throw Error('You do not have permission to edit that title field.');}
   if(payload&&(Object.hasOwn(payload,'links')||Object.hasOwn(payload,'officialLinks'))&&!canEditLinks)throw Error('You do not have permission to edit links.');
   const response=await owner.mutate(action,payload);active();
   if(response.status>=400||!response.body)throw Error(responseError(response));
   restore(response.body);
   if(action==='reject'){rejectionReason='';affectedFields=[];}
   const source=response.body.public_api.resource.attributes.nostr as {publicationStatus?:string;publicationScope?:string;metadataScope?:string}|undefined;
   message=action==='publish'?(source?.publicationStatus==='accepted_pending_projection'?'The original signed title was accepted and is waiting for actual local source projection. Public detail is not readable yet.':source?.publicationScope==='original-protocol'?'The original signed title publication receipt was accepted. Public readability remains a separate observation.':'Title published on this instance. No original-protocol publication is claimed.'):
    action==='submit'?'Draft submitted for instance review.':action==='reject'?'Draft rejected by the instance reviewer.':action==='create'?'Draft saved. Add a cover in the original language before submitting for review.':'Changes saved on this site. Original publications are unchanged.';
  }catch(error){if(!disposed&&token===generation&&owner===operationOwner)message=failure(error);}
  finally{if(!disposed&&token===generation&&owner===operationOwner){busy=false;pending=owner?.pending()??pending;if(owner){permissions=[...owner.account.permissions];roles=[...owner.account.roles];}}}
 }
 async function reload(){if(!owner||busy||coverBusy||coverDirty||dispositionBusy||relationBusy||relationDirty||removed||!current)return;if(metadataDirty&&!confirm('Discard unsaved title metadata changes and reload the actual server record?'))return;const operationOwner=owner,token=generation;busy=true;try{const response=await owner.read();active();if(response.status!==200||!response.body)throw Error(responseError(response));restore(response.body);message='Current title read from the server. No mutation was replayed; an unconfirmed command remains fenced.';}catch(error){if(!disposed&&token===generation&&owner===operationOwner)message=failure(error);}finally{if(!disposed&&token===generation&&owner===operationOwner){busy=false;pending=owner?.pending()??pending;if(owner){permissions=[...owner.account.permissions];roles=[...owner.account.roles];}}}}
</script>
<UnsavedEditorGuard dirty={ready&&(metadataDirty||!!rejectionReason.trim()||affectedFields.length>0)} />
<section class="space-y-4 rounded border p-4">
 <h3 class="text-lg font-medium">Title details</h3>
 <p>Changes apply to this site. Original publications remain unchanged.</p>
 <p>Save your title details first, then add a cover and related titles below.</p>
 {#if message}<p role="status" class="rounded border p-3">{message}</p>{/if}
 <AccountActionGate {issue} onready={initialize} />
 {#if !ready && $Session?.identity?.id}<button type="button" class="rounded border px-3 py-2" disabled={initializing} onclick={() => { void initialize(); }}>Check my access again</button>{/if}
 {#if removed}<p role="status">This title was soft-hidden on this instance. Original signed publications are unchanged. Reopen the catalogue to continue.</p>{/if}
 {#if metadataDirty}<p>Save metadata changes before cover, relation, review or instance title actions.</p>{/if}
 {#if coverDirty||relationDirty}<p>Save or discard unsaved cover or relation changes before editing title details or reviewing the draft.</p>{/if}
 {#if current}<p>Status: {current.state}.</p>{/if}
 {#if awaitingProjection}<p>This accepted original publication is read-only while local source projection is pending. Reload title details to observe actual public visibility.</p>{/if}
 <form onsubmit={event=>{event.preventDefault();void command(current?'update':'create');}} class="space-y-3">
  <fieldset disabled={!ready||busy||pending||coverBusy||coverPending||coverDirty||dispositionBusy||dispositionPending||relationBusy||relationPending||relationDirty||removed||awaitingProjection||!canWrite} class="space-y-3">
   <label class="block">Original-language title<input class="block w-full rounded border p-2" bind:value={title} disabled={!access.title} maxlength="500" required={!current} /></label>
   <label class="block">Original language<select class="block w-full rounded border p-2" value={customLanguage?'__other__':language} onchange={event=>{const value=event.currentTarget.value;customLanguage=value==='__other__';language=customLanguage?'':value;}}>
    {#if !customLanguage&&!languages.some(([code])=>code===language)}<option value={language}>{languageLabel(language,Locale.Languages)}</option>{/if}
    {#each languages as [code,name]}<option value={code}>{name}</option>{/each}<option value="__other__">Other language…</option>
   </select></label>
   {#if customLanguage}<label class="block">Other original language code<input class="block w-full rounded border p-2" bind:value={language} required placeholder="For example, ta or pt-br" /></label>{/if}
   <label class="block">Description<textarea class="block w-full rounded border p-2" bind:value={description} maxlength="30000"></textarea></label>
   <label class="block">Publication status<select class="block rounded border p-2" bind:value={status}><option value="ongoing">Ongoing</option><option value="completed">Completed</option><option value="hiatus">Hiatus</option><option value="cancelled">Cancelled</option></select></label>
   <label class="block">Content rating<select class="block rounded border p-2" bind:value={rating}><option value="safe">Safe</option><option value="suggestive">Suggestive</option><option value="erotica">Erotica</option><option value="pornographic">Pornographic</option></select></label>
   <fieldset disabled={!access.title} class="space-y-2 rounded border p-3"><legend class="font-medium">Alternate titles</legend>
    {#each fields.altTitles as alternate,index}<div class="flex flex-wrap gap-2"><label>Language<select class="block rounded border p-2" bind:value={alternate.language} required>{#if !languages.some(([code])=>code===alternate.language)}<option value={alternate.language}>{languageLabel(alternate.language,Locale.Languages)||'Choose language'}</option>{/if}{#each languages as [code,label]}<option value={code}>{label}</option>{/each}</select></label><label class="flex-1">Alternate title<input class="block w-full rounded border p-2" bind:value={alternate.text} maxlength="500" required /></label><button type="button" class="rounded border px-3 py-1" onclick={()=>{fields.altTitles=fields.altTitles.filter((_,i)=>i!==index);}}>Remove alternate title</button></div>{/each}
    <button type="button" class="rounded border px-3 py-1" disabled={fields.altTitles.length>=100} onclick={()=>{fields.altTitles=[...fields.altTitles,{language,text:''}];}}>Add alternate title</button>
   </fieldset>
   <div class="grid gap-3 sm:grid-cols-2">
    <label>Publication year<input class="block w-full rounded border p-2" type="text" inputmode="numeric" pattern={'[1-9][0-9]{0,3}'} bind:value={fields.year} maxlength="4" placeholder="Unknown" /></label>
    <label>Demographic<select class="block w-full rounded border p-2" bind:value={fields.demographic}><option value="none">Unspecified</option><option value="shounen">Shounen</option><option value="shoujo">Shoujo</option><option value="seinen">Seinen</option><option value="josei">Josei</option></select></label>
    <label>Last volume<input class="block w-full rounded border p-2" bind:value={fields.lastVolume} maxlength="32" /></label>
    <label>Last chapter<input class="block w-full rounded border p-2" bind:value={fields.lastChapter} maxlength="32" /></label>
   </div>
   <label class="block"><input type="checkbox" bind:checked={fields.resetChapters} /> Chapter numbering resets on each new volume</label>
   <PublicEntityPicker label="Authors" bind:selected={fields.authors} search={(query,signal)=>owner!.search('author',query,signal)} disabled={!ready||busy||pending||!access.authors} />
   <PublicEntityPicker label="Artists" bind:selected={fields.artists} search={(query,signal)=>owner!.search('author',query,signal)} disabled={!ready||busy||pending||!access.artists} />
   <PublicEntityPicker label="Tags" bind:selected={fields.tags} search={(query,signal)=>owner!.search('tag',query,signal)} disabled={!ready||busy||pending||!access.tags} />
   <fieldset disabled={!canEditLinks} class="grid gap-3 rounded border p-3 sm:grid-cols-2"><legend class="font-medium">External links</legend>{#each titleLinks as [key,label]}<label>{label}<input class="block w-full rounded border p-2" value={fields.links[key]??''} oninput={event=>{fields.links[key]=event.currentTarget.value;}} maxlength="2048" placeholder={['raw','engtl','amz','ebj','cdj'].includes(key)?'https://…':'Provider ID or URL'} /></label>{/each}<p class="sm:col-span-2 text-sm">Clear a field to remove that link when you save. Other saved links are kept.</p>{#if !canEditLinks}<p>You do not have permission to edit links.</p>{/if}</fieldset>
   <fieldset disabled={!canEditLinks} class="space-y-2 rounded border p-3"><legend class="font-medium">Additional official link information</legend>
    <p>These entries are saved as text, not clickable links. Remove a row to delete it when you save.</p>
    {#each fields.officialLinks as entry,index}<div class="flex flex-wrap gap-2"><label>Label<textarea class="block rounded border p-2" bind:value={entry.key} rows="1" maxlength="100"></textarea></label><label class="flex-1">Text<textarea class="block w-full rounded border p-2" bind:value={entry.value} rows="2" maxlength="2048"></textarea></label><button type="button" class="rounded border px-3 py-1" onclick={()=>{fields.officialLinks=fields.officialLinks.filter((_,i)=>i!==index);}}>Remove official link</button></div>{/each}
    <button type="button" class="rounded border px-3 py-1" disabled={fields.officialLinks.length>=100} onclick={()=>{fields.officialLinks=[...fields.officialLinks,{key:'',value:''}];}}>Add official link</button>
    {#if !canEditLinks}<p>You do not have permission to edit links.</p>{/if}
   </fieldset>
   <button class="rounded border px-4 py-2" type="submit">{current?'Save changes':'Save draft'}</button>
  </fieldset>
 </form>
 {#if current}
  {#if current.state==='rejected'}<p role="status">Changes requested. Read the review below, edit this same draft, then resubmit. Saved covers and related titles stay attached.</p>{/if}
  {#if history.events.length}<section class="space-y-3 rounded border p-3" aria-label="Draft review history"><h2 class="font-medium">Review timeline</h2><ol>{#each history.events as event}<li class="border-b py-2"><strong>{event.action==='rejected'?'Changes requested':event.action==='submitted'?'Submitted for review':'Published'}</strong> · <time datetime={event.createdAt}>{new Date(event.createdAt).toLocaleString()}</time>{#if event.reason}<p>{event.reason}</p>{/if}{#if event.affectedFields.length}<p>Affected: {event.affectedFields.map(field=>reviewFields.find(([key])=>key===field)?.[1]??field).join(', ')}</p>{/if}</li>{/each}</ol>{#if history.hasEarlier}<p>Showing the latest 100 review events.</p>{/if}</section>{/if}
  {#key current.id}<PublicCoverEditor nativeAccount={capturedSession!.identity.id} title={current} accountId={owner!.account.id} disabled={busy||pending||dispositionBusy||dispositionPending||relationBusy||relationPending||relationDirty||removed||metadataDirty||awaitingProjection} assertCurrent={active} onState={state=>{coverPending=state.pending;coverBusy=state.busy;coverDirty=state.dirty??false;}} onChanged={async()=>{const response=await owner!.read();active();if(response.status!==200||!response.body)throw Error(responseError(response));restore(response.body);}} />{/key}
  {#if !removed}{#key current.id}<PublicRelationEditor nativeAccount={capturedSession!.identity.id} title={current} accountId={owner!.account.id} disabled={busy||pending||coverBusy||coverPending||coverDirty||dispositionBusy||dispositionPending||metadataDirty||awaitingProjection} assertCurrent={active} onState={state=>{relationBusy=state.busy;relationPending=state.pending;relationDirty=state.dirty??false;}} onChanged={async()=>{const response=await owner!.read();active();if(response.status!==200||!response.body)throw Error(responseError(response));restore(response.body);}} />{/key}{/if}
  {#if !hasCover&&(current.state==='draft'||current.state==='rejected')}<p>Add a cover in the title’s original language before submitting for review.</p>{/if}
  <div class="flex flex-wrap gap-3">
   {#if current.state==='draft'||current.state==='rejected'}<button class="rounded border px-4 py-2" type="button" disabled={!ready||busy||pending||coverBusy||coverPending||coverDirty||dispositionBusy||dispositionPending||relationBusy||relationPending||relationDirty||removed||metadataDirty||!access.submit||!hasCover} onclick={()=>command('submit')}>{current.state==='rejected'?'Resubmit for review':'Submit for review'}</button>{/if}
   {#if current.state==='submitted'&&canReview}
    <fieldset class="w-full space-y-2 rounded border p-3" disabled={busy||pending}><legend>Request changes</legend><label class="block">What should the author change?<textarea class="block w-full rounded border p-2" bind:value={rejectionReason} maxlength="2000" placeholder="Give a clear, helpful reason visible to the author."></textarea></label><div class="flex flex-wrap gap-3">{#each reviewFields as [field,label]}<label><input type="checkbox" value={field} bind:group={affectedFields} /> {label}</label>{/each}</div><p class="text-sm">This feedback is visible to the author. Do not include private moderation notes.</p></fieldset>
    <button class="rounded border px-4 py-2" type="button" disabled={!ready||busy||pending||coverBusy||coverPending||coverDirty||dispositionBusy||dispositionPending||relationBusy||relationPending||relationDirty||removed||metadataDirty} onclick={()=>command('publish')}>Publish reviewed title</button>
    <button class="rounded border px-4 py-2" type="button" disabled={!ready||busy||pending||coverBusy||coverPending||coverDirty||dispositionBusy||dispositionPending||relationBusy||relationPending||relationDirty||removed||metadataDirty||!rejectionReason.trim()} onclick={()=>command('reject')}>Request changes</button>
   {/if}
   <button class="rounded border px-4 py-2" type="button" disabled={!ready||busy||coverBusy||coverDirty||dispositionBusy||relationBusy||relationDirty||removed} onclick={reload}>Reload title details</button>
  </div>
  {#if !removed}<PublicTitleDisposition title={current} controller={owner!} disabled={busy||pending||coverBusy||coverPending||coverDirty||relationBusy||relationPending||relationDirty||metadataDirty||awaitingProjection} onTitle={restore} onRemoved={()=>{removed=true;}} onState={state=>{dispositionBusy=state.busy;dispositionPending=state.pending;}} />{/if}
  <a class="underline" href={`/title/${current.id}/edit?draft=${current.state!=='published'}`}>Open title editor</a>
 {/if}
</section>
