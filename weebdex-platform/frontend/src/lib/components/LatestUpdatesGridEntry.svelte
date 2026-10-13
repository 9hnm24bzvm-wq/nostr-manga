<script lang="ts">
	import { Locale, Utils } from "$lib";
	import { MessagesSquareIcon, UsersIcon } from "@lucide/svelte";
	import ImageLoader from "./ui/ImageLoader.svelte";

	const { chapter }: { chapter: Chapter } = $props();
	// svelte-ignore state_referenced_locally
	const title = Utils.chapterHeadline(chapter);
</script>

<div class="group text-muted-foreground relative mt-2 pb-5 text-[0.8125rem]">
	<a class="absolute inset-0" href="/chapter/{chapter.id}" {title}></a>
	<div class="mb-1 flex items-center gap-1.5">
		<div class="size-4.5 shrink-0">
			<ImageLoader
				src="/images/flags/{Locale.to3166(chapter.language)}.svg"
				alt={Locale.toLanguage(chapter.language)}
			/>
		</div>
		<h3 class="text-foreground truncate font-medium">
			{title}
		</h3>
	</div>
	<div class="flex items-center gap-1">
		<UsersIcon class="shrink-0" size="14" />
		<div class="truncate">
			{#if chapter.relationships.groups?.length}
				{#each chapter.relationships.groups as group, i}
					{#if i > 0}
						{#if i === chapter.relationships.groups.length - 1}
							{" & "}
						{:else}
							{", "}
						{/if}
					{/if}
					<a class="hover:text-accent-foreground relative" href="/group/{group.id}" title={group.name}>
						{group.name}
					</a>
				{/each}
			{:else}
				<em class="pr-2">No Group</em>
			{/if}
		</div>
	</div>
	<div class="flex items-center gap-1">
		<div class="w-0 flex-auto">
			<time datetime={chapter.published_at ?? chapter.created_at}>
				{(chapter.published_at ?? chapter.created_at).ago()}
			</time>
		</div>
		<a
			class="hover:text-accent-foreground relative flex shrink-0 items-center gap-1.5"
			href="/forums/threads/{chapter.relationships.thread.id}"
			title="Go to comments"
		>
			<MessagesSquareIcon class="shrink-0" size="16" />
			{#if chapter.relationships.thread.relationships.stats.replies}
				{chapter.relationships.thread.relationships.stats.replies.short()}
			{/if}
		</a>
	</div>
</div>
