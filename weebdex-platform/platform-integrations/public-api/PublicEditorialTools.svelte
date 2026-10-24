<script lang="ts">
 import { onMount,onDestroy,untrack } from 'svelte';
 import { createEditorialController,type EditorialController,type EditorialKind,type EditorialAction } from './editorial-client';
 import {editorialFields,editorialFieldsChanged,editorialPayload,completeMembershipRoles,authorLinks,groupLinks,type EditorialFields} from './editor-fields';
 import PublicEntityPicker from './PublicEntityPicker.svelte';
 import type { Resource } from './types';
 import { record } from './validation';
 let {kind,target}:{kind:EditorialKind;target?:string}=$props();
 const selected=untrack(()=>({kind,target})),href=window.location.href;
 let owner=$state<EditorialController>(),disposed=false;
 let current=$state<Resource>(),ready=$state(false),busy=$state(false),pending=$state(false),removed=$state(false),notice=$state('');
 let fields=$state<EditorialFields>(editorialFields(selected.kind)),evidence=$state('');
 let permissions=$state<string[]>([]),roles=$state<string[]>([]);
 const active=()=>{if(disposed||kind!==selected.kind||target!==selected.target||window.location.href!==href)throw Error('The editorial route changed. Reopen the intended target.');};
 const staff=$derived(roles.some(r=>['moderator','staff','sysop'].includes(r)));
 const isLeader=$derived(!!current?.relationships.some(r=>r.type==='leader'&&r.id===owner?.account.id));
 const canEdit=$derived(current?(kind==='author'?permissions.includes('author.update')&&(!current.attributes.locked||staff):staff&&permissions.includes('scanlation_group.update')||isLeader&&permissions.includes('self.scanlation_group.update')&&!current.attributes.locked):permissions.includes(kind==='author'?'author.create':'scanlation_group.create'));
 const canLock=$derived(!!current&&staff&&canEdit&&(kind==='author'?permissions.includes('author.lock.update')&&typeof current.attributes.locked==='boolean':permissions.includes('scanlation_group.update')));
 const canClaim=$derived(kind==='group'&&!!current&&!current.relationships.some(r=>r.type==='leader')&&!current.attributes.locked&&permissions.includes('report.create'));
 const canDelete=$derived(!!current&&staff&&permissions.includes(kind==='author'?'author.delete':'scanlation_group.delete'));
 const dirty=$derived(editorialFieldsChanged(kind,fields,current));
 const disabled=$derived(!ready||busy||pending||removed||!canEdit);
 const membershipSupported=$derived(completeMembershipRoles(current));
 function restore(row:Resource){current=row;fields=editorialFields(kind,row);}
 function failure(error:unknown){return error instanceof Error?error.message:'Editorial command was not confirmed.';}
 function responseError(response:{status:number;body?:unknown}){const body=response.body as {errors?:{detail?:string}[]}|undefined;return body?.errors?.[0]?.detail??`Request refused (HTTP ${response.status})`;}
 onDestroy(()=>{disposed=true;owner?.dispose();});
 onMount(()=>{void(async()=>{try{owner=await createEditorialController(kind,target,active);active();permissions=[...owner.account.permissions];roles=[...owner.account.roles];if(target){const response=await owner.read();active();if(response.status!==200||!response.body)throw Error(responseError(response));restore(response.body);}pending=owner.pending();ready=true;}catch(error){if(!disposed)notice=failure(error);}})();});
 async function command(action:EditorialAction){
  if(!owner||!ready||busy||pending||removed)return;
  if((action==='create'||action==='update')&&!canEdit||action==='claim'&&!canClaim||action==='delete'&&!canDelete)return;
  if(dirty&&(action==='claim'||action==='delete')){notice='Save or reset unsaved metadata before claim or removal actions.';return;}
  busy=true;notice='';try{
   let payload:Record<string,unknown>|undefined;
   if(action==='create'||action==='update'){
    payload=editorialPayload(kind,fields,current);
    if(Object.hasOwn(payload,'locked')&&!canLock)throw Error('Current staff lock permission is required.');
    if(action==='update'&&Object.keys(payload).length===1){notice='No metadata changes to save.';return;}
    if(kind==='group'&&current&&Object.hasOwn(payload,'leader')&&payload.leader!==current.relationships.find(r=>r.type==='leader')?.id&&!confirm('Transfer local group leadership to the selected native account? You may lose permission to edit this group. Original group keys and signing authority will not change.'))return;
   }else if(action==='claim'){payload={version:current?.attributes.version,details:evidence};}
   else if(!confirm(`Hide this ${kind} on this instance at version ${current?.attributes.version}? ${kind==='author'?'Referenced authors cannot be deleted. ':''}Original signed publications and peer copies remain unchanged.`))return;
   const response=await owner.mutate(action,payload);active();if(response.status>=400)throw Error(responseError(response));
   if(action==='delete'){removed=true;notice=`${kind==='author'?'Author':'Group'} hidden on this instance. Original source publications are unchanged.`;}
   else if(action==='claim'){const receipt=record(response.body);notice=`Claim request ${receipt.requestId} is awaiting explicit instance moderator review. Local leadership and original signing authority have not changed.`;}
   else{const row=owner.current();if(!row)throw Error('Actual editorial receipt missing.');restore(row);notice=action==='create'?(kind==='group'?'Local group created with your bound account as its instance leader. No original group key or source publisher was created.':'Local author record created. No original source publication was created.'):'Instance metadata saved. Original source keys and signed publications are unchanged.';}
  }catch(error){if(!disposed)notice=failure(error);}finally{if(!disposed){busy=false;pending=owner?.pending()??pending;}}
 }
 async function reload(){if(!owner||busy||removed||!current)return;if(dirty&&!confirm('Discard unsaved editorial metadata and reload the actual server record?'))return;busy=true;try{const response=await owner.read();active();if(response.status!==200||!response.body)throw Error(responseError(response));restore(response.body);notice='Actual current record reloaded. No mutation was replayed; uncertain writes remain fenced.';}catch(error){if(!disposed)notice=failure(error);}finally{if(!disposed){busy=false;pending=owner?.pending()??pending;}}}
