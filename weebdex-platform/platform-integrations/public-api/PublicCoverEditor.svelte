<script lang="ts">
 import { onMount, onDestroy } from 'svelte';
 import { Locale } from '$lib';
 import { createSessionCoverController, coverCommandCapabilities, type SignedCoverController, type CoverRecord } from './session-cover-client';
 import type { MangaView } from './types';
 let { nativeAccount, title, accountId, disabled=false, assertCurrent, onChanged, onState }: { nativeAccount:string; title:MangaView; accountId:string; disabled?:boolean; assertCurrent:()=>void; onChanged:()=>Promise<void>; onState:(state:{pending:boolean;busy:boolean})=>void }=$props();
 let controller=$state<SignedCoverController>();
 let disposed=false,previewEpoch=0;
 let covers=$state<CoverRecord[]>([]),total=$state(0),offset=$state(0),pageReady=$state(false),ready=$state(false),busy=$state(false),pending=$state(false),message=$state('');
 const languages=Object.entries({...Locale.Languages,ta:'Tamil'}).filter(([code])=>/^[a-z]{2}(-[a-z]{2})?$/.test(code)).sort((a,b)=>a[1].localeCompare(b[1]));
 let customLanguage=$state(false);
 let file=$state<File|null>(null),locale=$state(''),volume=$state(''),description=$state(''),main=$state(true),selectedId=$state(''),preview=$state<string|null>(null);
 const selected=$derived(covers.find(row=>row.id===selectedId));
 const capabilities=$derived(coverCommandCapabilities(controller?.account,title));
 const canEdit=$derived(capabilities.edit&&pageReady),canRemove=$derived(capabilities.remove&&pageReady);
 const dirty=$derived(Boolean(file)||(selected?locale!==(selected.locale??'')||volume!==selected.volume||description!==selected.description||main!==selected.main:locale!==title.language||volume!==''||description!==''||!main));
 function active(){if(disposed)throw Error('Cover editor closed');assertCurrent();}
 function publishState(){pending=controller?.pending()??false;onState({pending,busy});}
 function clearPreview(){previewEpoch++;if(preview)URL.revokeObjectURL(preview);preview=null;}
 function choose(row?:CoverRecord){clearPreview();customLanguage=false;selectedId=row?.id??'';locale=row?row.locale??'':title.language;volume=row?.volume??'';description=row?.description??'';main=row?.main??true;file=null;}
 async function list(nextOffset=offset,keep=selectedId){
  if(!controller)return;
  pageReady=false;clearPreview();
  try{
   let result=await controller.read(nextOffset);active();
   if(result.data.length===0&&nextOffset>0){
    nextOffset=result.total?Math.floor((result.total-1)/100)*100:0;
    result=await controller.read(nextOffset);active();
   }
   if(result.data.length===0&&result.total>0){
    offset=0;throw Error('Cover pages changed again. Read current covers to refresh.');
   }
   covers=result.data;total=result.total;offset=result.offset;
   choose(covers.find(row=>row.id===keep));pageReady=true;
  }catch(error){if(!disposed){covers=[];choose();}throw error;}
 }
 async function page(nextOffset:number){
  if(!ready||busy||nextOffset<0)return;
  if(dirty&&!confirm('Discard unsaved cover changes and read this page?'))return;
  await run(()=>list(nextOffset,nextOffset===offset?selectedId:''));
 }
 function select(value:string){
  if(!pageReady||busy||value===selectedId)return;
  if(dirty&&!confirm('Discard unsaved cover changes and select another cover?'))return;
  choose(covers.find(row=>row.id===value));
 }
 async function run(action:()=>Promise<void>){if(busy||disposed)return;busy=true;message='';publishState();try{active();await action();}catch(error){if(!disposed)message=error instanceof Error?error.message:'Cover operation was not confirmed.';}finally{if(!disposed){busy=false;publishState();}}}
 onMount(()=>{void run(async()=>{controller=await createSessionCoverController(nativeAccount,active,()=>title,accountId);active();locale=title.language;ready=true;await list();if(controller.pending())message='A previous cover command has an unconfirmed outcome. This editor is read-only; inspect actual covers and ask the instance operator to confirm before any retry. Nothing was replayed.';});});
 onDestroy(()=>{disposed=true;controller?.dispose();clearPreview();});
 async function upload(){if(!controller||!file||!canEdit||disabled||pending)return;await run(async()=>{const accepted=await controller!.upload(file!,{locale,volume,description,main});active();selectedId=accepted.id;file=null;await list(0,accepted.id);await onChanged();active();message='Cover saved. The original image is preserved unchanged.';});}
 async function update(){if(!controller||!selected||!canEdit||disabled||pending)return;const row=selected;await run(async()=>{await controller!.update(row,{locale,volume,description,main});active();await list();await onChanged();active();message='Cover details saved. The original image is unchanged.';});}
 async function remove(){if(!controller||!selected||!canRemove||disabled||pending)return;const row=selected;if(!confirm('Approve this instance-local cover removal now? The cover will be hidden here. Original signed publications are unchanged.'))return;await run(async()=>{await controller!.remove(row);active();choose();await list();await onChanged();active();message='Instance cover removed with moderator approval. No original event was deleted.';});}
 async function showOriginal(){if(!controller||!selected?.original)return;const row=selected,epoch=++previewEpoch;await run(async()=>{const blob=await controller!.preview(row);active();if(epoch!==previewEpoch||selectedId!==row.id)return;const url=URL.createObjectURL(blob);if(disposed||epoch!==previewEpoch){URL.revokeObjectURL(url);return;}if(preview)URL.revokeObjectURL(preview);preview=url;message='Cover preview loaded.';});}
