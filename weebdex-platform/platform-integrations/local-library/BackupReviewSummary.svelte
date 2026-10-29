<script lang="ts">
 import type {ImportReview,DeviceBackupRestoreReview} from './library';
 const {imported,restore}:{imported:ImportReview;restore?:DeviceBackupRestoreReview}=$props();
 const entries=$derived(imported.snapshot.records);
 const count=(family:string)=>entries.filter(row=>row.family===family).length;
 const listNames=$derived(entries.filter(row=>row.family==='lists').slice(0,10).map(row=>row.name||'Unnamed list'));
</script>
<section class="space-y-3" aria-label="Backup contents">
 <p class="font-medium break-words">{imported.source}</p>
 <p class="text-muted-foreground text-sm">Saved {new Date(imported.snapshot.exported_at).toLocaleString()}</p>
 <ul class="list-inside list-disc text-sm">
  <li>{count('series')} saved titles</li>
  <li>{count('chapters')} chapter progress entries</li>
  <li>{count('lists')} lists{listNames.length?': '+listNames.join(', '):''}{count('lists')>listNames.length?'…':''}</li>
  <li>{count('members')} titles added to lists</li>
 </ul>
 {#if imported.collisions}<p class="text-sm">{imported.collisions} saved items are already on this device. The restore choice below decides which version to keep.</p>{/if}
 {#if restore}
  <div class="space-y-2 rounded border p-3" aria-label="Restore result">
   <h3 class="font-medium">{restore.mode==='merge'?'Combine backup with this library':'Replace this library with the backup'}</h3>
   <p>{restore.summary.records} saved items after restoring.</p>
   <p>{restore.mode==='merge'?'For matching items, the more recent save wins. If both were saved at the same time, this device’s version is kept.':'Your current saved titles, progress and lists will be replaced. Earlier versions and past removals are kept for recovery.'}</p>
   {#if restore.summary.merged}<p>{restore.summary.merged} earlier versions will be kept in recovery history.</p>{/if}
   {#if restore.summary.deleted}<p>{restore.summary.deleted} older items will stay removed, including list entries whose title or list was removed.</p>{/if}
   {#if restore.summary.unresolved}<p role="status">{restore.summary.unresolved} saved items could not be matched to this catalogue. They are kept for recovery, but may not be available to read.</p>{/if}
   <p>This browser keeps its own library identity. Nothing is published.</p>
  </div>
 {/if}
 <details class="text-sm"><summary class="cursor-pointer">Backup details for support</summary>
  <p class="break-all">Original site: {imported.snapshot.scope.origin}</p>
  {#if imported.snapshot.version===2}<p class="break-all">Backup device reference: {imported.snapshot.ownerId}</p>{/if}
  <p class="break-all">Catalogue reference: {imported.snapshot.scope.catalogNamespace??'Not recorded'}</p>
  <p>Total saved entries: {entries.length}. Matching entries: {imported.collisions??0}.</p>
 </details>
</section>
