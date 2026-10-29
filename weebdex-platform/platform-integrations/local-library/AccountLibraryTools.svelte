<script lang="ts">
 import {onDestroy,untrack,tick} from 'svelte';
 import {nativeAuthority} from '../frontend-behavior/mutation-authority';
 import {accountLibrary} from './native-account-library';
 import type {AccountList,NativeAccountLibrary} from './native-account-library-client';
 import {publicLibraryCandidates} from '../public-api/library-catalogue';
 import GuestAccountAdoption from './GuestAccountAdoption.svelte';
 const {assertViewCurrent=()=>{}}:{assertViewCurrent?:()=>void}=$props();
 let rows=$state<AccountList[]>([]),page=$state(1),total=$state(0),revision=$state<number>(),accountName=$state(''),pending=$state(false),error=$state(''),notice=$state(''),needsCheck=$state(false),listName=$state(''),showAdoption=$state(false);
 let edit=$state.raw<AccountList>(),editRevision=$state<number>(),draftName=$state(''),draftIds=$state<string[]>([]),names=$state<Record<string,string>>({}),picker=$state<{id:string;title:string}[]>([]),pickerPage=$state(1),pickerTotal=$state(0),search=$state(''),pickerSearch=$state(''),chosen=$state(''),deleting=$state.raw<AccountList>();
 let adoptionBusy=$state(false);
 const busy=$derived(pending||adoptionBusy);
 let reconcileTarget:string|undefined,createReviewName:string|undefined,writeConfirmed=false;
 let epoch=0,disposed=false,returnFocus:HTMLButtonElement|undefined;
 function clearEdit(){edit=undefined;editRevision=undefined;draftName='';draftIds=[];picker=[];pickerTotal=0;search='';pickerSearch='';chosen='';deleting=undefined;}
 function guard(current:number){return()=>{assertViewCurrent();if(disposed||current!==epoch)throw Error('This account library view changed. Reopen it to continue.');};}
 async function action(work:(client:NativeAccountLibrary,check:()=>void)=>Promise<void>,write=false){if(busy||disposed)return;const current=epoch,check=guard(current);pending=true;error='';if(write)writeConfirmed=false;
  try{const access=await accountLibrary(check);access.check();accountName=access.client.account.name;await work(access.client,access.check);access.check();}
  catch(cause){if(!disposed&&current===epoch){error=cause instanceof Error?cause.message:'This library request could not be completed.';if(write)needsCheck=!writeConfirmed;}}
  finally{if(!disposed&&current===epoch)pending=false;}
 }
 async function readPage(client:NativeAccountLibrary,check:()=>void,next:number){check();const result=await client.lists(new URLSearchParams({page:String(next),limit:'20'}));check();rows=result.data;page=next;total=result.total;revision=result.revision;needsCheck=false;}
 function reload(next=page){clearEdit();return action(async(client,check)=>{if(needsCheck){if(reconcileTarget){check();await client.reconcileList(reconcileTarget);check();reconcileTarget=undefined;}else if(createReviewName){check();const all=await client.lists(new URLSearchParams({page:'1',limit:'100'}));check();if(all.total!==all.data.length)throw Error('Your lists could not be checked completely. Nothing was sent again.');if(all.data.some(row=>row.name===createReviewName)){listName='';notice='A list with that name is now in your account. Check it before creating another.';}else notice='No list with the reviewed name was found. You can review the name and create it again.';createReviewName=undefined;}}await readPage(client,check,next);});}
 // A confirmed copy refreshes only this captured account view. It is a read,
 // not a replay of the copy, and remains owned while child controls are busy.
 async function copied(assertCopyCurrent:()=>void){const current=epoch,assertParent=guard(current),check=()=>{assertCopyCurrent();assertParent();};
  try{check();pending=true;revision=undefined;error='';const access=await accountLibrary(check);access.check();await readPage(access.client,()=>{check();access.check();},1);check();}
  catch(cause){if(!disposed&&current===epoch){try{check();}catch{return;}error='Your saves were copied, but your lists could not be refreshed. Choose Refresh lists to try again.';}}
  finally{if(!disposed&&current===epoch)pending=false;}
 }
 $effect(()=>{const actor=$nativeAuthority.actor;$nativeAuthority.epoch;untrack(()=>{epoch++;rows=[];revision=undefined;total=0;page=1;accountName='';pending=false;adoptionBusy=false;needsCheck=false;reconcileTarget=undefined;createReviewName=undefined;writeConfirmed=false;error='';notice='';listName='';showAdoption=false;clearEdit();if(actor)void reload(1);});});
 onDestroy(()=>{disposed=true;epoch++;});
 async function create(event:SubmitEvent){event.preventDefault();if(revision===undefined||needsCheck||!listName.trim())return;const expected=revision,name=listName.trim();await action(async(client,check)=>{check();reconcileTarget=undefined;createReviewName=name;await client.createList(expected,name);check();writeConfirmed=true;createReviewName=undefined;listName='';notice='Private list created.';await readPage(client,check,1);},true);}
 async function loadNames(ids:string[],check:()=>void){if(!ids.length)return;check();const q=new URLSearchParams({page:'1',limit:'100',sort:'title'});for(const rating of ['safe','suggestive','erotica','pornographic'])q.append('contentRating',rating);try{const result=await publicLibraryCandidates<{id:string;title:string}>('/manga','id',ids,q);check();names={...names,...Object.fromEntries(result.map(row=>[row.id,row.title]))};}catch{check();notice='Some title names could not be loaded. Your list is unchanged.';}}
 async function readPicker(client:NativeAccountLibrary,check:()=>void,next:number,term:string){check();const q=new URLSearchParams({page:String(next),limit:'20',sort:'title',order:'asc'});if(term.trim())q.set('title',term.trim());const result=await client.titles(q);check();picker=result.data.map(row=>({id:row.id,title:row.title}));pickerPage=next;pickerTotal=result.total;pickerSearch=term;chosen='';names={...names,...Object.fromEntries(picker.map(row=>[row.id,row.title]))};}
 async function manage(row:AccountList,event:MouseEvent){const current=epoch;returnFocus=event.currentTarget as HTMLButtonElement;clearEdit();await action(async(client,check)=>{check();const result=await client.list(row.id);check();edit=result.data;editRevision=result.revision;draftName=result.data.name;draftIds=[...(result.data.mangaIds??[])];await loadNames(draftIds,check);await readPicker(client,check,1,'');});await tick();if(!disposed&&current===epoch&&edit?.id===row.id)document.getElementById('account-list-name')?.focus();}
 async function cancelDeletion(){deleting=undefined;await tick();if(returnFocus?.isConnected)returnFocus.focus();}
 async function cancelEdit(){clearEdit();await tick();if(returnFocus?.isConnected)returnFocus.focus();}
 async function save(event:SubmitEvent){event.preventDefault();const selected=edit,expected=editRevision,name=draftName.trim(),ids=[...draftIds];if(!selected||expected===undefined||needsCheck)return;await action(async(client,check)=>{check();reconcileTarget=selected.id;await client.updateList(selected.id,expected,selected.version,{name,mangaIds:ids});check();writeConfirmed=true;reconcileTarget=undefined;clearEdit();notice='List saved to your account.';await readPage(client,check,page);},true);}
 function add(){if(chosen&&!draftIds.includes(chosen)&&draftIds.length<100){draftIds=[...draftIds,chosen];chosen='';}}
 function move(index:number,direction:-1|1){const next=index+direction;if(next<0||next>=draftIds.length)return;const ids=[...draftIds];[ids[index],ids[next]]=[ids[next],ids[index]];draftIds=ids;}
 async function remove(){const selected=deleting,expected=revision;if(!selected||expected===undefined||needsCheck)return;deleting=undefined;await action(async(client,check)=>{check();reconcileTarget=selected.id;await client.removeList(selected.id,expected,selected.version);check();writeConfirmed=true;reconcileTarget=undefined;notice='List deleted. Saved titles and reading progress are kept.';await readPage(client,check,Math.max(1,total%20===1?page-1:page));},true);}
 const title=(id:string)=>names[id]||'Unavailable title';
