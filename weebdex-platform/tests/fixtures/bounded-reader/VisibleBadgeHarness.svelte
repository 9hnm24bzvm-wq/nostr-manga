<script lang="ts">
  // Mount actual retained title consumers after the production overlay chain.
  // This harness is qualification source, never a production route or UI stub.
  import MangaActions from '../../../frontend/src/routes/(app)/title/MangaActions.svelte';
  import MangaChapterListSection from '../../../frontend/src/routes/(app)/title/MangaChapterListSection.svelte';
  import { setManga, setReadHistory } from '$lib/Context';
  import { writable } from 'svelte/store';
  let { manga, wholeTitleHistory, observeHistory }: { manga: Manga; wholeTitleHistory?: Record<string, Record<string, null>>; observeHistory?: (value: unknown) => void } = $props();
  const history = writable(wholeTitleHistory);
  setManga(() => manga); setReadHistory(history);
  $effect(() => { observeHistory?.($history); });
</script>
<MangaActions />
<MangaChapterListSection />
