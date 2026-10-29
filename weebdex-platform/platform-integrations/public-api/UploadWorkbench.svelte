<script lang="ts">
 import {onDestroy,untrack} from 'svelte';
 import {Session} from '$lib/stores';
 import {createUploadWorkbench,type UploadWorkItem} from './upload-workbench';
 import {loadUploadAccount,publicUploadOwner,type PublicUploadOwner} from './native-upload-client';
 import UploadSchedulesPanel from './UploadSchedulesPanel.svelte';
 let entries=$state<UploadWorkItem[]>([]),busy=$state(false),message=$state(''),hasMore=$state(false),owner=$state<PublicUploadOwner>();
 let generation=0,disposed=false,client:Awaited<ReturnType<typeof createUploadWorkbench>>|undefined;
 const label=(entry:UploadWorkItem)=>entry.needsReconciliation?'Check publishing status':entry.state==='open'?'Private staged pages':entry.state==='scheduled'?'Scheduled or needs attention':entry.state==='committed'?'Upload accepted':'Cancelled';
 async function refresh(){if(busy||!$Session?.identity?.id)return;const captured=$Session,run=++generation;const current=()=>{if(disposed||run!==generation||captured!==$Session)throw Error('Account view changed.');};busy=true;message='';entries=[];owner?.dispose();owner=undefined;client?.dispose();client=undefined;
  try{const next=await createUploadWorkbench(captured.identity.id,current);current();client=next;const result=await next.list();current();entries=result.entries;hasMore=result.hasMore;if(entries.length){const target=entries[0].mangaId,user=await loadUploadAccount(current);current();owner=publicUploadOwner(()=>target,user,'manga','caller');}}
  catch(error){try{current();message=error instanceof Error?error.message:'Saved uploads could not be checked.';}catch{}}
  finally{try{current();busy=false;}catch{}}
 }
 $effect(()=>{const session=$Session;untrack(()=>{generation++;client?.dispose();owner?.dispose();client=undefined;owner=undefined;entries=[];message='';hasMore=false;busy=false;if(session?.identity?.id)void refresh();});});
 onDestroy(()=>{disposed=true;generation++;client?.dispose();owner?.dispose();});
</script>
<section class="mx-auto w-full max-w-4xl space-y-4 p-4">
 <h1 class="text-2xl font-semibold">Your uploads</h1>
 <p>Resume private pages, check publishing results, or manage scheduled chapters. Checking status never submits an upload again.</p>
 {#if !$Session?.identity?.id}<p><a class="underline" href="/auth/login">Sign in</a> to see your private upload work.</p>{:else}
 <button type="button" class="rounded border px-3 py-2" disabled={busy} onclick={refresh}>{busy?'Checking saved work…':'Refresh saved work'}</button>
 {#if message}<p role="alert">{message}</p>{/if}
 {#if !busy&&!message&&!entries.length}<p>No saved uploads were found for this account. Open a title and choose Upload chapter to begin.</p>{/if}
 {#if hasMore}<p role="status">Showing the 50 most recently updated uploads. Older work is still kept; contact the site team if a recovery item is missing.</p>{/if}
 <ul class="space-y-3">{#each entries as entry (entry.id)}<li class="rounded border p-3 space-y-2"><h2 class="font-medium">{entry.title}</h2><p>{label(entry)}</p><p class="text-sm">Updated {new Date(entry.updatedAt).toLocaleString()}</p>{#if entry.needsReconciliation}<p>The publishing result needs to be checked. Do not create a duplicate upload.</p>{/if}<a class="underline" href={entry.href}>{entry.state==='committed'&&!entry.needsReconciliation&&entry.resultChapterId?'Open chapter':'Open saved upload'}</a><details><summary>Support details</summary><p>Upload reference: {entry.id}</p></details></li>{/each}</ul>
 {#if owner}<UploadSchedulesPanel {owner}/>{/if}
 {/if}
</section>
