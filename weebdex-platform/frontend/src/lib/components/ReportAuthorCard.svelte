<script lang="ts">
	import ImageLoader from "./ui/ImageLoader.svelte";
	import { buttonVariants } from "./ui/button";

	const { author }: { author: Author } = $props();
</script>

<article>
	<a
		class={buttonVariants({
			class: "w-full justify-start border-none px-3 py-4",
			variant: "outline"
		})}
		href="/author/{author.id}"
		title={author.name}
	>
		<figure class="bg-card size-5 shrink-0 overflow-hidden rounded-full">
			<ImageLoader class="size-full object-cover" src="/images/avatar.webp" />
		</figure>
		<span class="w-0 flex-auto truncate">
			{author.name}
		</span>
		<span class="shrink-0">[Author]</span>
	</a>
</article>
