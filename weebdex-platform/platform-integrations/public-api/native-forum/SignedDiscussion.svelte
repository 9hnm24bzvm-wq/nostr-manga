<script lang="ts">
 import {onDestroy,untrack} from 'svelte';
 import {page} from '$app/state';
 import {goto} from '$app/navigation';
 import {Utils,Typography} from '$lib';
 import {signerIdentity} from '../../local-library/publication';
 import type {PublicReply} from '../native-user-forum/data';
 import type {NativeReportContext,NativeReportTarget} from '../native-user-forum/report-client';
 import type {DiscussionIntent,SignedDiscussionView} from '../native-user-forum/signed-client';
 import {createSignedNativeDiscussion} from './signed-client';
 import {nativeDiscussionTarget,type NativeDiscussionTarget} from './context';

 let {target}:{target:NativeDiscussionTarget}=$props();
 let view=$state<SignedDiscussionView>(),failure=$state(''),pending=$state(false),reload=$state(0),uncertain=$state(false);
 let reportView=$state<NativeReportContext>(),reportReview=$state.raw<Record<string,unknown>>(),reportReason=$state(''),reportDetails=$state(''),reportResult=$state(''),reportUncertain=$state(false),canReport=$state(false);
 let accountName=$state(''),accountId=$state(''),draft=$state(''),parentId=$state<number>(),editing=$state<number>();
 let review=$state.raw<{intent:DiscussionIntent;prepared:Record<string,unknown>}>();
 let generation=0,disposed=false,controller:AbortController|undefined,signed:Awaited<ReturnType<typeof createSignedNativeDiscussion>>|undefined;
 const selectedPage=$derived(Number(page.url.searchParams.get('page')??'1'));
 const oldest=$derived(page.url.searchParams.get('sort')==='oldest');
 const key=()=>JSON.stringify([target.kind,target.targetId,target.threadId,$signerIdentity.owner,$signerIdentity.generation,page.url.href]);
 const feed=$derived(view?.feed);
 function clearReport(){reportView=undefined;reportReview=undefined;reportReason='';reportDetails='';signed?.reports.clear();}
 function clearDraft(){draft='';parentId=undefined;editing=undefined;review=undefined;}
 function navigate(next:number,sort=oldest){
  const url=new URL(page.url.href);
  if(next===1)url.searchParams.delete('page');else url.searchParams.set('page',String(next));
  if(sort)url.searchParams.set('sort','oldest');else url.searchParams.delete('sort');
  void goto(url.pathname+url.search+url.hash);
 }
 $effect(()=>{
  const context=key(),number=selectedPage,order=oldest?'asc':'desc';reload;
  const current=++generation;controller?.abort();signed?.dispose();signed=undefined;
  const request=new AbortController();controller=request;
  view=undefined;failure='';pending=false;accountName='';accountId='';uncertain=false;reportUncertain=false;canReport=false;reportResult='';untrack(clearDraft);untrack(clearReport);
  const raw=page.url.searchParams.get('page'),sort=page.url.searchParams.get('sort');
  if(page.url.searchParams.getAll('page').length>1||page.url.searchParams.getAll('sort').length>1||raw!==null&&!/^[1-9][0-9]*$/.test(raw)||!Number.isSafeInteger(number)||number<1||number>1000000||sort!==null&&!['latest','oldest'].includes(sort)){
   failure='Invalid discussion page or sort.';return;
  }
  const check=()=>{if(disposed||request.signal.aborted||current!==generation||context!==key())throw Error('Signed discussion context changed');};
  pending=true;
  void (async()=>{
   const selected=nativeDiscussionTarget(target,target.threadId);
   const client=await createSignedNativeDiscussion(selected.kind,selected.targetId,selected.threadId,check,{},request.signal);
   try{check();}catch(error){client.dispose();throw error;}
   signed=client;accountName=client.account.name;accountId=client.account.id;uncertain=client.pending();reportUncertain=client.reports.pending();canReport=client.account.permissions.includes('report.create');
   const value=await client.read(number,order);check();uncertain=client.pending();view=value;
  })().catch(error=>{
   if(!disposed&&!request.signal.aborted&&current===generation&&context===key())failure=error instanceof Error?error.message:'Signed comments could not be loaded.';
  }).finally(()=>{if(!disposed&&current===generation)pending=false;});
  return()=>request.abort();
 });
 onDestroy(()=>{disposed=true;generation++;controller?.abort();signed?.dispose();});
 function prepare(intent:DiscussionIntent){
  if(!signed||pending||uncertain||reportUncertain||review)return;clearReport();
  try{const prepared=signed.plan(intent);review={intent:{...intent,...(prepared.body===undefined?{}:{body:String(prepared.body)})},prepared};failure='';}
  catch(error){failure=error instanceof Error?error.message:'This action could not be reviewed.';}
 }
 function compose(event:SubmitEvent){event.preventDefault();prepare(editing===undefined?{operation:'create',body:draft,...(parentId===undefined?{}:{parentId})}:{operation:'update',replyId:editing,body:draft});}
 function edit(reply:PublicReply){if(pending||review||uncertain||reportUncertain)return;clearReport();clearDraft();editing=reply.id;draft=reply.body??'';}
 function replyTo(reply:PublicReply){if(pending||review||uncertain||reportUncertain)return;clearReport();clearDraft();parentId=reply.id;}
 async function confirm(){
  if(!review||!signed||pending||uncertain||reportUncertain)return;
  const prepared=review.prepared,client=signed,current=generation,context=key();
  pending=true;failure='';review=undefined;
  try{await client.mutate(prepared);if(disposed||current!==generation||context!==key())return;clearDraft();reload++;}
  catch(error){
   if(!disposed&&current===generation&&context===key()){
    failure=error instanceof Error?error.message:'Discussion outcome could not be confirmed.';
    uncertain=client.pending();view=undefined;clearDraft();
   }
  }finally{if(!disposed&&current===generation)pending=false;}
 }
 async function openReport(chosen:NativeReportTarget){
  if(!signed||pending||uncertain||reportUncertain||!canReport)return;
  const client=signed,current=generation,context=key();clearDraft();clearReport();failure='';reportResult='';pending=true;
  try{const value=await client.reports.read(chosen);if(disposed||current!==generation||context!==key())return;reportView=value;reportUncertain=client.reports.pending();}
  catch(error){if(!disposed&&current===generation&&context===key())failure=error instanceof Error?error.message:'Report reasons could not be loaded.';}
  finally{if(!disposed&&current===generation)pending=false;}
 }
 function prepareReport(event:SubmitEvent){event.preventDefault();if(!signed||pending||uncertain||reportUncertain)return;try{reportReview=signed.reports.plan(reportReason,reportDetails);failure='';}catch(error){failure=error instanceof Error?error.message:'Native report could not be reviewed.';}}
 async function confirmReport(){
  if(!signed||pending||uncertain||reportUncertain||!reportReview)return;
  const client=signed,current=generation,context=key(),prepared=reportReview;reportReview=undefined;pending=true;failure='';
  try{await client.reports.submit(prepared);if(disposed||current!==generation||context!==key())return;clearReport();reportResult='Report sent privately to this site’s moderators. It was not published publicly.';}
  catch(error){if(!disposed&&current===generation&&context===key()){clearReport();reportUncertain=client.reports.pending();failure=error instanceof Error?error.message:'Native report outcome could not be confirmed.';}}
  finally{if(!disposed&&current===generation)pending=false;}
 }
 function cancelRequest(){
  generation++;controller?.abort();uncertain=signed?.pending()??uncertain;reportUncertain=signed?.reports.pending()??reportUncertain;signed?.dispose();signed=undefined;view=undefined;pending=false;clearDraft();clearReport();
  failure='Discussion request cancelled. A sent action may still have an unknown outcome; no action was replayed.';
 }
