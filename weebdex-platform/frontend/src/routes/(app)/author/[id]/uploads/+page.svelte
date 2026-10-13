<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import { API } from "$lib";
	import ChapterFeed from "$lib/components/ChapterFeed.svelte";
	import * as ButtonGroup from "$lib/components/ui/button-group";
	import { Button } from "$lib/components/ui/button/index.js";
	import * as Empty from "$lib/components/ui/empty";
	import { Spinner } from "$lib/components/ui/spinner";
	import { getAuthor, getChaptersCount } from "$lib/Context";
	import { prefs } from "$lib/stores/prefs.js";
	import { cn } from "$lib/Utils";

	const author = getAuthor();
	const chaptersCount = $derived(getChaptersCount()());
</script>

{#if chaptersCount}
	<section class="flex flex-auto flex-col space-y-2">
		<header>
			<ButtonGroup.Root>
				<Button
					class={cn("text-muted-foreground border-0", {
						"!text-accent-foreground !border-accent border-b-2": page.url.searchParams.get("order") !== "asc"
					})}
					variant="outline"
					onclick={() => {
						const q = new URLSearchParams(window.location.search);
						q.delete("order");

						const target = window.location.pathname + q.toString();
						if (target !== window.location.pathname + window.location.search) {
							goto(target);
						}
					}}
				>
					Latest
				</Button>
				<Button
					class={cn("text-muted-foreground border-0", {
						"!text-accent-foreground !border-accent border-b-2": page.url.searchParams.get("order") === "asc"
					})}
					variant="outline"
					onclick={() => {
						const q = new URLSearchParams(window.location.search);
						q.set("order", "asc");

						const target = window.location.pathname + q.toString();
						if (target !== window.location.pathname + window.location.search) {
							goto(target);
						}
					}}
				>
					Oldest
				</Button>
			</ButtonGroup.Root>
		</header>
		{#key page.url.search}
			<ChapterFeed
				onSearch={(n?: number) => {
					const q = new URLSearchParams();
					prefs.applyMangaPreferences(q);
					prefs.applyChapterPreferences(q);

					q.set("authorOrArtist", $author.id);
					if (n) q.set("page", n.toString());
					if (page.url.searchParams.has("order")) q.set("order", "asc");
					q.set("limit", "100");

					return API.Chapter.Search(q);
				}}
			/>
		{/key}
	</section>
{:else if Number.isInteger(chaptersCount)}
	<Empty.Root>
		<Empty.Header>
			<Empty.Title>No uploads found</Empty.Title>
			<Empty.Description>This group has not uploaded anything yet.</Empty.Description>
		</Empty.Header>
	</Empty.Root>
{:else}
	<Empty.Root>
		<Spinner class="size-10" />
	</Empty.Root>
{/if}
