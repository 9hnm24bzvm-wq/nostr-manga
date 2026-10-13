<script lang="ts">
	import { UsersIcon } from "@lucide/svelte";
	import { buttonVariants } from "./ui/button";

	const { group }: { group: ScanlationGroup } = $props();
</script>

<article>
	<a
		class={buttonVariants({
			class: "w-full justify-start border-none px-3 py-4",
			variant: "outline"
		})}
		href="/group/{group.id}"
		title={group.name}
	>
		<span class="flex size-4 shrink-0 items-center">
			<UsersIcon class="size-full" />
		</span>
		<span class="w-0 flex-auto truncate font-semibold">{group.name}</span>
		<span class="shrink-0 font-semibold">[Group]</span>
	</a>
</article>
