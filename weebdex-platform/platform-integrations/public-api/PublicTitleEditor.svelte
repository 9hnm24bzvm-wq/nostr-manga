<script lang="ts">
 import IdentityGate from '../local-library/IdentityGate.svelte';
 import { signerIdentity } from '../local-library/publication';
 import { onDestroy, onMount, untrack } from 'svelte';
 import { createSignedTitleController, titleAwaitingProjection, type SignedTitleController, type TitleAction } from './title-client';
 import PublicCoverEditor from './PublicCoverEditor.svelte';
 import PublicTitleDisposition from './PublicTitleDisposition.svelte';
 import PublicRelationEditor from './PublicRelationEditor.svelte';
 import type { MangaView } from './types';
 import PublicEntityPicker from './PublicEntityPicker.svelte';
 import {titleEditAffordances,richTitleFields,richTitlePayload,titleFieldsChanged,titleLinks,type RichTitleFields} from './editor-fields';
 let { data }: { data?: { manga?: { id:string }; covers?: unknown[] } } = $props();
 const captured=untrack(()=>data?.manga?.id),path=window.location.pathname;
 let owner=$state<SignedTitleController>();
 let disposed=false;
 let current=$state<MangaView>(),ready=$state(false),busy=$state(false),pending=$state(false),message=$state('');
 let title=$state(''),language=$state('en'),description=$state(''),status=$state('ongoing'),rating=$state('safe');
 let fields=$state<RichTitleFields>(richTitleFields());
 let coverPending=$state(false),coverBusy=$state(false),dispositionBusy=$state(false),dispositionPending=$state(false),removed=$state(false);
 let relationBusy=$state(false),relationPending=$state(false);
 let permissions=$state<string[]>([]),roles=$state<string[]>([]);
 const active=()=>{if(disposed||window.location.pathname!==path||data?.manga?.id!==captured)throw Error('The title editor changed. Reopen the current route.');};
 onDestroy(()=>{disposed=true;owner?.dispose();});
 const access=$derived(titleEditAffordances(current,{id:owner?.account.id??'',permissions,roles}));
 const canWrite=$derived(access.write);
 const canEditLinks=$derived(access.links);
 const canReview=$derived(access.review);
 const metadataDirty=$derived(!!current&&(title!==originalText(current,'title')||description!==originalText(current,'description')||language!==current.language||status!==current.status||rating!==current.content_rating||titleFieldsChanged(fields,current)));
 const awaitingProjection=$derived(titleAwaitingProjection(current));
 const hasCover=$derived(!!current?.relationships.cover&&current.relationships.cover.language===current.language);
 function originalText(value:MangaView,key:'title'|'description'){const map=value.public_api.resource.attributes[key];return map&&typeof map==='object'&&value.language in map&&typeof (map as Record<string,unknown>)[value.language]==='string'?(map as Record<string,string>)[value.language]:'';}
 function restore(value:MangaView){current=value;title=originalText(value,'title');language=value.language;description=originalText(value,'description');status=value.status;rating=value.content_rating;fields=richTitleFields(value);}
 function failure(error:unknown){return error instanceof Error?error.message:'The title command was not confirmed.';}
 function responseError(response:{status:number;body?:unknown}){const body=response.body as {errors?:{detail?:string}[]}|undefined;return body?.errors?.[0]?.detail??`Title request refused (HTTP ${response.status})`;}
 let initializing = $state(false);
 async function initialize(){if(initializing||disposed||!$signerIdentity.owner)return;initializing=true;message='';try{owner=await createSignedTitleController(active,captured);active();permissions=[...owner.account.permissions];roles=[...owner.account.roles];pending=owner.pending();if(captured){const response=await owner.read();active();if(response.status!==200||!response.body)throw Error(responseError(response));restore(response.body);}ready=true;if(pending)message='A previous title command has an unconfirmed outcome. It will not be replayed. Review the actual account drafts or contact the instance operator before further changes.';}catch(error){if(!disposed)message=failure(error);}finally{if(!disposed)initializing=false;}}
 onMount(()=>{void initialize();});
 async function command(action:TitleAction){
  if(!owner||!ready||busy||pending||coverBusy||coverPending||dispositionBusy||dispositionPending||relationBusy||relationPending||removed)return;
  if(metadataDirty&&action!=='create'&&action!=='update'){message='Save metadata changes before reviewing this title.';return;}
  if(action==='create'||action==='update'){if(!canWrite){message='You do not have permission to edit this title.';return;}}
  else if(action==='submit'?!access.submit:!canReview){message='You do not have permission for this title transition.';return;}
  busy=true;message='';
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
   if(payload){for(const [keys,allowed] of [[['title','alt_titles'],access.title],[['authors'],access.authors],[['artists'],access.artists],[['tags'],access.tags]] as const)if(!allowed&&keys.some(key=>Object.hasOwn(payload!,key)))throw Error('You do not have permission to edit that title field.');}
   if(payload&&(Object.hasOwn(payload,'links')||Object.hasOwn(payload,'officialLinks'))&&!canEditLinks)throw Error('You do not have permission to edit links.');
   const response=await owner.mutate(action,payload);active();
   if(response.status>=400||!response.body)throw Error(responseError(response));
   restore(response.body);
   const source=response.body.public_api.resource.attributes.nostr as {publicationStatus?:string;publicationScope?:string;metadataScope?:string}|undefined;
   message=action==='publish'?(source?.publicationStatus==='accepted_pending_projection'?'The original signed title was accepted and is waiting for actual local source projection. Public detail is not readable yet.':source?.publicationScope==='original-protocol'?'The original signed title publication receipt was accepted. Public readability remains a separate observation.':'Title published on this instance. No original-protocol publication is claimed.'):
    action==='submit'?'Draft submitted for instance review.':action==='reject'?'Draft rejected by the instance reviewer.':action==='create'?'Metadata draft saved. Upload an actual original-language cover below before submission.':'Instance-local metadata saved. Original signed source metadata is unchanged.';
  }catch(error){if(!disposed)message=failure(error);}
  finally{if(!disposed){busy=false;pending=owner?.pending()??pending;if(owner){permissions=[...owner.account.permissions];roles=[...owner.account.roles];}}}
 }
 async function reload(){if(!owner||busy||coverBusy||dispositionBusy||relationBusy||removed||!current)return;if(metadataDirty&&!confirm('Discard unsaved title metadata changes and reload the actual server record?'))return;busy=true;try{const response=await owner.read();active();if(response.status!==200||!response.body)throw Error(responseError(response));restore(response.body);message='Current title read from the server. No mutation was replayed; an unconfirmed command remains fenced.';}catch(error){if(!disposed)message=failure(error);}finally{if(!disposed){busy=false;pending=owner?.pending()??pending;if(owner){permissions=[...owner.account.permissions];roles=[...owner.account.roles];}}}}
