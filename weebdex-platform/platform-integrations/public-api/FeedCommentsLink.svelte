<script lang="ts">
  import { MessagesSquareIcon } from '@lucide/svelte';
  import { feedCommentsDestination } from './feed-comments';
  let { chapter, manga }: { chapter: Chapter; manga?: Manga } = $props();
  const destination = $derived(feedCommentsDestination(chapter, manga));
</script>
{#if destination}
  <a class="hover:text-accent-foreground relative inline-flex max-w-full items-center gap-1.5 text-sm" href={destination.href} title={destination.title}>
    <MessagesSquareIcon class="shrink-0" size={16} />
    <span>{destination.label}{#if destination.replies} ({destination.replies.toLocaleString()}){/if}</span>
  </a>
{:else}
  <span class="text-muted-foreground text-sm">Comments not loaded</span>
{/if}
