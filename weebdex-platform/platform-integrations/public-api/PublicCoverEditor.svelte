<script lang="ts">
 import { onMount, onDestroy } from 'svelte';
 import { createSignedCoverController, type SignedCoverController, type CoverRecord } from './cover-client';
 import type { MangaView } from './types';
 let { title, accountId, disabled=false, assertCurrent, onChanged, onState }: { title:MangaView; accountId:string; disabled?:boolean; assertCurrent:()=>void; onChanged:()=>Promise<void>; onState:(state:{pending:boolean;busy:boolean})=>void }=$props();
 let controller=$state<SignedCoverController>();
 let disposed=false,previewEpoch=0;
 let covers=$state<CoverRecord[]>([]),total=$state(0),ready=$state(false),busy=$state(false),pending=$state(false),message=$state('');
 let file=$state<File|null>(null),locale=$state(''),volume=$state(''),description=$state(''),main=$state(true),selectedId=$state(''),preview=$state<string|null>(null);
 const selected=$derived(covers.find(row=>row.id===selectedId));
 const canRemove=$derived(!!controller?.account.roles.some(role=>['moderator','staff','sysop'].includes(role))&&!!controller?.account.permissions.includes('cover.delete'));
 function active(){if(disposed)throw Error('Cover editor closed');assertCurrent();}
 function publishState(){pending=controller?.pending()??false;onState({pending,busy});}
 function clearPreview(){previewEpoch++;if(preview)URL.revokeObjectURL(preview);preview=null;}
 function choose(row?:CoverRecord){clearPreview();selectedId=row?.id??'';locale=row?.locale??title.language;volume=row?.volume??'';description=row?.description??'';main=row?.main??true;file=null;}
 async function list(){if(!controller)return;const result=await controller.read();active();covers=result.data;total=result.total;const row=covers.find(row=>row.id===selectedId);if(row)choose(row);}
 async function run(action:()=>Promise<void>){if(busy||disposed)return;busy=true;message='';publishState();try{active();await action();}catch(error){if(!disposed)message=error instanceof Error?error.message:'Cover operation was not confirmed.';}finally{if(!disposed){busy=false;publishState();}}}
 onMount(()=>{void run(async()=>{controller=await createSignedCoverController(active,()=>title,accountId);active();locale=title.language;await list();ready=true;if(controller.pending())message='A previous cover command has an unconfirmed outcome. This editor is read-only; inspect actual covers and ask the instance operator to confirm before any retry. Nothing was replayed.';});});
 onDestroy(()=>{disposed=true;controller?.dispose();clearPreview();});
 async function upload(){if(!controller||!file||disabled||pending)return;await run(async()=>{const accepted=await controller!.upload(file!,{locale,volume,description,main});active();selectedId=accepted.id;file=null;await list();await onChanged();active();message='Original cover stored byte-for-byte on this instance. No derivative or original signed publication was created.';});}
 async function update(){if(!controller||!selected||disabled||pending)return;const row=selected;await run(async()=>{await controller!.update(row,{locale,volume,description,main});active();await list();await onChanged();active();message='Instance cover metadata saved. Original image bytes and signed source are unchanged.';});}
 async function remove(){if(!controller||!selected||!canRemove||disabled||pending)return;const row=selected;if(!confirm('Approve this instance-local cover removal now? The cover will be hidden here. Original signed publications are unchanged.'))return;await run(async()=>{await controller!.remove(row);active();choose();await list();await onChanged();active();message='Instance cover removed with moderator approval. No original event was deleted.';});}
 async function showOriginal(){if(!controller||!selected?.original)return;const row=selected,epoch=++previewEpoch;await run(async()=>{const blob=await controller!.preview(row);active();if(epoch!==previewEpoch||selectedId!==row.id)return;const url=URL.createObjectURL(blob);if(disposed||epoch!==previewEpoch){URL.revokeObjectURL(url);return;}if(preview)URL.revokeObjectURL(preview);preview=url;message='Loaded the exact signed/hash-checked original preview.';});}
</script>
<section class="space-y-3 rounded border p-3" aria-label="Signed original covers">
 <h4 class="font-medium">Original cover</h4>
 <p class="text-sm">Upload exact PNG, JPEG or WebP originals up to 25 MiB. No compression or derivative is generated. Submission needs a hosted cover in the title’s original language.</p>
 {#if message}<p role="status">{message}</p>{/if}
 {#if pending}<p>A cover command is unconfirmed. Reloading only reads current records and does not replay or clear that command.</p>{/if}
 <button type="button" class="rounded border px-3 py-2" disabled={!ready||busy} onclick={()=>run(list)}>Read current covers</button>
 {#if total>100}<p>Showing the latest {covers.length} of {total} covers. This bounded editor does not claim the list is complete.</p>{/if}
 <label class="block">Saved cover<select class="block rounded border p-2" disabled={!ready||busy} value={selectedId} onchange={event=>choose(covers.find(row=>row.id===(event.target as HTMLSelectElement).value))}><option value="">New original upload</option>{#each covers as row}<option value={row.id}>{row.locale??'unknown locale'} · {row.volume||'no volume'} · {row.id}{row.main?' · main':''}</option>{/each}</select></label>
 <fieldset disabled={!ready||busy||pending||disabled} class="space-y-2">
  {#if !selected}<label class="block">Original image<input class="block" type="file" accept="image/png,image/jpeg,image/webp,.png,.jpg,.jpeg,.webp" onchange={event=>{file=(event.currentTarget as HTMLInputElement).files?.[0]??null;}} /></label>{/if}
  <label class="block">Cover language<input class="block rounded border p-2" bind:value={locale} maxlength="12" /></label>
  <label class="block">Volume<input class="block rounded border p-2" bind:value={volume} maxlength="32" /></label>
  <label class="block">Description<textarea class="block w-full rounded border p-2" bind:value={description} maxlength="20000"></textarea></label>
  <label><input type="checkbox" bind:checked={main} /> Use as this instance’s main cover</label>
  {#if selected}<button type="button" class="rounded border px-3 py-2" onclick={update}>Save cover metadata</button>{#if canRemove}<button type="button" class="rounded border px-3 py-2" onclick={remove}>Approve instance cover removal</button>{/if}{:else}<button type="button" class="rounded border px-3 py-2" disabled={!file} onclick={upload}>Upload exact original cover</button>{/if}
 </fieldset>
 {#if selected?.original}<p class="break-all text-xs">Actual original SHA-256: {selected.original.sha256}. {selected.original.bytes} bytes. Cover version {selected.version}.</p><button type="button" class="rounded border px-3 py-2" disabled={busy} onclick={showOriginal}>Load signed original preview</button>{/if}
 {#if preview}<img src={preview} alt="Verified original cover" class="max-h-72 max-w-full object-contain" />{/if}
</section>
