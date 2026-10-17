<script lang="ts">
 import { onDestroy, onMount, untrack } from 'svelte';
 import { createSignedTitleController, titleAwaitingProjection, type SignedTitleController, type TitleAction } from './title-client';
 import PublicCoverEditor from './PublicCoverEditor.svelte';
 import type { MangaView } from './types';
 let { data }: { data?: { manga?: { id:string }; covers?: unknown[] } } = $props();
 const captured=untrack(()=>data?.manga?.id),path=window.location.pathname;
 let owner=$state<SignedTitleController>();
 let disposed=false;
 let current=$state<MangaView>(),ready=$state(false),busy=$state(false),pending=$state(false),message=$state('');
 let title=$state(''),language=$state('en'),description=$state(''),status=$state('ongoing'),rating=$state('safe');
 let coverPending=$state(false),coverBusy=$state(false);
 let permissions=$state<string[]>([]),roles=$state<string[]>([]);
 const active=()=>{if(disposed||window.location.pathname!==path||data?.manga?.id!==captured)throw Error('The title editor changed. Reopen the current route.');};
 onDestroy(()=>{disposed=true;owner?.dispose();});
 const canWrite=$derived(current?(current.state==='published'?permissions.includes('manga.update'):permissions.includes('manga.create')):permissions.includes('manga.create'));
 const canReview=$derived(roles.some(role=>['moderator','staff','sysop'].includes(role))&&permissions.includes('manga.state.update'));
 const awaitingProjection=$derived(titleAwaitingProjection(current));
 const hasCover=$derived(!!current?.relationships.cover&&current.relationships.cover.language===current.language);
 function originalText(value:MangaView,key:'title'|'description'){const map=value.public_api.resource.attributes[key];return map&&typeof map==='object'&&value.language in map&&typeof (map as Record<string,unknown>)[value.language]==='string'?(map as Record<string,string>)[value.language]:'';}
 function restore(value:MangaView){current=value;title=originalText(value,'title');language=value.language;description=originalText(value,'description');status=value.status;rating=value.content_rating;}
 function failure(error:unknown){return error instanceof Error?error.message:'The title command was not confirmed.';}
 function responseError(response:{status:number;body?:unknown}){const body=response.body as {errors?:{detail?:string}[]}|undefined;return body?.errors?.[0]?.detail??`Title request refused (HTTP ${response.status})`;}
 onMount(()=>{void(async()=>{try{owner=await createSignedTitleController(active,captured);active();permissions=[...owner.account.permissions];roles=[...owner.account.roles];pending=owner.pending();if(captured){const response=await owner.read();active();if(response.status!==200||!response.body)throw Error(responseError(response));restore(response.body);}ready=true;if(pending)message='A previous title command has an unconfirmed outcome. It will not be replayed. Review the actual account drafts or contact the instance operator before further changes.';}catch(error){if(!disposed)message=failure(error);}})();});
 async function command(action:TitleAction){
  if(!owner||!ready||busy||pending||coverBusy||coverPending)return;
  busy=true;message='';
  try{
   let payload:Record<string,unknown>|undefined;
   if(action==='create')payload={title,language,description,status,content_rating:rating,authors:[],artists:[],tags:[]};
   else if(action==='update'){
    if(!current)throw Error('Read the current title first.');
    payload={version:current.version};
    if(title!==originalText(current,'title'))payload.title=title;
    if(description!==originalText(current,'description'))payload.description=description;
    if(language!==current.language)payload.language=language;
    if(status!==current.status)payload.status=status;
    if(rating!==current.content_rating)payload.content_rating=rating;
    if(Object.keys(payload).length===1){message='No metadata changes to save.';return;}
   }
   const response=await owner.mutate(action,payload);active();
   if(response.status>=400||!response.body)throw Error(responseError(response));
   restore(response.body);
   const source=response.body.public_api.resource.attributes.nostr as {publicationStatus?:string;publicationScope?:string;metadataScope?:string}|undefined;
   message=action==='publish'?(source?.publicationStatus==='accepted_pending_projection'?'The original signed title was accepted and is waiting for actual local source projection. Public detail is not readable yet.':source?.publicationScope==='original-protocol'?'The original signed title publication receipt was accepted. Public readability remains a separate observation.':'Title published on this instance. No original-protocol publication is claimed.'):
    action==='submit'?'Draft submitted for instance review.':action==='reject'?'Draft rejected by the instance reviewer.':action==='create'?'Metadata draft saved. Upload an actual original-language cover below before submission.':'Instance-local metadata saved. Original signed source metadata is unchanged.';
  }catch(error){if(!disposed)message=failure(error);}
  finally{if(!disposed){busy=false;pending=owner?.pending()??pending;}}
 }
 async function reload(){if(!owner||busy||coverBusy||!current)return;busy=true;try{const response=await owner.read();active();if(response.status!==200||!response.body)throw Error(responseError(response));restore(response.body);message='Current title read from the server. No mutation was replayed; an unconfirmed command remains fenced.';}catch(error){if(!disposed)message=failure(error);}finally{if(!disposed){busy=false;pending=owner?.pending()??pending;}}}
