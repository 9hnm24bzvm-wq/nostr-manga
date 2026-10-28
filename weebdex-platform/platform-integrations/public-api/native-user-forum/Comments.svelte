<script lang="ts">
 import IdentityGate from '../../local-library/IdentityGate.svelte';
 import {onDestroy,untrack} from 'svelte';
 import {page} from '$app/state';
 import {goto} from '$app/navigation';
 import {getUser} from '$lib/Context';
 import {Utils,Typography} from '$lib';
 import {useComments} from '$lib/components/comments/context.svelte';
 import {signerIdentity} from '../../local-library/publication';
 import {readNativeUserDiscussion,type PublicDiscussionPage,type PublicReply} from './reader';
 import {createSignedUserDiscussion,type DiscussionIntent,type SignedDiscussionView} from './signed-client';
 import type {NativeReportContext,NativeReportTarget} from './report-client';
 const user=getUser(),comments=useComments();
 let feed=$state<PublicDiscussionPage>(),signedView=$state<SignedDiscussionView>(),failure=$state(''),pending=$state(false),reload=$state(0),useSigner=$state(false),accountName=$state(''),uncertain=$state(false);
 let reportView=$state<NativeReportContext>(),reportReview=$state.raw<Record<string,unknown>>(),reportReason=$state(''),reportDetails=$state(''),reportResult=$state(''),reportUncertain=$state(false),canReport=$state(false);
 let draft=$state(''),parentId=$state<number>(),editing=$state<number>(),review=$state.raw<{intent:DiscussionIntent;prepared:Record<string,unknown>}>();
 let generation=0,disposed=false,controller:AbortController|undefined,signed:Awaited<ReturnType<typeof createSignedUserDiscussion>>|undefined,lastBase='';
 const selectedPage=$derived(Number(page.url.searchParams.get('page')??'1'));
 const oldest=$derived(page.url.searchParams.get('sort')==='oldest');
 const base=()=>JSON.stringify([$user.id,comments?.thread?.id,$signerIdentity.owner,$signerIdentity.generation]);
 const key=()=>JSON.stringify([base(),page.url.href]);
 function navigate(next:number,sort=oldest){const url=new URL(page.url.href);if(next===1)url.searchParams.delete('page');else url.searchParams.set('page',String(next));if(sort)url.searchParams.set('sort','oldest');else url.searchParams.delete('sort');void goto(url.pathname+url.search+url.hash);}
 function clearReport(){reportView=undefined;reportReview=undefined;reportReason='';reportDetails='';signed?.reports.clear();}
 function clearDraft(){draft='';parentId=undefined;editing=undefined;review=undefined;}
 $effect(()=>{
  const context=key(),scope=base(),number=selectedPage,order=oldest?'asc':'desc',wanted=useSigner;reload;
  const current=++generation,previous=lastBase;lastBase=scope;controller?.abort();signed?.dispose();signed=undefined;
  const request=new AbortController();controller=request;feed=undefined;signedView=undefined;failure='';pending=false;accountName='';uncertain=false;reportUncertain=false;canReport=false;reportResult='';untrack(clearDraft);untrack(clearReport);
  if(previous&&previous!==scope&&wanted){useSigner=false;return;}
  const thread=comments?.thread?.id,id=$user.id;
  if(!thread||!$user.public_account_follow_available)return;
  const raw=page.url.searchParams.get('page'),sort=page.url.searchParams.get('sort');
  if(page.url.searchParams.getAll('page').length>1||page.url.searchParams.getAll('sort').length>1||raw!==null&&!/^[1-9][0-9]*$/.test(raw)||sort!==null&&!['latest','oldest'].includes(sort)){failure='Invalid profile comment page or sort.';return;}
  const check=()=>{if(disposed||request.signal.aborted||current!==generation||context!==key())throw Error('Profile discussion context changed');};
  pending=true;
  void (async()=>{if(wanted){const next=await createSignedUserDiscussion(id,thread,check,{},request.signal);try{check();}catch(error){next.dispose();throw error;}signed=next;const value=await next.read(number,order);check();accountName=next.account.name;uncertain=next.pending();reportUncertain=next.reports.pending();canReport=next.account.permissions.includes('report.create');signedView=value;feed=value.feed;}else{const value=await readNativeUserDiscussion(id,thread,number,request.signal,check,order);check();feed=value;}})().catch(error=>{if(!disposed&&!request.signal.aborted&&current===generation&&context===key())failure=error instanceof Error?error.message:'Profile comments could not be loaded.';}).finally(()=>{if(!disposed&&current===generation)pending=false;});
  return()=>request.abort();
 });
 onDestroy(()=>{disposed=true;generation++;controller?.abort();signed?.dispose();});
 function prepare(intent:DiscussionIntent){if(!signed||pending)return;clearReport();try{const prepared=signed.plan(intent);review={intent:{...intent,...(prepared.body===undefined?{}:{body:String(prepared.body)})},prepared};failure='';}catch(error){failure=error instanceof Error?error.message:'This action could not be reviewed.';}}
 function compose(event:SubmitEvent){event.preventDefault();prepare(editing===undefined?{operation:'create',body:draft,...(parentId===undefined?{}:{parentId})}:{operation:'update',replyId:editing,body:draft});}
 function edit(reply:PublicReply){clearReport();clearDraft();editing=reply.id;draft=reply.body??'';}
 function replyTo(reply:PublicReply){clearReport();clearDraft();parentId=reply.id;}
 async function confirm(){if(!review||!signed||pending)return;const next=review,client=signed,current=generation,context=key();pending=true;failure='';review=undefined;
  try{await client.mutate(next.prepared);if(disposed||current!==generation||context!==key())return;clearDraft();reload++;}
  catch(error){if(!disposed&&current===generation&&context===key()){failure=error instanceof Error?error.message:'Discussion outcome could not be confirmed.';uncertain=client.pending();signedView=undefined;clearDraft();}}
  finally{if(!disposed&&current===generation)pending=false;}
 }
 async function openReport(target:NativeReportTarget){if(!signed||pending||reportUncertain)return;const client=signed,current=generation,context=key();clearDraft();clearReport();failure='';reportResult='';pending=true;try{const value=await client.reports.read(target);if(disposed||current!==generation||context!==key())return;reportView=value;reportUncertain=client.reports.pending();}catch(error){if(!disposed&&current===generation&&context===key())failure=error instanceof Error?error.message:'Report reasons could not be loaded.';}finally{if(!disposed&&current===generation)pending=false;}}
 function prepareReport(event:SubmitEvent){event.preventDefault();if(!signed||pending)return;try{reportReview=signed.reports.plan(reportReason,reportDetails);failure='';}catch(error){failure=error instanceof Error?error.message:'Native report could not be reviewed.';}}
 async function confirmReport(){if(!signed||pending||!reportReview)return;const client=signed,current=generation,context=key(),prepared=reportReview;reportReview=undefined;pending=true;failure='';try{await client.reports.submit(prepared);if(disposed||current!==generation||context!==key())return;clearReport();reportResult='Report sent privately to this site’s moderators. It was not published publicly.';}catch(error){if(!disposed&&current===generation&&context===key()){clearReport();reportUncertain=client.reports.pending();failure=error instanceof Error?error.message:'Native report outcome could not be confirmed.';}}finally{if(!disposed&&current===generation)pending=false;}}
 function cancelRequest(){generation++;controller?.abort();signed?.dispose();signed=undefined;feed=undefined;signedView=undefined;pending=false;clearDraft();clearReport();failure='Discussion request cancelled. Nothing was replayed.';}
