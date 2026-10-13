<script lang="ts">
	import { Utils } from "$lib";
	import { BookIcon } from "@lucide/svelte";
	import { buttonVariants } from "./ui/button";

	const { manga }: { manga: Manga } = $props();
</script>

<article>
	<a
		class={buttonVariants({
			class: "w-full justify-start border-none px-3 py-4",
			variant: "outline"
		})}
		href="/title/{manga.id}{Utils.makeSlug(manga.title)}"
		title={manga.title}
	>
		<span class="flex size-4 shrink-0 items-center">
			<BookIcon class="size-full" />
		</span>
		<span class="w-0 flex-auto truncate font-semibold">{manga.title}</span>
		<span class="shrink-0 font-semibold">[Title]</span>
	</a>
</article>