</script>
<section class="space-y-4 rounded border p-4">
 <h3 class="text-lg font-medium">Signed title metadata</h3>
 <p>Uses the selected public signing identity and its explicitly bound native account. Published edits are instance-local annotations; original signed source content is immutable.</p>
 <p>Cover controls use their own exact signed original-image interface below. Relation changes, title deletion, lock/unlock and official-link controls remain unavailable; this form never calls their native chains.</p>
 {#if message}<p role="status" class="rounded border p-3">{message}</p>{/if}
 {#if !ready}<p>Select a signing identity in Public identity settings and reopen this editor. A cookie session alone does not authorize these commands.</p>{/if}
 {#if current}<p>Current state: {current.state}. Version: {current.version}.</p>{/if}
 {#if awaitingProjection}<p>This accepted original publication is read-only while local source projection is pending. Reload current metadata to observe actual public visibility.</p>{/if}
 <form onsubmit={event=>{event.preventDefault();void command(current?'update':'create');}} class="space-y-3">
  <fieldset disabled={!ready||busy||pending||coverBusy||coverPending||awaitingProjection||!canWrite} class="space-y-3">
   <label class="block">Original-language title<input class="block w-full rounded border p-2" bind:value={title} maxlength="500" required={!current} /></label>
   <label class="block">Original language<input class="block w-full rounded border p-2" bind:value={language} maxlength="12" required /></label>
   <label class="block">Description<textarea class="block w-full rounded border p-2" bind:value={description} maxlength="30000"></textarea></label>
   <label class="block">Publication status<select class="block rounded border p-2" bind:value={status}><option value="ongoing">Ongoing</option><option value="completed">Completed</option><option value="hiatus">Hiatus</option><option value="cancelled">Cancelled</option></select></label>
   <label class="block">Content rating<select class="block rounded border p-2" bind:value={rating}><option value="safe">Safe</option><option value="suggestive">Suggestive</option><option value="erotica">Erotica</option><option value="pornographic">Pornographic</option></select></label>
   <button class="rounded border px-4 py-2" type="submit">{current?'Save instance metadata':'Save metadata draft'}</button>
  </fieldset>
 </form>
 {#if current}
  {#key current.id}<PublicCoverEditor title={current} accountId={owner!.account.id} disabled={busy||pending||awaitingProjection||!canWrite} assertCurrent={active} onState={state=>{coverPending=state.pending;coverBusy=state.busy;}} onChanged={async()=>{const response=await owner!.read();active();if(response.status!==200||!response.body)throw Error(responseError(response));restore(response.body);}} />{/key}
  {#if !hasCover&&(current.state==='draft'||current.state==='rejected')}<p>Submission needs an actual original-language cover with hosted original bytes. Upload an original-language cover below and reload its acknowledged metadata. The server checks actual cover availability.</p>{/if}
  <div class="flex flex-wrap gap-3">
   {#if current.state==='draft'||current.state==='rejected'}<button class="rounded border px-4 py-2" type="button" disabled={!ready||busy||pending||coverBusy||coverPending||!canWrite||!hasCover} onclick={()=>command('submit')}>Submit for review</button>{/if}
   {#if current.state==='submitted'&&canReview}
    <button class="rounded border px-4 py-2" type="button" disabled={!ready||busy||pending||coverBusy||coverPending} onclick={()=>command('publish')}>Publish reviewed title</button>
    <button class="rounded border px-4 py-2" type="button" disabled={!ready||busy||pending||coverBusy||coverPending} onclick={()=>command('reject')}>Reject reviewed draft</button>
   {/if}
   <button class="rounded border px-4 py-2" type="button" disabled={!ready||busy||coverBusy} onclick={reload}>Reload current metadata</button>
  </div>
  <a class="underline" href={`/title/${current.id}/edit?draft=${current.state!=='published'}`}>Open canonical title editor</a>
 {/if}
</section>