</script>
<section class="mx-auto max-w-270 space-y-4 p-5">
 <h1 class="text-xl font-medium">Signed {kind==='author'?'author':'group'} {current?'settings':'creation'}</h1>
 <p>Uses the selected signing identity and its explicitly bound instance account. These are local catalogue operations. Original group keys, source roles and signed events remain separate.</p>
 {#if notice}<p role="status" class="rounded border p-3">{notice}</p>{/if}
 {#if !ready}<p>Select the intended identity in <a class="underline" href="/settings/identity">Public identity settings</a>, then reopen this page.</p>{/if}
 {#if pending}<p role="alert">A prior command has an unconfirmed outcome. It will not be replayed. Review the exact server record or ask the instance operator before further mutations.</p>{/if}
 {#if current}<p>Canonical UUID: {current.id}. Current version: {current.attributes.version}.</p>{/if}
 {#if dirty&&current}<p>There are unsaved changes. Save or reset before a claim or removal.</p>{/if}
 <form class="space-y-4" onsubmit={event=>{event.preventDefault();void command(current?'update':'create');}}>
  <fieldset class="space-y-4" {disabled}>
   <label class="block">Name<input class="block w-full rounded border p-2" bind:value={fields.name} maxlength="200" required disabled={kind==='group'&&!!current&&!(staff&&permissions.includes('scanlation_group.update'))} /></label>
   {#if kind==='author'}
    <fieldset class="space-y-3 rounded border p-3"><legend class="font-medium">Localized biographies</legend>{#each fields.biography as entry,index}<div class="space-y-2"><label>Biography language<input class="block rounded border p-2" bind:value={entry.language} maxlength="100" required /></label><label class="block">Biography<textarea class="block w-full rounded border p-2" bind:value={entry.text} maxlength="25000"></textarea></label><button type="button" class="rounded border px-3 py-1" onclick={()=>{fields.biography=fields.biography.filter((_,i)=>i!==index);}}>Remove biography language</button></div>{/each}<button type="button" class="rounded border px-3 py-1" disabled={fields.biography.length>=100} onclick={()=>{fields.biography=[...fields.biography,{language:'',text:''}];}}>Add biography language</button><p class="text-sm">Every stored language is retained unless you explicitly edit or remove its row.</p></fieldset>
   {:else}
    <label class="block">Description<textarea class="block w-full rounded border p-2" bind:value={fields.description} maxlength="25000"></textarea></label>
    <label class="block"><input type="checkbox" bind:checked={fields.inactive} /> Group is inactive</label>
    <label class="block">Publication delay<input class="block rounded border p-2" bind:value={fields.publishDelay} maxlength="2048" placeholder="P1D or PT2H" /></label>
    {#if current}<label class="block">Focused languages<input class="block w-full rounded border p-2" bind:value={fields.focusedLanguages} placeholder="ta, en, pt-br" maxlength="1000" /></label><p class="text-sm">Changed focused languages must be lowercase two-letter codes, optionally with a two-letter region. Unchanged imported labels are retained.</p>{/if}
   {/if}
   <fieldset class="grid gap-3 rounded border p-3 sm:grid-cols-2"><legend class="font-medium">Contact links</legend>{#each kind==='author'?authorLinks:groupLinks as [key,label]}<label>{label}<input class="block w-full rounded border p-2" type={key==='contactEmail'?'email':kind==='author'||['website','twitter','mangaUpdates'].includes(key)?'url':'text'} bind:value={fields.links[key]} maxlength="2048" /></label>{/each}</fieldset>
   {#if current&&canLock}<label class="block"><input type="checkbox" bind:checked={fields.locked} /> Lock this {kind} against non-staff edits</label>{/if}
   {#if kind==='group'&&current}
    {#if membershipSupported}
     <section class="space-y-3"><h2 class="font-medium">Local group roles</h2><p>A leader may also be an officer; other members must have one role. Changing leadership can remove your editing access. These roles never grant original source signing authority.</p>
      <PublicEntityPicker label="Leader" bind:selected={fields.leader} single search={(query,signal)=>owner!.search('user',query,signal)} {disabled} />
      <PublicEntityPicker label="Officers" bind:selected={fields.officers} search={(query,signal)=>owner!.search('user',query,signal)} {disabled} />
      <PublicEntityPicker label="Members" bind:selected={fields.members} search={(query,signal)=>owner!.search('user',query,signal)} {disabled} />
     </section>
    {:else}<p>Membership editing needs a server response with complete local officer roles. Existing membership is preserved.</p>{/if}
   {/if}
   <div class="flex gap-3"><button class="rounded border px-4 py-2" type="submit" disabled={!!current&&!dirty}>{current?'Save instance metadata':'Create local record'}</button><button class="rounded border px-4 py-2" type="button" disabled={!dirty} onclick={()=>{fields=editorialFields(kind,current);}}>Reset changes</button></div>
  </fieldset>
 </form>
 {#if canClaim&&!removed}<section class="space-y-3 rounded border p-3"><h2>Request local group leadership review</h2><p>Provide a public ownership-evidence URL and explanation for an instance moderator. Submitting does not claim the group or change any original key.</p><textarea aria-label="Ownership evidence" class="block w-full rounded border p-2" bind:value={evidence} maxlength="8000" disabled={busy||pending||dirty}></textarea><button class="rounded border px-4 py-2" type="button" disabled={busy||pending||dirty||!evidence.trim()} onclick={()=>command('claim')}>Submit claim for review</button></section>{/if}
 <div class="flex flex-wrap gap-3">{#if canDelete&&!removed}<button class="rounded border px-4 py-2" type="button" disabled={busy||pending||dirty} onclick={()=>command('delete')}>Hide {kind} on this instance</button>{/if}{#if current&&!removed}<button class="rounded border px-4 py-2" type="button" disabled={busy} onclick={reload}>Reload current record</button><a class="underline" href={`/${kind}/${current.id}`}>Open catalogue record</a>{/if}</div>
 <nav class="flex flex-wrap gap-3"><a class="underline" href="/catalogue-tools?kind=author">Create author</a><a class="underline" href="/catalogue-tools?kind=group">Create group</a></nav>
</section>