</script>
<section id="comments" class="space-y-5" data-thread-id={target.threadId} data-signed-discussion-kind={target.kind} data-signed-discussion-target={target.targetId}>
 <p>Signed discussion{accountName?' as '+accountName:''}{accountId?' ('+accountId+')':''}. Actions affect only this instance’s native discussion, not protocol source comments.</p>
 <nav class="flex gap-3" aria-label="Comment order">
  <button type="button" disabled={pending||!oldest} onclick={()=>navigate(1,false)}>Latest</button>
  <button type="button" disabled={pending||oldest} onclick={()=>navigate(1,true)}>Oldest</button>
 </nav>
 {#if pending}<p role="status">Loading or updating signed comments…</p><button type="button" onclick={cancelRequest}>Cancel discussion request</button>{/if}
 {#if failure}<p role="alert">{failure}</p>{/if}
 {#if uncertain}<p role="alert">An earlier signed action has an unknown outcome. Do not repeat it. Reading cannot prove that it failed or clear its pending safeguard; ask this instance’s operator to reconcile it.</p>{/if}
 <button type="button" disabled={pending} onclick={()=>reload++}>Reload signed comments</button>
 {#if reportUncertain}<p role="alert">An earlier private report has an unknown result. Do not repeat it. Viewing reports cannot confirm the result; ask this site’s support team to check it. <a class="underline" href="/reports/signed/native">View your discussion reports</a>.</p>{/if}
 {#if reportResult}<p role="status">{reportResult}</p>{/if}
 {#if view&&canReport}<button type="button" disabled={pending||uncertain||reportUncertain} onclick={()=>openReport({type:'thread'})}>Report discussion</button>{/if}
 {#if view&&!uncertain&&!reportUncertain}
  <p>{view.thread.locked?'This discussion is locked.':'This discussion is open.'}</p>
  {#if view.actions.threadVote}
   <div class="flex gap-3">
    {#if !view.thread.locked||view.threadVote===1}<button type="button" disabled={pending||!!review||!!reportView} onclick={()=>prepare({operation:'thread_vote',vote:view?.threadVote===1?0:1})}>{view.threadVote===1?'Remove discussion upvote':'Upvote discussion'}</button>{/if}
    {#if !view.thread.locked||view.threadVote===-1}<button type="button" disabled={pending||!!review||!!reportView} onclick={()=>prepare({operation:'thread_vote',vote:view?.threadVote===-1?0:-1})}>{view.threadVote===-1?'Remove discussion downvote':'Downvote discussion'}</button>{/if}
   </div>
  {/if}
  {#if view.actions.create||editing!==undefined}
   <form onsubmit={compose} class="space-y-2">
    <label>{editing===undefined?(parentId===undefined?'New comment':'Reply to comment '+parentId):'Edit comment '+editing}<textarea aria-label="Comment text" bind:value={draft} disabled={pending||!!review||!!reportView} class="w-full" /></label>
    <p>Comments can contain up to 50 KB of text; very long comments may need shortening.</p>
    <button type="submit" disabled={pending||!!review||!!reportView}>Review comment</button>
    {#if editing!==undefined||parentId!==undefined}<button type="button" disabled={pending} onclick={clearDraft}>Cancel reply or edit</button>{/if}
   </form>
  {/if}
 {/if}
 {#if review}
  <section aria-label="Review signed discussion action" class="space-y-2 border p-3">
   <p>Review your {review.intent.operation} as {accountName}.</p>
   {#if review.intent.parentId!==undefined}<p>Replying to comment {review.intent.parentId}.</p>{/if}
   {#if review.intent.body!==undefined}<div class="typography comment" {@attach Typography.exec}>{@html Utils.parseComment(review.intent.body)}</div>{/if}
   {#if review.intent.vote!==undefined}<p>Current vote: {String(review.prepared.expectedVote)}. New vote: {review.intent.vote}.</p>{/if}
   {#if review.intent.operation==='delete'}<p>This moderator action permanently deletes the reply.</p>{/if}
   {#if review.intent.operation==='remove'}<p>This action removes the reply’s visible body.</p>{/if}
   <p>This changes only the discussion on this site.</p>
   <button type="button" disabled={pending} onclick={confirm}>Confirm signed action</button>
   <button type="button" disabled={pending} onclick={()=>review=undefined}>Cancel review</button>
  </section>
 {/if}
 {#if reportView&&!reportUncertain}
  <section aria-label="Native discussion report" class="space-y-2 border p-3">
   <p>You are reporting this {reportView.target.type} as {accountName}. Only this site’s moderators receive the report; it is not sent to the original publisher.</p>
   {#if !reportReview}
    <form onsubmit={prepareReport}>
     <label>Report reason<select aria-label="Report reason" bind:value={reportReason} disabled={pending}><option value="">Choose a reason</option>{#each reportView.reasons as reason}<option value={reason.id}>{reason.reason}{reason.detailsRequired?' (details required)':''}</option>{/each}</select></label>
     <label>Report details<textarea aria-label="Report details" bind:value={reportDetails} disabled={pending}/></label>
     <p>Report details can contain up to 20 KB of text.</p>
     <button type="submit" disabled={pending}>Review report</button>
    </form>
   {:else}
    <section aria-label="Review native discussion report"><p>Reason: {reportView.reasons.find(reason=>reason.id===reportReview?.reasonId)?.reason}</p><p>{String(reportReview.details)}</p><button type="button" disabled={pending} onclick={confirmReport}>Confirm private report</button></section>
   {/if}
   <button type="button" disabled={pending} onclick={clearReport}>Cancel report</button>
  </section>
 {/if}
 {#if feed}
  {#if !feed.data.length}<p>No comments on this page.</p>{/if}
  {#each feed.data as reply (reply.id)}
   <article class="space-y-2 border-t pt-4" id={'p'+reply.id}>
    <header class="text-muted-foreground text-sm">{#if reply.author}<a class="underline" href={'/user/'+encodeURIComponent(reply.author.id)}>{reply.author.name}</a>{:else}Unavailable author{/if} · #{reply.number} · <time datetime={reply.createdAt}>{reply.createdAt}</time></header>
    {#if reply.body!==undefined}<div class="typography comment flex-auto" {@attach Typography.exec}>{@html Utils.parseComment(reply.body)}</div>{:else}<p>This comment is unavailable.</p>{/if}
    <p class="text-muted-foreground text-xs">{reply.up} up · {reply.down} down</p>
    {#if view&&canReport&&reply.state==='active'}<button type="button" disabled={pending||uncertain||reportUncertain} onclick={()=>openReport({type:'reply',replyId:reply.id,version:reply.version})}>Report comment #{reply.number}</button>{/if}
    {#if view&&!uncertain&&!reportUncertain}
     {@const actions=view.actions.replies[String(reply.id)]}
     <div class="flex flex-wrap gap-3">
      {#if view.actions.create&&reply.state==='active'}<button type="button" disabled={pending||!!review||!!reportView} onclick={()=>replyTo(reply)}>Reply to #{reply.number}</button>{/if}
      {#if actions?.update}<button type="button" disabled={pending||!!review||!!reportView} onclick={()=>edit(reply)}>Edit #{reply.number}</button>{/if}
      {#if actions?.remove}<button type="button" disabled={pending||!!review||!!reportView} onclick={()=>prepare({operation:'remove',replyId:reply.id})}>Remove #{reply.number}</button>{/if}
      {#if actions?.delete}<button type="button" disabled={pending||!!review||!!reportView} onclick={()=>prepare({operation:'delete',replyId:reply.id})}>Permanently delete #{reply.number}</button>{/if}
      {#if actions?.vote}
       {#if !view.thread.locked&&reply.state==='active'||reply.vote===1}<button type="button" disabled={pending||!!review||!!reportView} onclick={()=>prepare({operation:'reply_vote',replyId:reply.id,vote:reply.vote===1?0:1})}>{reply.vote===1?'Remove upvote':'Upvote'} #{reply.number}</button>{/if}
       {#if !view.thread.locked&&reply.state==='active'||reply.vote===-1}<button type="button" disabled={pending||!!review||!!reportView} onclick={()=>prepare({operation:'reply_vote',replyId:reply.id,vote:reply.vote===-1?0:-1})}>{reply.vote===-1?'Remove downvote':'Downvote'} #{reply.number}</button>{/if}
      {/if}
     </div>
    {/if}
   </article>
  {/each}
  <nav class="flex gap-3" aria-label="Comment pages">
   <button type="button" disabled={pending||selectedPage===1} onclick={()=>navigate(selectedPage-1)}>Previous comments</button>
   <span>Page {selectedPage}</span>
   <button type="button" disabled={pending||selectedPage>=1000000||selectedPage*feed.limit>=feed.total} onclick={()=>navigate(selectedPage+1)}>Next comments</button>
  </nav>
 {/if}
</section>
