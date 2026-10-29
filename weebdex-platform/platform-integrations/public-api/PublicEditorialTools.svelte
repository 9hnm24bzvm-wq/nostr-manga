<script lang="ts">
 import UnsavedEditorGuard from './UnsavedEditorGuard.svelte';
 import {allowEditorDiscard} from './editor-unsaved';
 import {Locale} from '$lib';
 import {canonicalLanguageOptions,languageLabel} from './language';
 import NativeGroupTeam from './NativeGroupTeam.svelte';
 import AccountActionGate from './AccountActionGate.svelte';
 import {Session} from '$lib/stores';
 import {nativeAccountIssue,type NativeAccountIssue} from './native-account-scope';
 import { onMount,onDestroy,untrack } from 'svelte';
 import { createSessionEditorialController,type EditorialController,type EditorialKind,type EditorialAction } from './session-editorial-client';
 import {editorialFields,editorialFieldsChanged,editorialPayload,completeMembershipRoles,authorLinks,groupLinks,type EditorialFields} from './editor-fields';
 import PublicEntityPicker from './PublicEntityPicker.svelte';
 import type { Resource } from './types';
 import { record } from './validation';
 let {kind,target}:{kind:EditorialKind;target?:string}=$props();
 const selected=untrack(()=>({kind,target})),href=window.location.href;
 let owner=$state<EditorialController>(),disposed=false,generation=0,capturedSession=untrack(()=>$Session),issue=$state<NativeAccountIssue>();
 let current=$state<Resource>(),ready=$state(false),busy=$state(false),pending=$state(false),removed=$state(false),notice=$state('');
 const languages=canonicalLanguageOptions(Locale.Languages);
 let fields=$state<EditorialFields>(editorialFields(selected.kind)),evidence=$state('');
 const focused=$derived(fields.focusedLanguages.split(',').map(v=>v.trim()).filter(Boolean));
 const delay=$derived(/^(?:P(\d+)D|PT(\d+)H)$/.exec(fields.publishDelay));
 function changeDelay(amount:string,unit:string){fields.publishDelay=amount===''?'':unit==='days'?`P${amount}D`:`PT${amount}H`;}
 function focusLanguage(code:string,checked:boolean){fields.focusedLanguages=(checked?[...focused,code]:focused.filter(v=>v!==code)).join(', ');}
 let permissions=$state<string[]>([]),roles=$state<string[]>([]);
 const active=()=>{if(disposed||$Session!==capturedSession||kind!==selected.kind||target!==selected.target||window.location.href!==href)throw Error('The editorial route changed. Reopen the intended target.');};
 const staff=$derived(roles.some(r=>['moderator','staff','sysop'].includes(r)));
 const isLeader=$derived(!!current?.relationships.some(r=>r.type==='leader'&&r.id===owner?.account.id));
 const canEdit=$derived(current?(kind==='author'?permissions.includes('author.update')&&(!current.attributes.locked||staff):staff&&permissions.includes('scanlation_group.update')||isLeader&&permissions.includes('self.scanlation_group.update')&&!current.attributes.locked):permissions.includes(kind==='author'?'author.create':'scanlation_group.create'));
 const canLock=$derived(!!current&&staff&&canEdit&&(kind==='author'?permissions.includes('author.lock.update')&&typeof current.attributes.locked==='boolean':permissions.includes('scanlation_group.update')));
 const canClaim=$derived(kind==='group'&&!!current&&!current.relationships.some(r=>r.type==='leader')&&!current.attributes.locked&&permissions.includes('report.create'));
 const canDelete=$derived(!!current&&staff&&permissions.includes(kind==='author'?'author.delete':'scanlation_group.delete'));
 const dirty=$derived(editorialFieldsChanged(kind,fields,current));
 const disabled=$derived(!ready||busy||pending||removed||!canEdit);
 const acceptedMembers=$derived(new Set<string>(current?.relationships.filter(row=>row.type==='leader'||row.type==='member').map(row=>row.id)??[]));
 const memberSearch=async(query:string,signal:AbortSignal)=>{const rows=await owner!.search('user',query,signal);return staff?rows:rows.filter(row=>acceptedMembers.has(row.id));};
 const membershipSupported=$derived(completeMembershipRoles(current));
 function restore(row:Resource){current=row;fields=editorialFields(kind,row);}
 function failure(error:unknown){return error instanceof Error?error.message:'Editorial command was not confirmed.';}
 function responseError(response:{status:number;body?:unknown}){const body=response.body as {errors?:{detail?:string}[]}|undefined;return body?.errors?.[0]?.detail??`Request refused (HTTP ${response.status})`;}
 onDestroy(()=>{disposed=true;owner?.dispose();});
 let initializing = $state(false);
 async function initialize(){if(initializing||disposed||!$Session?.identity?.id)return;capturedSession=$Session;const session=capturedSession,token=generation,check=()=>{active();if(token!==generation||$Session!==session)throw Error('The contribution account changed.');};initializing=true;issue=undefined;notice='';try{const next=await createSessionEditorialController(session.identity.id,kind,target,check);try{check();}catch(error){next.dispose();throw error;}owner=next;permissions=[...owner.account.permissions];roles=[...owner.account.roles];if(target){const response=await owner.read();active();if(response.status!==200||!response.body)throw Error(responseError(response));restore(response.body);}pending=owner.pending();ready=true;}catch(error){if(!disposed&&token===generation){issue=nativeAccountIssue(error);notice=failure(error);}}finally{if(!disposed&&token===generation)initializing=false;}}
 $effect(()=>{const session=$Session;untrack(()=>{generation++;owner?.dispose();owner=undefined;capturedSession=session;current=undefined;ready=false;busy=false;pending=false;removed=false;notice='';issue=undefined;initializing=false;fields=editorialFields(selected.kind);evidence='';permissions=[];roles=[];if(session?.identity?.id)void initialize();});});
 async function command(action:EditorialAction){
  if(!owner||!ready||busy||pending||removed)return;
  if((action==='create'||action==='update')&&!canEdit||action==='claim'&&!canClaim||action==='delete'&&!canDelete)return;
  if(dirty&&(action==='claim'||action==='delete')){notice='Save or reset unsaved metadata before claim or removal actions.';return;}
  const operationOwner=owner,token=generation;busy=true;notice='';try{
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
   else{const row=owner.current();if(!row)throw Error('Actual editorial receipt missing.');restore(row);notice=action==='create'?(kind==='group'?'Group created with your account as its leader on this site. Publishing permissions have not changed.':'Author created on this site.'):'Changes saved on this site. Original publications are unchanged.';}
  }catch(error){if(!disposed&&token===generation&&owner===operationOwner)notice=failure(error);}finally{if(!disposed&&token===generation&&owner===operationOwner){busy=false;pending=owner?.pending()??pending;}}
 }
 async function reload(){if(!owner||busy||removed||!current)return;if(dirty&&!confirm('Discard unsaved editorial metadata and reload the actual server record?'))return;const operationOwner=owner,token=generation;busy=true;try{const response=await owner.read();active();if(response.status!==200||!response.body)throw Error(responseError(response));restore(response.body);notice='Actual current record reloaded. No mutation was replayed; uncertain writes remain fenced.';}catch(error){if(!disposed&&token===generation&&owner===operationOwner)notice=failure(error);}finally{if(!disposed&&token===generation&&owner===operationOwner){busy=false;pending=owner?.pending()??pending;}}}
