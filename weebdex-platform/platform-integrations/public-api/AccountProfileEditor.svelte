<script lang="ts">
 import {onDestroy,untrack} from 'svelte';
 import {page} from '$app/state';
 import {UserLink,Utils} from '$lib';
 import LinkGroup from '$lib/components/LinkGroup.svelte';
 import LinkGroupButton from '$lib/components/LinkGroupButton.svelte';
 import {Button} from '$lib/components/ui/button';
 import Label from '$lib/components/ui/label/label.svelte';
 import RichTextarea from '$lib/components/ui/RichTextarea.svelte';
 import Separator from '$lib/components/ui/separator/separator.svelte';
 import {PlusIcon} from '@lucide/svelte';
 import {Session} from '$lib/stores';
 import AccountActionGate from './AccountActionGate.svelte';
 import {nativeAccountIssue,type NativeAccountIssue} from './native-account-scope';
 import {loadAccountProfile,accountProfileFields,type AccountProfileView,type AccountProfilePatch} from './session-account-profile-client';
 const {targetId,nativeEligible,onSaved=()=>{}}:{targetId:string;nativeEligible:boolean;onSaved?:(view:AccountProfileView)=>void}=$props();
 let client:Awaited<ReturnType<typeof loadAccountProfile>>|undefined,epoch=0,disposed=false;
 let current=$state<AccountProfileView>(),draft=$state<{description:string;links:ExternalLink[]}>(),pending=$state(false),error=$state(''),notice=$state('');
 let access=$state<NativeAccountIssue>();
 const eligible=$derived(nativeEligible&&!!$Session?.identity?.id&&/^\/user\/[^/]+\/settings$/.test(page.url.pathname));
 const changed=$derived(!!current&&!!draft&&(draft.description!==current.description||['discord','twitter','website'].some(field=>(draft!.links.find(link=>link.code===field)?.link??'')!==current![field as keyof AccountProfileView])));
 const valid=$derived(!!draft&&draft.links.every(link=>link.valid)&&new Set(draft.links.map(link=>link.code)).size===draft.links.length);
 function guard(){const revision=epoch,target=targetId,location=page.url.href,session=$Session;return()=>{if(disposed||epoch!==revision||targetId!==target||page.url.href!==location||session!==$Session||!eligible)throw Error('The selected profile or account changed. Reopen profile settings from your account.');};}
 function reset(){if(!current)return;draft={description:current.description,links:['discord','twitter','website'].filter(field=>!!current![field as keyof AccountProfileView]).map(code=>({code,link:String(current![code as keyof AccountProfileView]),valid:true}))};}
 function clear(){epoch++;client?.dispose();client=undefined;access=undefined;current=undefined;draft=undefined;pending=false;error='';notice='';}
 $effect(()=>{const target=targetId,native=nativeEligible,location=page.url.href,session=$Session;untrack(()=>{clear();if(eligible)void load();});});
 onDestroy(()=>{disposed=true;epoch++;client?.dispose();});
 async function load(){if(pending||!eligible)return;const assert=guard();pending=true;error='';notice='';current=undefined;draft=undefined;try{const selected=client??await loadAccountProfile(targetId,$Session!.identity.id,assert);try{assert();}catch(cause){selected.dispose();throw cause;}client=selected;const observed=await selected.reload();assert();client=selected;current=observed;reset();}catch(cause){try{assert();access=nativeAccountIssue(cause);error=cause instanceof Error?cause.message:'Your public account profile could not be loaded.';}catch{}}finally{try{assert();pending=false;}catch{}}}
 async function save(event:Event){event.preventDefault();if(pending||!eligible||!current||!draft||!client||current.needsRecheck||!changed||!valid)return;const assert=guard(),selected=client,before=current,submitted={description:draft.description,links:draft.links.map(link=>({...link}))},patch:AccountProfilePatch={};if(submitted.description!==before.description)patch.description=submitted.description;for(const field of accountProfileFields.filter(field=>field!=='description')){const text=submitted.links.find(link=>link.code===field)?.link??'';if(text!==before[field])patch[field]=text;}pending=true;error='';notice='';try{const result=await selected.save(patch);assert();current=result;notice='Your profile has been saved.';if(draft.description===submitted.description&&JSON.stringify(draft.links)===JSON.stringify(submitted.links))reset();try{onSaved(result);}catch{error='Your profile was saved. Refresh this page to update its public presentation.';}}catch(cause){try{assert();current=selected.view();error=cause instanceof Error?cause.message:'The profile change was not confirmed. Reload before trying again.';}catch{}}finally{try{assert();pending=false;}catch{}}}
</script>
<section class="mt-4 space-y-5">
 <AccountActionGate issue={access} onready={load} />
 {#if !eligible}<p class="text-muted-foreground">Sign in and open your own profile to edit your biography and contact links.</p><a class="underline" href="/settings/account">Account settings</a>
 {:else}
  {#if !current||current.needsRecheck}<Button variant="outline" disabled={pending} onclick={load}>{current?.needsRecheck?'Reload account profile':'Load account profile'}</Button>{/if}
  {#if current&&draft}<form class="space-y-5" onsubmit={save} onreset={event=>{event.preventDefault();reset();}}>
   <div class="space-y-2"><Label>Username</Label><p class="text-foreground/85">{current.username}</p><p class="text-sm text-muted-foreground">Your biography and contact links are public on this instance.</p></div>
   <div class="space-y-2"><Label for="biography">Biography</Label><RichTextarea class="min-h-35" id="biography" bind:value={() => draft?.description ?? '', value => { if (draft) draft.description = value; }} parser={Utils.parseDescription} disabled={pending||current.needsRecheck}/></div>
   <Separator/><div class="space-y-2"><Label>Contact Info{#if draft.links.length} ({draft.links.length}){/if}</Label>{#if draft.links.length}<div class="space-y-1"><LinkGroup bind:value={() => draft?.links ?? [], value => { if (draft) draft.links = value; }} module={UserLink} disabled={pending||current.needsRecheck}/></div>{/if}<LinkGroupButton bind:links={() => draft?.links ?? [], value => { if (draft) draft.links = value; }} disabled={pending||current.needsRecheck}><PlusIcon/>Add Contact Info</LinkGroupButton></div>
   <Separator/><div class="bg-background sticky bottom-0 flex items-center justify-end gap-1 py-1"><Button class="!text-destructive w-24 border-none" type="reset" variant="outline" disabled={pending||!changed}>Revert</Button><Button class="w-30" type="submit" disabled={pending||current.needsRecheck||!changed||!valid}>Save</Button></div>
  </form>{/if}
  {#if pending}<p role="status" class="text-sm text-muted-foreground">Checking account profile…</p>{/if}
  {#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
  {#if notice}<p role="status" class="text-sm">{notice}</p>{/if}
 {/if}
</section>