</script>
<section class="space-y-4 rounded border p-4">
 <h3 class="text-lg font-medium">Title details</h3>
 <p>Changes apply to this site. Original publications remain unchanged.</p>
 <p>Save your title details first, then add a cover and related titles below.</p>
 {#if message}<p role="status" class="rounded border p-3">{message}</p>{/if}
 <IdentityGate show={!ready && !$signerIdentity.owner} onfinish={() => { if ($signerIdentity.owner) void initialize(); }} />
 {#if !ready && $signerIdentity.owner}<button type="button" class="rounded border px-3 py-2" disabled={initializing} onclick={() => { void initialize(); }}>Check my access again</button>{/if}
 {#if removed}<p role="status">This title was soft-hidden on this instance. Original signed publications are unchanged. Reopen the catalogue to continue.</p>{/if}
 {#if metadataDirty}<p>Save metadata changes before cover, relation, review or instance title actions.</p>{/if}
 {#if current}<p>Status: {current.state}.</p>{/if}
 {#if awaitingProjection}<p>This accepted original publication is read-only while local source projection is pending. Reload title details to observe actual public visibility.</p>{/if}
 <form onsubmit={event=>{event.preventDefault();void command(current?'update':'create');}} class="space-y-3">
  <fieldset disabled={!ready||busy||pending||coverBusy||coverPending||dispositionBusy||dispositionPending||relationBusy||relationPending||removed||awaitingProjection||!canWrite} class="space-y-3">
   <label class="block">Original-language title<input class="block w-full rounded border p-2" bind:value={title} disabled={!access.title} maxlength="500" required={!current} /></label>
   <p class="text-sm">Use a language code, such as en for English, ja for Japanese, or pt-br for Brazilian Portuguese.</p>
   <label class="block">Original language<input class="block w-full rounded border p-2" bind:value={language} required={!current} /></label>
   <label class="block">Description<textarea class="block w-full rounded border p-2" bind:value={description} maxlength="30000"></textarea></label>
   <label class="block">Publication status<select class="block rounded border p-2" bind:value={status}><option value="ongoing">Ongoing</option><option value="completed">Completed</option><option value="hiatus">Hiatus</option><option value="cancelled">Cancelled</option></select></label>
   <label class="block">Content rating<select class="block rounded border p-2" bind:value={rating}><option value="safe">Safe</option><option value="suggestive">Suggestive</option><option value="erotica">Erotica</option><option value="pornographic">Pornographic</option></select></label>
   <fieldset disabled={!access.title} class="space-y-2 rounded border p-3"><legend class="font-medium">Alternate titles</legend>
    {#each fields.altTitles as alternate,index}<div class="flex flex-wrap gap-2"><label>Language<input class="block rounded border p-2" bind:value={alternate.language} maxlength="100" required /></label><label class="flex-1">Alternate title<input class="block w-full rounded border p-2" bind:value={alternate.text} maxlength="500" required /></label><button type="button" class="rounded border px-3 py-1" onclick={()=>{fields.altTitles=fields.altTitles.filter((_,i)=>i!==index);}}>Remove alternate title</button></div>{/each}
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
  {#key current.id}<PublicCoverEditor title={current} accountId={owner!.account.id} disabled={busy||pending||dispositionBusy||dispositionPending||relationBusy||relationPending||removed||metadataDirty||awaitingProjection} assertCurrent={active} onState={state=>{coverPending=state.pending;coverBusy=state.busy;}} onChanged={async()=>{const response=await owner!.read();active();if(response.status!==200||!response.body)throw Error(responseError(response));restore(response.body);}} />{/key}
  {#if !removed}{#key current.id}<PublicRelationEditor title={current} accountId={owner!.account.id} disabled={busy||pending||coverBusy||coverPending||dispositionBusy||dispositionPending||metadataDirty||awaitingProjection} assertCurrent={active} onState={state=>{relationBusy=state.busy;relationPending=state.pending;}} onChanged={async()=>{const response=await owner!.read();active();if(response.status!==200||!response.body)throw Error(responseError(response));restore(response.body);}} />{/key}{/if}
  {#if !hasCover&&(current.state==='draft'||current.state==='rejected')}<p>Add a cover in the title’s original language before submitting for review.</p>{/if}
  <div class="flex flex-wrap gap-3">
   {#if current.state==='draft'||current.state==='rejected'}<button class="rounded border px-4 py-2" type="button" disabled={!ready||busy||pending||coverBusy||coverPending||dispositionBusy||dispositionPending||relationBusy||relationPending||removed||metadataDirty||!access.submit||!hasCover} onclick={()=>command('submit')}>Submit for review</button>{/if}
   {#if current.state==='submitted'&&canReview}
    <button class="rounded border px-4 py-2" type="button" disabled={!ready||busy||pending||coverBusy||coverPending||dispositionBusy||dispositionPending||relationBusy||relationPending||removed||metadataDirty} onclick={()=>command('publish')}>Publish reviewed title</button>
    <button class="rounded border px-4 py-2" type="button" disabled={!ready||busy||pending||coverBusy||coverPending||dispositionBusy||dispositionPending||relationBusy||relationPending||removed||metadataDirty} onclick={()=>command('reject')}>Reject reviewed draft</button>
   {/if}
   <button class="rounded border px-4 py-2" type="button" disabled={!ready||busy||coverBusy||dispositionBusy||relationBusy||removed} onclick={reload}>Reload title details</button>
  </div>
  {#if !removed}<PublicTitleDisposition title={current} controller={owner!} disabled={busy||pending||coverBusy||coverPending||relationBusy||relationPending||metadataDirty||awaitingProjection} onTitle={restore} onRemoved={()=>{removed=true;}} onState={state=>{dispositionBusy=state.busy;dispositionPending=state.pending;}} />{/if}
  <a class="underline" href={`/title/${current.id}/edit?draft=${current.state!=='published'}`}>Open title editor</a>
 {/if}
</section>
