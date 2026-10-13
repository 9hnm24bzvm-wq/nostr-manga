<script lang="ts">
	import { UserIcon } from "@lucide/svelte";
	import { buttonVariants } from "./ui/button";

	const { user }: { user: User } = $props();
</script>

<article>
	<a
		class={buttonVariants({
			class: "w-full justify-start border-none px-3 py-4",
			variant: "outline"
		})}
		href="/user/{user.id}"
		title={user.name}
	>
		<span class="flex size-4 shrink-0 items-center">
			<UserIcon class="size-full" />
		</span>
		<span class="w-0 flex-auto truncate font-semibold">{user.name}</span>
		<span class="shrink-0 font-semibold">[User]</span>
	</a>
</article>