</script>
<section class="space-y-3 rounded border p-3" aria-label="Cover images">
 <h4 class="font-medium">Cover image</h4>
 <p class="text-sm">Choose a PNG, JPEG or WebP image up to 25 MiB. Your original file is kept unchanged. To submit a draft, add a cover in the title’s original language.</p>
 {#if message}<p role="status">{message}</p>{/if}
 {#if pending}<p>A cover command is unconfirmed. Reloading only reads current records and does not replay or clear that command.</p>{/if}
 <button type="button" class="rounded border px-3 py-2" disabled={!ready||busy} onclick={()=>page(offset)}>Refresh covers</button>
 {#if pageReady}<p aria-live="polite">{#if total}Showing {offset+1}-{offset+covers.length} of {total} covers, newest first.{:else}No saved covers.{/if}</p>{/if}
 <nav aria-label="Cover pages" class="flex gap-2"><button type="button" class="rounded border px-3 py-2" disabled={!pageReady||busy||offset===0} onclick={()=>page(Math.max(0,offset-100))}>Previous covers</button><button type="button" class="rounded border px-3 py-2" disabled={!pageReady||busy||total-offset<=100} onclick={()=>page(offset+100)}>Next covers</button></nav>
 <label class="block">Saved cover<select class="block rounded border p-2" disabled={!pageReady||busy} value={selectedId} onchange={event=>{const target=event.target as HTMLSelectElement;select(target.value);target.value=selectedId;}}><option value="">Add a new cover</option>{#each covers as row}<option value={row.id}>{Locale.toLanguage(row.locale??'')||'Unknown language'}{row.volume?' · Volume '+row.volume:''}{row.main?' · main':''}</option>{/each}</select></label>
 <fieldset disabled={!pageReady||busy||pending||disabled} class="space-y-2">
  <fieldset disabled={!canEdit} class="space-y-2">
  {#if !selected}<label class="block">Image file<input class="block" type="file" accept="image/png,image/jpeg,image/webp,.png,.jpg,.jpeg,.webp" onchange={event=>{file=(event.currentTarget as HTMLInputElement).files?.[0]??null;}} /></label>{/if}
  <label class="block">Cover language<select class="block rounded border p-2" value={customLanguage?'__other__':locale} onchange={event=>{const value=event.currentTarget.value;customLanguage=value==='__other__';locale=customLanguage?'':value;}}>
   {#if !customLanguage&&!languages.some(([code])=>code===locale)}<option value={locale}>{Locale.toLanguage(locale)||'Unspecified language'}</option>{/if}
   {#each languages as [code,name]}<option value={code}>{name}</option>{/each}<option value="__other__">Other language…</option>
  </select></label>
  {#if customLanguage}<label class="block">Other language code<input class="block rounded border p-2" bind:value={locale} placeholder="For example, ta or pt-br" /></label>{/if}
  <label class="block">Volume<input class="block rounded border p-2" bind:value={volume} maxlength="32" /></label>
  <label class="block">Description<textarea class="block w-full rounded border p-2" bind:value={description} maxlength="20000"></textarea></label>
  <label><input type="checkbox" bind:checked={main} /> Use as the main cover</label>
  {#if selected}<button type="button" class="rounded border px-3 py-2" onclick={update}>Save cover details</button>{:else}<button type="button" class="rounded border px-3 py-2" disabled={!file} onclick={upload}>Upload cover</button>{/if}
  </fieldset>
  {#if selected&&canRemove}<button type="button" class="rounded border px-3 py-2" onclick={remove}>Approve instance cover removal</button>{/if}
 </fieldset>
 {#if selected?.original}<details class="text-xs"><summary>Image file details</summary><p class="break-all">SHA-256: {selected.original.sha256}. {selected.original.bytes} bytes. Cover version {selected.version}.</p></details><button type="button" class="rounded border px-3 py-2" disabled={busy} onclick={showOriginal}>Preview cover</button>{/if}
 {#if preview}<img src={preview} alt="Verified original cover" class="max-h-72 max-w-full object-contain" />{/if}
</section>
