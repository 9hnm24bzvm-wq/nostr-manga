<script lang="ts">
 import { onDestroy } from 'svelte';
 import type { MangaView } from './types';
 import type { SignedTitleController, TitleDispositionAction } from './title-client';
 let { title, controller, disabled=false, onTitle, onRemoved, onState }: { title:MangaView;controller:SignedTitleController;disabled?:boolean;onTitle:(title:MangaView)=>void;onRemoved:()=>void;onState:(state:{busy:boolean;pending:boolean})=>void }=$props();
 let disposed=false,busy=$state(false),message=$state(''),requestId=$state<string>(),requestVersion=$state<number>();
 const moderator=$derived(controller.account.roles.some(role=>['moderator','staff','sysop'].includes(role)));
 const canLock=$derived(moderator&&controller.account.permissions.includes('manga.lock.update'));
 const canHide=$derived(moderator&&controller.account.permissions.includes('manga.delete'));
 const owns=$derived(title.public_api.resource.relationships.some(row=>row.type==='creator'&&row.id===controller.account.id));
 const canRequest=$derived(owns&&controller.account.permissions.includes('manga.create'));
 onDestroy(()=>{disposed=true;});
 async function run(action:TitleDispositionAction){
  if(busy||disabled||controller.pending()||disposed)return;
  const confirmation=action==='remove'?(canHide?'Approve this instance title removal now? The title and its chapters will become hidden here immediately. Original signed events and image bytes are unchanged.':'Request instance moderator review of this title’s removal? The title stays visible here until approval. No original deletion request is sent.'):`${action==='lock'?'Lock':'Unlock'} title editing on this instance now? The original signed title remains unchanged.`;
  if(!confirm(confirmation))return;
  busy=true;message='';onState({busy,pending:controller.pending()});
  try{
   controller.assert();const result=await controller.disposition(action,action==='remove'?(canHide?'approve-hide':'request-review'):undefined);controller.assert();if(disposed)return;
   if(result.status===200){onTitle(result.title);message=result.title.locked?'Title editing locked on this instance.':'Title editing unlocked on this instance.';}
   else if(result.status===204){message='Title soft-hidden on this instance with moderator approval. Original signed publications remain unchanged.';onRemoved();}
   else{requestId=result.requestId;requestVersion=result.targetVersion;message='Removal review is durably queued. The title remains visible until an instance moderator approves it. No original deletion was requested.';}
  }catch(error){if(!disposed)message=error instanceof Error?error.message:'The title disposition was not confirmed.';}
  finally{if(!disposed){busy=false;onState({busy,pending:controller.pending()});}}
 }
</script>
<section class="space-y-3 rounded border p-3" aria-label="Instance title disposition">
 <h4 class="font-medium">Instance title controls</h4>
 <p class="text-sm">These controls affect this instance only. Original signed events and hosted original image bytes are not revised or deleted.</p>
 {#if message}<p role="status">{message}</p>{/if}
 {#if requestId}<p class="break-all text-sm">Removal review: {requestId}. Reviewed title version: {requestVersion}.{#if requestVersion!==title.version} The title changed afterward; a moderator must review its current state before removal.{/if}</p>{/if}
 <div class="flex flex-wrap gap-3">
  {#if canLock}<button type="button" class="rounded border px-3 py-2" disabled={disabled||busy||controller.pending()} onclick={()=>run(title.locked?'unlock':'lock')}>{title.locked?'Unlock instance editing':'Lock instance editing'}</button>{/if}
  {#if canHide||canRequest}<button type="button" class="rounded border px-3 py-2" disabled={disabled||busy||controller.pending()||(!canHide&&!!requestId)} onclick={()=>run('remove')}>{canHide?'Approve instance title removal':requestId?'Removal review queued':'Request title removal review'}</button>{:else}<p>Removal review is available to this title’s current owner. Immediate instance removal requires moderator status and manga.delete.</p>{/if}
 </div>
</section>
