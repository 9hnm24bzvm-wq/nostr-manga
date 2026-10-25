<script lang="ts">
  import { Constants } from '$lib';
  import { useReader } from '$lib/components/reader/context.svelte';
  import GroupPage from './GroupPage.svelte';
  import Status from './Status.svelte';
  import { plainAction } from './presentation';
  const reader = useReader(), controller = reader.navigation, readerPrefs = reader.prefs;
  const rtl = $derived($readerPrefs.direction === Constants.Reader.Direction.RTL);
  const disabled = $derived($controller.pending || !$controller.current);
</script>
<div class="space-y-2">
  <div class="flex items-center gap-1">
    <button type="button" aria-label={rtl ? 'Last chapter' : 'First chapter'} disabled={disabled} onclick={event => plainAction(event, () => rtl ? reader.gotoLastChapter() : reader.gotoFirstChapter())}>«</button>
    <button type="button" aria-label={rtl ? 'Next chapter' : 'Previous chapter'} disabled={disabled || !(rtl ? $controller.next : $controller.previous)} onclick={event => plainAction(event, () => rtl ? reader.gotoNextChapter() : reader.gotoPreviousChapter())}>‹</button>
    <GroupPage {controller} showStatus={false} />
    <button type="button" aria-label={rtl ? 'Previous chapter' : 'Next chapter'} disabled={disabled || !(rtl ? $controller.previous : $controller.next)} onclick={event => plainAction(event, () => rtl ? reader.gotoPreviousChapter() : reader.gotoNextChapter())}>›</button>
    <button type="button" aria-label={rtl ? 'First chapter' : 'Last chapter'} disabled={disabled} onclick={event => plainAction(event, () => rtl ? reader.gotoFirstChapter() : reader.gotoLastChapter())}>»</button>
  </div>
  <Status {controller} refresh />
</div>