</script>
{#if comments?.thread && $user.public_account_follow_available}
 <section id="comments" class="space-y-5" data-thread-id={comments.thread.id} data-public-user-discussion={$user.id}>
  <p class="text-muted-foreground text-sm">Profile comments use your public identity, which may be a different account from your email login.</p>
  <IdentityGate show={!$signerIdentity.owner} />
  {#if !useSigner}
   {#if $signerIdentity.owner}<button type="button" disabled={pending} onclick={()=>useSigner=true}>Use public identity</button>{/if}
  {:else}<p>Signed discussion{accountName?' as '+accountName:''}. <button type="button" disabled={pending} onclick={()=>useSigner=false}>Read anonymously</button></p>{/if}
  <nav class="flex gap-3" aria-label="Profile comment order"><button type="button" disabled={pending||!oldest} onclick={()=>navigate(selectedPage,false)}>Latest</button><button type="button" disabled={pending||oldest} onclick={()=>navigate(selectedPage,true)}>Oldest</button></nav>
  {#if pending}<p role="status">Loading or updating profile comments…</p><button type="button" onclick={cancelRequest}>Cancel discussion request</button>{/if}
  {#if failure}<p role="alert">{failure}</p>{/if}
  {#if uncertain}<p role="alert">An earlier signed action has an unknown outcome. Do not repeat it. Reading cannot prove that it failed or clear its pending safeguard; ask this instance’s operator to reconcile it.</p>{/if}
  <button type="button" disabled={pending} onclick={()=>reload++}>Reload profile comments</button>
  {#if reportUncertain}<p role="alert">An earlier report has an unknown result. Do not repeat it. Reloading cannot confirm the result; ask this site’s support team to check it.</p>{/if}
  {#if reportResult}<p role="status">{reportResult}</p>{/if}
  {#if useSigner}<p><a class="underline" href="/reports/signed/native">View my discussion reports</a>. Reading never clears an uncertain submission safeguard.</p>{/if}
  {#if signedView&&canReport}<button disabled={pending||reportUncertain} onclick={()=>openReport({type:'thread'})}>Report profile discussion</button>{/if}
  {#if signedView&&!uncertain}
   <p>{signedView.thread.locked?'This discussion is locked.':'This discussion is open.'}</p>
   {#if signedView.actions.threadVote}<div class="flex gap-3">{#if !signedView.thread.locked||signedView.threadVote===1}<button disabled={pending} onclick={()=>prepare({operation:'thread_vote',vote:signedView?.threadVote===1?0:1})}>{signedView.threadVote===1?'Remove profile upvote':'Upvote profile'}</button>{/if}{#if !signedView.thread.locked||signedView.threadVote===-1}<button disabled={pending} onclick={()=>prepare({operation:'thread_vote',vote:signedView?.threadVote===-1?0:-1})}>{signedView.threadVote===-1?'Remove profile downvote':'Downvote profile'}</button>{/if}</div>{/if}
   {#if signedView.actions.create||editing!==undefined}
    <form onsubmit={compose} class="space-y-2"><label>{editing===undefined?(parentId===undefined?'New profile comment':'Reply to comment '+parentId):'Edit comment '+editing}<textarea aria-label="Profile comment text" bind:value={draft} disabled={pending||!!review} class="w-full" /></label><p>Comments can contain up to 50 KB of text; very long comments may need shortening.</p><button type="submit" disabled={pending||!!review}>Review comment</button>{#if editing!==undefined||parentId!==undefined}<button type="button" disabled={pending} onclick={clearDraft}>Cancel reply or edit</button>{/if}</form>
   {/if}
  {/if}
  {#if review}
   <section aria-label="Review signed discussion action" class="space-y-2 border p-3"><p>Review your {review.intent.operation} as {accountName}. This affects only the discussion on this site.</p>{#if review.intent.body!==undefined}<div class="typography comment" {@attach Typography.exec}>{@html Utils.parseComment(review.intent.body)}</div>{/if}{#if review.intent.vote!==undefined}<p>New vote: {review.intent.vote}</p>{/if}{#if review.intent.operation==='delete'}<p>This moderator action permanently deletes the reply.</p>{/if}<button type="button" disabled={pending} onclick={confirm}>Confirm signed action</button><button type="button" disabled={pending} onclick={()=>review=undefined}>Cancel review</button></section>
  {/if}
  {#if reportView&&!reportUncertain}
   <section aria-label="Native discussion report" class="space-y-2 border p-3"><p>You are reporting this {reportView.target.type} as {accountName}. Only this site’s moderators receive the report; it is not sent to the original publisher.</p>
    {#if !reportReview}<form onsubmit={prepareReport}><label>Report reason<select aria-label="Report reason" bind:value={reportReason} disabled={pending}><option value="">Choose a reason</option>{#each reportView.reasons as reason}<option value={reason.id}>{reason.reason}{reason.detailsRequired?' (details required)':''}</option>{/each}</select></label><label>Report details<textarea aria-label="Report details" bind:value={reportDetails} disabled={pending}/></label><p>Report details can contain up to 20 KB of text.</p><button type="submit" disabled={pending}>Review report</button></form>
    {:else}<section aria-label="Review native discussion report"><p>Reason: {reportView.reasons.find(reason=>reason.id===reportReview?.reasonId)?.reason}</p><p>{String(reportReview.details)}</p><button type="button" disabled={pending} onclick={confirmReport}>Confirm private report</button></section>{/if}
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
     {#if signedView&&canReport&&reply.state==='active'}<button disabled={pending||reportUncertain} onclick={()=>openReport({type:'reply',replyId:reply.id,version:reply.version})}>Report comment #{reply.number}</button>{/if}
     {#if signedView&&!uncertain}
      {@const actions=signedView.actions.replies[String(reply.id)]}
      <div class="flex flex-wrap gap-3">
       {#if signedView.actions.create&&reply.state==='active'}<button disabled={pending||!!review} onclick={()=>replyTo(reply)}>Reply to #{reply.number}</button>{/if}
       {#if actions?.update}<button disabled={pending||!!review} onclick={()=>edit(reply)}>Edit #{reply.number}</button>{/if}
       {#if actions?.remove}<button disabled={pending||!!review} onclick={()=>prepare({operation:'remove',replyId:reply.id})}>Remove #{reply.number}</button>{/if}
       {#if actions?.delete}<button disabled={pending||!!review} onclick={()=>prepare({operation:'delete',replyId:reply.id})}>Permanently delete #{reply.number}</button>{/if}
       {#if actions?.vote}{#if !signedView.thread.locked&&reply.state==='active'||reply.vote===1}<button disabled={pending||!!review} onclick={()=>prepare({operation:'reply_vote',replyId:reply.id,vote:reply.vote===1?0:1})}>{reply.vote===1?'Remove upvote':'Upvote'} #{reply.number}</button>{/if}{#if !signedView.thread.locked&&reply.state==='active'||reply.vote===-1}<button disabled={pending||!!review} onclick={()=>prepare({operation:'reply_vote',replyId:reply.id,vote:reply.vote===-1?0:-1})}>{reply.vote===-1?'Remove downvote':'Downvote'} #{reply.number}</button>{/if}{/if}
      </div>
     {/if}
    </article>
   {/each}
   <nav class="flex gap-3" aria-label="Profile comment pages"><button type="button" disabled={pending||selectedPage===1} onclick={()=>navigate(selectedPage-1)}>Previous comments</button><span>Page {selectedPage}</span><button type="button" disabled={pending||selectedPage>=1000000||selectedPage*feed.limit>=feed.total} onclick={()=>navigate(selectedPage+1)}>Next comments</button></nav>
  {/if}
 </section>
{:else}<p role="status">Comments are unavailable for this resource.</p>{/if}
