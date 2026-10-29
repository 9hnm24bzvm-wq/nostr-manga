<script lang="ts">
  import AccountActionGate from './AccountActionGate.svelte';
  import {onDestroy,untrack} from 'svelte';
  import {Star} from '@lucide/svelte';
  import {Button} from '$lib/components/ui/button';
  import * as Dialog from '$lib/components/ui/dialog';
  import {Session} from '$lib/stores';
  import type {PersonalRating} from './rating-client';
  import {loadSessionRating} from './session-rating-client';
  import {nativeAccountIssue,type NativeAccountIssue} from './native-account-scope';
  const {mangaId,onSaved=async()=>{}}:{mangaId:string;onSaved?:()=>Promise<unknown>}=$props();
  let open=$state(false),pending=$state(false),error=$state(''),notice=$state(''),choice=$state('');
  let access=$state<NativeAccountIssue>();
  let current=$state<PersonalRating>(),client:Awaited<ReturnType<typeof loadSessionRating>>|undefined;
  let epoch=0,disposed=false,lastTarget=mangaId;
  function guard(){const generation=epoch,target=mangaId,session=$Session,owner=$Session?.identity?.id;return()=>{if(disposed||!open||epoch!==generation||target!==mangaId||owner!==$Session?.identity?.id||session!==$Session)throw Error('The selected title or rating account changed. Reopen your rating.');};}
  function clear(){epoch++;client?.dispose();client=undefined;current=undefined;choice='';error='';notice='';pending=false;access=undefined;}
  $effect(()=>{const target=mangaId,session=$Session;untrack(()=>{clear();if(target!==lastTarget)open=false;lastTarget=target;});});
  onDestroy(()=>{disposed=true;epoch++;client?.dispose();});
  async function load(){if(pending||!$Session?.identity?.id)return;const assert=guard();pending=true;error='';notice='';access=undefined;client?.dispose();client=undefined;current=undefined;try{const selected=await loadSessionRating(mangaId,$Session!.identity.id,assert);try{assert();}catch(cause){selected.dispose();throw cause;}client=selected;const result=await selected.reload();assert();current=result;choice=result.rating===null?'':String(result.rating);}catch(cause){try{assert();access=nativeAccountIssue(cause);error=cause instanceof Error?cause.message:'Your rating could not be loaded.';}catch{}}finally{try{assert();pending=false;}catch{}}}
  function setOpen(value:boolean){if(!value){open=false;clear();return;}open=true;if($Session?.identity?.id)void load();}
  async function save(remove=false){if(pending||!client||!current||current.needsRecheck)return;const assert=guard(),selected=client;pending=true;error='';notice='';try{const result=remove?await selected.remove():await selected.save(Number(choice));assert();current=result;choice=result.rating===null?'':String(result.rating);notice=remove?'Rating removed.':'Rating saved.';try{await onSaved();assert();}catch{assert();notice+=' Refresh the title to see its latest average.';}}catch(cause){try{assert();current=selected.view();access=nativeAccountIssue(cause);error=cause instanceof Error?cause.message:'Your rating was not confirmed. Reload it before trying again.';}catch{}}finally{try{assert();pending=false;}catch{}}}
</script>
<Button class="border-none" type="button" variant="outline" onclick={()=>setOpen(true)}><Star/>Rate</Button>
<Dialog.Root {open} onOpenChange={setOpen}>
  <Dialog.Content class="sm:max-w-sm">
    <Dialog.Header><Dialog.Title>Your rating</Dialog.Title><Dialog.Description>Choose a score from 1 to 10 for this title.</Dialog.Description></Dialog.Header>
    <AccountActionGate issue={access} onready={load} />
    {#if !$Session?.identity?.id}
      <p class="text-sm text-muted-foreground">Sign in to save your rating to your account.</p>
    {:else if current}
      <p class="text-sm text-muted-foreground">Saved to {current.username} on this site.</p>
      <label class="space-y-1 text-sm">Your score
        <select class="block w-full rounded-md border bg-background px-3 py-2" bind:value={choice} disabled={pending||current.needsRecheck}>
          <option value="">Choose a score</option>{#each Array.from({length:10},(_,index)=>index+1) as score}<option value={String(score)}>{score} / 10</option>{/each}
        </select>
      </label>
      <div class="flex flex-wrap gap-2">
        <Button disabled={pending||!choice||current.needsRecheck} onclick={()=>save()}>Save rating</Button>
        {#if current.rating!==null}<Button variant="outline" disabled={pending||current.needsRecheck} onclick={()=>save(true)}>Remove rating</Button>{/if}
        {#if current.needsRecheck}<Button variant="outline" disabled={pending} onclick={load}>Reload rating</Button>{/if}
      </div>
    {:else if !pending}
      <Button variant="outline" onclick={load}>Load my rating</Button>
    {/if}
    {#if pending}<p role="status" class="text-sm text-muted-foreground">Checking your account rating…</p>{/if}
    {#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
    {#if notice}<p role="status" class="text-sm">{notice}</p>{/if}
  </Dialog.Content>
</Dialog.Root>
