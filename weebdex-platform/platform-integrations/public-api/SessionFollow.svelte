<script lang="ts">
  import {onDestroy,untrack} from 'svelte';
  import {UserPlus} from '@lucide/svelte';
  import {Button} from '$lib/components/ui/button';
  import * as Dialog from '$lib/components/ui/dialog';
  import {refreshAccountLibraryTitles} from '../local-library/native-account-library';
  import {Session} from '$lib/stores';
  import AccountActionGate from './AccountActionGate.svelte';
  import {nativeAccountIssue,type NativeAccountIssue} from './native-account-scope';
  import {publicUuid} from './validation';
  import {loadSessionFollow,type AccountFollowKind,type AccountFollowView} from './session-follow-client';
  let {kind='user',targetId,requestedId=targetId,expectedAccountId,label,onChanged=async()=>{}}:{kind?:AccountFollowKind;targetId:string;requestedId?:string;expectedAccountId?:string;label?:string;onChanged?:(view:AccountFollowView)=>unknown|Promise<unknown>}=$props();
  let open=$state(false),pending=$state(false),error=$state(''),notice=$state(''),access=$state<NativeAccountIssue>(),current=$state<AccountFollowView>();
  let client:Awaited<ReturnType<typeof loadSessionFollow>>|undefined,epoch=0,disposed=false,lastTarget=targetId;
  function guard(){const revision=epoch,target=targetId,request=requestedId,type=kind,sourceAccount=expectedAccountId,session=$Session;return()=>{if(disposed||!open||epoch!==revision||targetId!==target||requestedId!==request||kind!==type||expectedAccountId!==sourceAccount||session!==$Session)throw Error('The profile or account changed. Reopen following.');};}
  function clear(){epoch++;client?.dispose();client=undefined;current=undefined;pending=false;error='';notice='';access=undefined;}
  $effect(()=>{const target=targetId,request=requestedId,type=kind,sourceAccount=expectedAccountId,session=$Session;untrack(()=>{clear();if(target!==lastTarget)open=false;lastTarget=target;});});
  onDestroy(()=>{disposed=true;clear();});
  async function load(){if(pending||!$Session?.identity?.id)return;const assert=guard();pending=true;error='';notice='';access=undefined;client?.dispose();client=undefined;current=undefined;try{if(publicUuid(targetId)!==publicUuid(requestedId))throw Error('Open this profile’s current page before following.');const selected=await loadSessionFollow(kind,targetId,$Session.identity.id,assert);try{assert();}catch(cause){selected.dispose();throw cause;}client=selected;if(expectedAccountId!==undefined&&selected.view().accountId!==publicUuid(expectedAccountId))throw Error('Your account changed. Reopen following from the current account.');current=await selected.reload();assert();}catch(cause){try{assert();access=nativeAccountIssue(cause);error=cause instanceof Error?cause.message:'Following could not be checked.';}catch{}}finally{try{assert();pending=false;}catch{}}}
  function setOpen(value:boolean){if(!value){open=false;clear();return;}open=true;if($Session?.identity?.id)void load();}
  async function save(){if(pending||!client||!current||current.needsRecheck)return;const assert=guard(),selected=client,followed=!current.followed;pending=true;error='';notice='';try{const result=followed?await selected.follow():await selected.unfollow();assert();current=result;if(kind==='manga')refreshAccountLibraryTitles();notice=kind==='manga'?(followed?'Chapter updates are on.':'Chapter updates are off. Your library entry and reading progress are unchanged.'):(followed?'Following.':'Unfollowed.');try{await onChanged(result);assert();}catch{assert();notice+=' Refresh the page to see the latest count.';}}catch(cause){try{assert();current=selected.view();access=nativeAccountIssue(cause);error=cause instanceof Error?cause.message:'The change was not confirmed. Reload before trying again.';}catch{}}finally{try{assert();pending=false;}catch{}}}
</script>
<Button class="border-none" type="button" variant="outline" onclick={()=>setOpen(true)}><UserPlus/>{label??'Follow'}</Button>
<Dialog.Root {open} onOpenChange={setOpen}>
  <Dialog.Content class="sm:max-w-sm">
    <Dialog.Header><Dialog.Title>{kind==='manga'?'Chapter updates':'Following'}</Dialog.Title><Dialog.Description>{#if kind==='manga'}Follow to see new chapters in My Updates. Your reading status and progress are kept when you follow or unfollow. If this title isn’t saved yet, following adds it as Plan to Read.{:else}Follow this {kind==='group'?'group':'reader'} with your account.{/if}</Dialog.Description></Dialog.Header>
    <AccountActionGate issue={access} onready={load}/>
    {#if current}
      <p class="text-sm text-muted-foreground">{kind==='manga'?(current.followed?'Chapter updates are on.':'Chapter updates are off.'):(current.followed?'You follow this '+(kind==='group'?'group':'reader')+'.':'You are not following yet.')}</p>
      <Button disabled={pending||current.needsRecheck} onclick={save}>{current.followed?'Unfollow':'Follow'}</Button>
      {#if current.needsRecheck}<Button variant="outline" disabled={pending} onclick={load}>Reload following</Button>{/if}
    {:else if !pending&&$Session?.identity?.id}<Button variant="outline" onclick={load}>Check following</Button>{/if}
    {#if pending}<p role="status" class="text-sm text-muted-foreground">Checking your account…</p>{/if}
    {#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
    {#if notice}<p role="status" class="text-sm">{notice}</p>{/if}
  </Dialog.Content>
</Dialog.Root>
