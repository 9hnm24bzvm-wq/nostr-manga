<script lang="ts">
 import { onMount,onDestroy,untrack } from 'svelte';
 import { createEditorialController,type EditorialController,type EditorialKind,type EditorialAction } from './editorial-client';
 import type { Resource } from './types';
 import { record } from './validation';
 let {kind,target}:{kind:EditorialKind;target?:string}=$props();
 const selected=untrack(()=>({kind,target})),href=window.location.href;
 let owner:EditorialController|undefined,disposed=false;
 let current=$state<Resource>(),ready=$state(false),busy=$state(false),pending=$state(false),removed=$state(false),notice=$state('');
 let name=$state(''),text=$state(''),language=$state('en'),website=$state(''),extra=$state('{}'),evidence=$state('');
 let permissions=$state<string[]>([]),roles=$state<string[]>([]);
 const active=()=>{if(disposed||kind!==selected.kind||target!==selected.target||window.location.href!==href)throw Error('The editorial route changed. Reopen the intended target.');};
 const staff=$derived(roles.some(r=>['moderator','staff','sysop'].includes(r)));
 const isLeader=$derived(!!current?.relationships.some(r=>r.type==='leader'&&r.id===owner?.account.id));
 const canEdit=$derived(current?(kind==='author'?permissions.includes('author.update')&&(!current.attributes.locked||staff):staff&&permissions.includes('scanlation_group.update')||isLeader&&permissions.includes('self.scanlation_group.update')&&!current.attributes.locked):permissions.includes(kind==='author'?'author.create':'scanlation_group.create'));
 const canClaim=$derived(kind==='group'&&!!current&&!current.relationships.some(r=>r.type==='leader')&&!current.attributes.locked&&permissions.includes('report.create'));
 const canDelete=$derived(kind==='author'&&!!current&&staff&&permissions.includes('author.delete'));
 function restore(row:Resource){current=row;name=String(row.attributes.name??'');website=typeof row.attributes.website==='string'?row.attributes.website:'';if(kind==='author'){const biography=record(row.attributes.biography??{});language=Object.hasOwn(biography,'en')?'en':Object.keys(biography)[0]??'en';text=String(biography[language]??'');}else{text=typeof row.attributes.description==='string'?row.attributes.description:'';}extra='{}';}
 function failure(error:unknown){return error instanceof Error?error.message:'Editorial command was not confirmed.';}
 function responseError(response:{status:number;body?:unknown}){const body=response.body as {errors?:{detail?:string}[]}|undefined;return body?.errors?.[0]?.detail??`Request refused (HTTP ${response.status})`;}
 onDestroy(()=>{disposed=true;owner?.dispose();});
 onMount(()=>{void(async()=>{try{owner=await createEditorialController(kind,target,active);active();permissions=[...owner.account.permissions];roles=[...owner.account.roles];if(target){const response=await owner.read();active();if(response.status!==200||!response.body)throw Error(responseError(response));restore(response.body);}pending=owner.pending();ready=true;}catch(error){if(!disposed)notice=failure(error);}})();});
 async function command(action:EditorialAction){if(!owner||!ready||busy||pending||removed)return;busy=true;notice='';try{
  let payload:Record<string,unknown>|undefined;
  if(action==='create'||action==='update'){
   payload={...record(JSON.parse(extra),'additional supported metadata')};
   if(action==='create'||name!==current?.attributes.name)payload.name=name;
   if(kind==='author'){
    const old=record(current?.attributes.biography??{});if(action==='create'||text!==String(old[language]??''))payload.biography={...old,[language]:text};
   }else if(action==='create'||text!==String(current?.attributes.description??''))payload.description=text||null;
   if(action==='create'||website!==String(current?.attributes.website??''))payload.website=website||null;
   if(current)payload.version=current.attributes.version;
  }else if(action==='claim'){payload={version:current?.attributes.version,details:evidence};}
  else if(!confirm('Hide this author on this instance? Referenced authors cannot be deleted. Original signed publications and peer copies remain unchanged.'))return;
  const response=await owner.mutate(action,payload);active();if(response.status>=400)throw Error(responseError(response));
  if(action==='delete'){removed=true;notice='Author hidden on this instance. Original source publications are unchanged.';}
  else if(action==='claim'){const receipt=record(response.body);notice=`Claim request ${receipt.requestId} is awaiting explicit instance moderator review. Local leadership and original signing authority have not changed.`;}
  else{const row=owner.current();if(!row)throw Error('Actual editorial receipt missing.');restore(row);notice=action==='create'?(kind==='group'?'Local group created with your bound account as its instance leader. No original group key or source publisher was created.':'Local author record created. No original source publication was created.'):'Instance metadata saved. Original source keys and signed publications are unchanged.';}
 }catch(error){if(!disposed)notice=failure(error);}finally{if(!disposed){busy=false;pending=owner?.pending()??pending;}}}
 async function reload(){if(!owner||busy||removed||!current)return;busy=true;try{const response=await owner.read();active();if(response.status!==200||!response.body)throw Error(responseError(response));restore(response.body);notice='Actual current record reloaded. No mutation was replayed; uncertain writes remain fenced.';}catch(error){if(!disposed)notice=failure(error);}finally{if(!disposed){busy=false;pending=owner?.pending()??pending;}}}
