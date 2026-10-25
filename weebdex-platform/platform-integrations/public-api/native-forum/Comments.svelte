<script lang="ts">
 import {onDestroy} from 'svelte';
 import {page} from '$app/state';
 import {Comments} from '$lib/components/comments';
 import {useComments} from '$lib/components/comments/context.svelte';
 import {signerIdentity} from '../../local-library/publication';
 import {nativeDiscussionTarget,type NativeDiscussionTarget} from './context';
 import {readNativeDiscussionTarget} from './reader';
 import SignedDiscussion from './SignedDiscussion.svelte';
 let {class:className,target,resolveThread=false}:{class?:string;target?:NativeDiscussionTarget;resolveThread?:boolean}=$props();
 const comments=useComments();
 let resolved=$state<NativeDiscussionTarget>(),resolving=$state(false),failure=$state(''),reload=$state(0);
 let selected=$state<{mode:'browser'|'signed';scope:string}>();
 let generation=0,disposed=false,controller:AbortController|undefined;
 const direct=$derived.by(()=>{if(target===undefined||!comments?.thread)return undefined;try{return nativeDiscussionTarget(target,comments.thread.id);}catch{return undefined;}});
 const currentTarget=$derived(target===undefined?resolved:direct);
 const scope=$derived(JSON.stringify([comments?.thread?.id,currentTarget?.kind,currentTarget?.targetId,currentTarget?.threadId,$signerIdentity.owner,$signerIdentity.generation]));
 // A changed signing identity/target invalidates the choice. It never silently
 // replaces signed authority with a browser cookie, or one signer with another.
 const mode=$derived(selected?.scope===scope?selected.mode:undefined);
 const lookupKey=()=>JSON.stringify([comments?.thread?.id,resolveThread,target===undefined,page.url.pathname]);
 $effect(()=>{
  const context=lookupKey(),threadId=comments?.thread?.id;reload;
  const current=++generation;controller?.abort();resolved=undefined;resolving=false;failure='';
  if(target!==undefined||!resolveThread||!threadId)return;
  const request=new AbortController();controller=request;
  const check=()=>{if(disposed||request.signal.aborted||current!==generation||context!==lookupKey())throw Error('Forum discussion context changed');};
  resolving=true;
  void readNativeDiscussionTarget(threadId,request.signal,check).then(value=>{check();resolved=value??undefined;}).catch(error=>{if(!disposed&&!request.signal.aborted&&current===generation)failure=error instanceof Error?error.message:'The signed discussion target could not be checked.';}).finally(()=>{if(!disposed&&current===generation)resolving=false;});
  return()=>request.abort();
 });
 onDestroy(()=>{disposed=true;generation++;controller?.abort();});
</script>
{#if comments?.thread}
 <div class={className} data-native-discussion-mode={mode??'unselected'}>
  <nav class="mb-3 flex flex-wrap gap-3" aria-label="Discussion account">
   <button type="button" aria-pressed={mode==='browser'} onclick={()=>selected={mode:'browser',scope}}>Browser login</button>
   <button type="button" aria-pressed={mode==='signed'} disabled={!currentTarget||!$signerIdentity.owner||resolving} onclick={()=>selected={mode:'signed',scope}}>Signed account</button>
  </nav>
  <p class="text-muted-foreground mb-3 text-sm">Browser login uses the existing native session. Signed account uses only the native account explicitly bound to the selected signing identity. These accounts may differ.</p>
  {#if resolving}<p role="status">Checking this thread’s stored signed discussion target…</p>{/if}
  {#if failure}<p role="alert">{failure}</p><button type="button" disabled={resolving} onclick={()=>reload++}>Retry signed target lookup</button>
  {:else if !resolving&&!currentTarget}<p role="status">No verified manga, chapter or group target is available for signed actions on this thread. Browser login remains available.</p>{/if}
  {#if !$signerIdentity.owner}<p><a class="underline" href="/settings/identity">Connect a signing identity</a> to use a signed account.</p>{/if}
  {#if mode==='browser'}<Comments />
  {:else if mode==='signed'&&currentTarget}
   {#key scope}<SignedDiscussion target={currentTarget} />{/key}
  {:else}<p role="status">{selected===undefined?'Choose Browser login or Signed account to open this discussion.':'The account or discussion changed. Choose Browser login or Signed account again; no draft or action was moved.'}</p>{/if}
 </div>
{:else}<p class={className} role="status">Comments are unavailable for this resource.</p>{/if}