</script>
<svelte:head><title>{current?'Edit':'Create'} {kind==='author'?'author':'group'}</title></svelte:head>
<UnsavedEditorGuard dirty={ready&&dirty} />
<section class="mx-auto max-w-270 space-y-4 p-5">
 <h1 class="text-xl font-medium">{current?'Edit':'Create'} {kind==='author'?'author':'group'}</h1>
 <p>Create or edit catalogue information with your account.</p><details class="text-sm"><summary>Where changes apply</summary><p>Changes apply to this site. Original publications and publishing permissions remain unchanged.</p></details>
 {#if notice}<p role="status" class="rounded border p-3">{notice}</p>{/if}
 <AccountActionGate {issue} onready={initialize} />
 {#if !ready && $Session?.identity?.id}<button type="button" class="rounded border px-3 py-2" disabled={initializing} onclick={() => { void initialize(); }}>Check my access again</button>{/if}
 {#if pending}<p role="alert">We could not confirm an earlier change. Do not send it again. Refresh the saved details or ask support to check the result.</p>{/if}

 {#if dirty&&current}<p>There are unsaved changes. Save or reset before a claim or removal.</p>{/if}
 <form class="space-y-4" onsubmit={event=>{event.preventDefault();void command(current?'update':'create');}}>
  <fieldset class="space-y-4" {disabled}>
   <label class="block">Name<input class="block w-full rounded border p-2" bind:value={fields.name} maxlength="200" required disabled={kind==='group'&&!!current&&!(staff&&permissions.includes('scanlation_group.update'))} /></label>
   {#if kind==='author'}
    <fieldset class="space-y-3 rounded border p-3"><legend class="font-medium">Biographies</legend>{#each fields.biography as entry,index}<div class="space-y-2"><label>Biography language<select class="block rounded border p-2" bind:value={entry.language} required>{#if !entry.language}<option value="">Choose language</option>{:else if !languages.some(([code])=>code===entry.language)}<option value={entry.language}>{languageLabel(entry.language,Locale.Languages)}</option>{/if}{#each languages as [code,label]}<option value={code}>{label}</option>{/each}</select></label><label class="block">Biography<textarea class="block w-full rounded border p-2" bind:value={entry.text} maxlength="25000"></textarea></label><button type="button" class="rounded border px-3 py-1" onclick={()=>{fields.biography=fields.biography.filter((_,i)=>i!==index);}}>Remove biography language</button></div>{/each}<button type="button" class="rounded border px-3 py-1" disabled={fields.biography.length>=100} onclick={()=>{fields.biography=[...fields.biography,{language:'',text:''}];}}>Add biography language</button><p class="text-sm">Other languages are kept unless you remove them.</p></fieldset>
   {:else}
    <label class="block">Description<textarea class="block w-full rounded border p-2" bind:value={fields.description} maxlength="25000"></textarea></label>
    <label class="block"><input type="checkbox" bind:checked={fields.inactive} /> Group is inactive</label>
    <fieldset class="space-y-2 rounded border p-3"><legend>Preferred publication delay</legend><p>Informational only: uploads are not automatically delayed. Choose a release time when uploading if a delay is needed.</p><div class="flex gap-2"><label>Duration<input class="block rounded border p-2" type="number" min="0" max="87600" step="1" value={delay?.[1]??delay?.[2]??''} oninput={event=>changeDelay(event.currentTarget.value,delay?.[1]?'days':'hours')} /></label><label>Unit<select class="block rounded border p-2" value={delay?.[1]?'days':'hours'} onchange={event=>changeDelay(delay?.[1]??delay?.[2]??'0',event.currentTarget.value)}><option value="hours">Hours</option><option value="days">Days</option></select></label><button type="button" class="rounded border px-3" onclick={()=>fields.publishDelay=''}>Clear preference</button></div>{#if fields.publishDelay&&!delay}<p>Existing saved preference is retained until you choose a new duration.</p><details><summary>Saved value</summary><code>{fields.publishDelay}</code></details>{/if}</fieldset>
    {#if current}<fieldset class="space-y-2 rounded border p-3"><legend>Focused languages</legend><div class="grid max-h-64 gap-2 overflow-auto sm:grid-cols-3">{#each languages as [code,label]}<label><input type="checkbox" checked={focused.includes(code)} onchange={event=>focusLanguage(code,event.currentTarget.checked)} /> {label}</label>{/each}</div>{#each focused.filter(code=>!languages.some(([known])=>known===code)) as code}<p>{languageLabel(code,Locale.Languages)} — saved language <button type="button" class="underline" onclick={()=>focusLanguage(code,false)}>Remove</button>{#if code==='tam'} <button type="button" class="underline" onclick={()=>{focusLanguage('tam',false);if(!focused.includes('ta'))focusLanguage('ta',true);}}>Use Tamil instead</button>{/if}</p>{/each}<p class="text-sm">Existing language values stay unchanged until you edit this selection.</p></fieldset>{/if}
   {/if}
   <fieldset class="grid gap-3 rounded border p-3 sm:grid-cols-2"><legend class="font-medium">Contact links</legend>{#each kind==='author'?authorLinks:groupLinks as [key,label]}<label>{label}<input class="block w-full rounded border p-2" type={key==='contactEmail'?'email':kind==='author'||['website','twitter','mangaUpdates'].includes(key)?'url':'text'} bind:value={fields.links[key]} maxlength="2048" /></label>{/each}</fieldset>
   {#if current&&canLock}<label class="block"><input type="checkbox" bind:checked={fields.locked} /> Lock this {kind} against non-staff edits</label>{/if}
   {#if kind==='group'&&current}
    {#if membershipSupported}
     <section class="space-y-3"><h2 class="font-medium">Local group roles</h2><p>A leader may also be an officer; other members must have one role. Changing leadership can remove your editing access. These roles never grant original source signing authority. Invite new members below and wait for acceptance before changing their roles.</p>
      <PublicEntityPicker label="Leader" bind:selected={fields.leader} single search={memberSearch} {disabled} />
      <PublicEntityPicker label="Officers" bind:selected={fields.officers} search={memberSearch} {disabled} />
      <PublicEntityPicker label="Members" bind:selected={fields.members} search={memberSearch} {disabled} />
     </section>
    {:else}<p>Membership editing needs a server response with complete local officer roles. Existing membership is preserved.</p>{/if}
   {/if}
   <div class="flex gap-3"><button class="rounded border px-4 py-2" type="submit" disabled={!!current&&!dirty}>{current?'Save changes':kind==='author'?'Create author':'Create group'}</button><button class="rounded border px-4 py-2" type="button" disabled={!dirty} onclick={()=>{if(allowEditorDiscard(dirty,message=>window.confirm(message)))fields=editorialFields(kind,current);}}>Reset changes</button></div>
  </fieldset>
 </form>
 {#if canClaim&&!removed}<section class="space-y-3 rounded border p-3"><h2>Request local group leadership review</h2><p>Provide a public ownership-evidence URL and explanation for an instance moderator. Submitting does not claim the group or change any original key.</p><textarea aria-label="Ownership evidence" class="block w-full rounded border p-2" bind:value={evidence} maxlength="8000" disabled={busy||pending||dirty}></textarea><button class="rounded border px-4 py-2" type="button" disabled={busy||pending||dirty||!evidence.trim()} onclick={()=>command('claim')}>Submit claim for review</button></section>{/if}
 <div class="flex flex-wrap gap-3">{#if canDelete&&!removed}<button class="rounded border px-4 py-2" type="button" disabled={busy||pending||dirty} onclick={()=>command('delete')}>Hide {kind} on this instance</button>{/if}{#if current&&!removed}<button class="rounded border px-4 py-2" type="button" disabled={busy} onclick={reload}>Reload details</button><a class="underline" href={`/${kind}/${current.id}`}>Open catalogue record</a>{/if}</div>
 {#if kind==='group'&&current&&!removed}<NativeGroupTeam groupId={current.id} />{/if}
 <nav class="flex flex-wrap gap-3"><a class="underline" href="/catalogue-tools?kind=author">Create author</a><a class="underline" href="/catalogue-tools?kind=group">Create group</a></nav>
</section>