</script>
<section class="mx-auto max-w-270 space-y-4 p-5">
 <h1 class="text-xl font-medium">Signed {kind==='author'?'author':'group'} {current?'settings':'creation'}</h1>
 <p>Uses the selected signing identity and its explicitly bound instance account. These are local catalogue operations. Original group keys, source roles and signed events remain separate.</p>
 {#if notice}<p role="status" class="rounded border p-3">{notice}</p>{/if}
 {#if !ready}<p>Select the intended identity in <a class="underline" href="/settings/identity">Public identity settings</a>, then reopen this page.</p>{/if}
 {#if pending}<p role="alert">A prior command has an unconfirmed outcome. It will not be replayed. Review the exact server record or ask the instance operator before further mutations.</p>{/if}
 {#if current}<p>Canonical UUID: {current.id}. Current version: {current.attributes.version}.</p>{/if}
 <form class="space-y-3" onsubmit={event=>{event.preventDefault();void command(current?'update':'create');}}>
  <fieldset class="space-y-3" disabled={!ready||busy||pending||removed||!canEdit}>
   <label class="block">Name<input class="block w-full rounded border p-2" bind:value={name} maxlength="200" required disabled={kind==='group'&&!!current&&!(staff&&permissions.includes('scanlation_group.update'))} /></label>
   {#if kind==='author'}<label class="block">Biography language<input class="block rounded border p-2" bind:value={language} onchange={event=>{language=event.currentTarget.value;text=String(record(current?.attributes.biography??{})[language]??'');}} maxlength="12" /></label>{/if}
   <label class="block">{kind==='author'?'Biography':'Description'}<textarea class="block w-full rounded border p-2" bind:value={text} maxlength="25000"></textarea></label>
   <label class="block">Website<input type="url" class="block w-full rounded border p-2" bind:value={website} maxlength="2048" /></label>
   <label class="block">Additional supported metadata, JSON<textarea class="block w-full rounded border p-2" bind:value={extra} maxlength="25000"></textarea></label>
   <p class="text-sm">Only explicit changed fields are sent. For authors, other biography languages are preserved. Group membership edits use actual native-user UUIDs; they never grant source signing authority.</p>
   <button class="rounded border px-4 py-2" type="submit">{current?'Save instance metadata':'Create local record'}</button>
  </fieldset>
 </form>
 {#if canClaim&&!removed}<section class="space-y-3 rounded border p-3"><h2>Request local group leadership review</h2><p>Provide a public ownership-evidence URL and explanation for an instance moderator. Submitting does not claim the group or change any original key.</p><textarea aria-label="Ownership evidence" class="block w-full rounded border p-2" bind:value={evidence} maxlength="8000" disabled={busy||pending}></textarea><button class="rounded border px-4 py-2" type="button" disabled={busy||pending||!evidence.trim()} onclick={()=>command('claim')}>Submit claim for review</button></section>{/if}
 <div class="flex flex-wrap gap-3">{#if canDelete&&!removed}<button class="rounded border px-4 py-2" type="button" disabled={busy||pending} onclick={()=>command('delete')}>Hide author on this instance</button>{/if}{#if current&&!removed}<button class="rounded border px-4 py-2" type="button" disabled={busy} onclick={reload}>Reload current record</button><a class="underline" href={`/${kind}/${current.id}`}>Open catalogue record</a>{/if}</div>
 <nav class="flex flex-wrap gap-3"><a class="underline" href="/catalogue-tools?kind=author">Create author</a><a class="underline" href="/catalogue-tools?kind=group">Create group</a></nav>
</section>