</script>
<section class="space-y-5" aria-label="Account library tools" aria-busy={busy}>
 <header><h2 class="font-semibold">Your account lists</h2><p class="text-sm text-muted-foreground">{accountName?'Saved privately for '+accountName+'. ':''}Your lists are available on other devices when you sign in.</p></header>
 <form class="flex flex-wrap items-end gap-3" onsubmit={create}><label>New list name<input class="block rounded border p-2" bind:value={listName} maxlength={200} required disabled={busy||needsCheck}/></label><button type="submit" disabled={busy||needsCheck||revision===undefined||!listName.trim()}>Create list</button></form>
 {#if !pending&&revision!==undefined&&!rows.length}<p>No lists yet. Give your first list a name above.</p>{/if}
 {#each rows as row (row.id)}<div class="flex flex-wrap items-center gap-3 border-t pt-3"><a class="underline" href={'/library?list='+encodeURIComponent(row.id)}>{row.name}</a><span class="text-sm">{row.memberCount} titles</span><button type="button" disabled={busy||needsCheck} onclick={event=>manage(row,event)}>Manage {row.name}</button><button type="button" disabled={busy||needsCheck||revision===undefined} onclick={event=>{returnFocus=event.currentTarget as HTMLButtonElement;clearEdit();deleting=row;}}>Delete {row.name}</button></div>{/each}
 {#if total>20}<nav class="flex gap-3" aria-label="Account list pages"><button type="button" disabled={busy||page===1} onclick={()=>reload(page-1)}>Previous lists</button><span>Page {page} of {Math.ceil(total/20)}</span><button type="button" disabled={busy||page*20>=total} onclick={()=>reload(page+1)}>Next lists</button></nav>{/if}
 {#if deleting}<section class="space-y-2 rounded border p-3" aria-label="Confirm list deletion"><p>Delete “{deleting.name}”? Only the list is removed. Your saved titles and reading progress are kept.</p><button type="button" disabled={busy||needsCheck} onclick={remove}>Delete this list</button><button type="button" disabled={busy} onclick={cancelDeletion}>Cancel deletion</button></section>{/if}
 {#if edit}
  <form class="space-y-3 rounded border p-3" aria-label="Edit account list" onsubmit={save}>
   <label for="account-list-name">List name<input id="account-list-name" class="block w-full rounded border p-2" bind:value={draftName} maxlength={200} required disabled={busy||needsCheck}/></label>
   <p>{draftIds.length} of 100 titles. Changes are not saved until you choose Save list.</p>
   <ol class="space-y-2">{#each draftIds as id,i (id)}<li class="flex flex-wrap items-center gap-3"><a class="underline" href={'/title/'+encodeURIComponent(id)}>{i+1}. {title(id)}</a><button type="button" disabled={busy||needsCheck||i===0} aria-label={'Move '+title(id)+' up'} onclick={()=>move(i,-1)}>Up</button><button type="button" disabled={busy||needsCheck||i===draftIds.length-1} aria-label={'Move '+title(id)+' down'} onclick={()=>move(i,1)}>Down</button><button type="button" disabled={busy||needsCheck} aria-label={'Remove '+title(id)+' from this list'} onclick={()=>draftIds=draftIds.filter(value=>value!==id)}>Remove</button></li>{/each}</ol>
   <div class="space-y-3 border-t pt-3" aria-label="Find and add saved titles">
    <div class="flex flex-wrap items-end gap-3"><label class="block min-w-0 flex-1 space-y-1"><span class="block">Find a saved title</span><input class="block w-full min-w-0 rounded border p-2" bind:value={search} maxlength={200} disabled={busy}/></label><button type="button" disabled={busy} onclick={()=>action((client,check)=>readPicker(client,check,1,search))}>Find titles</button></div>
    <label class="block space-y-1"><span class="block">Saved title</span><select class="block w-full min-w-0 rounded border p-2" bind:value={chosen} disabled={busy||needsCheck}><option value="">Choose a saved title</option>{#each picker as row}<option value={row.id} disabled={draftIds.includes(row.id)}>{row.title}</option>{/each}</select></label>
    <button type="button" disabled={busy||needsCheck||!chosen||draftIds.length>=100} onclick={add}>Add to list</button>
    {#if pickerTotal>20}<nav class="flex gap-3" aria-label="Saved-title choices"><button type="button" disabled={busy||pickerPage===1} onclick={()=>action((client,check)=>readPicker(client,check,pickerPage-1,pickerSearch))}>Previous titles</button><span>Page {pickerPage} of {Math.ceil(pickerTotal/20)}</span><button type="button" disabled={busy||pickerPage*20>=pickerTotal} onclick={()=>action((client,check)=>readPicker(client,check,pickerPage+1,pickerSearch))}>More titles</button></nav>{/if}
   </div>
   <div class="flex gap-3"><button type="submit" disabled={busy||needsCheck||!draftName.trim()}>Save list</button><button type="button" disabled={busy} onclick={cancelEdit}>Cancel changes</button></div>
  </form>
 {/if}
 {#if pending}<p role="status">Checking or saving your account library…</p>{/if}
 {#if error}<p role="alert">{error}</p>{/if}
 {#if needsCheck}<p role="alert">This change was not confirmed. Check your account before trying again. Nothing is sent twice automatically.</p>{/if}
 <button type="button" class="underline" disabled={busy} onclick={()=>reload()}>{needsCheck?'Check account lists':'Refresh lists'}</button>
 {#if notice}<p role="status">{notice}</p>{/if}
 <section class="space-y-3 border-t pt-4"><button type="button" aria-expanded={showAdoption} disabled={busy} onclick={()=>showAdoption=!showAdoption}>{showAdoption?'Close device-copy tools':'Bring device saves into this account'}</button>{#if showAdoption}<GuestAccountAdoption {assertViewCurrent} onBusyChange={value=>adoptionBusy=value} onCopied={copied}/>{/if}</section>
</section>
