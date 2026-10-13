<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import { buttonVariants } from "$lib/components/ui/button";
	import { Session } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import { KeySquareIcon, LibraryIcon, Settings2Icon, SettingsIcon } from "@lucide/svelte";

	const { children } = $props();

	const tabs: Record<
		string,
		{
			name: string;
			icon: any;
			hidden?: boolean;
		}
	> = $derived({
		preferences: {
			name: "Preferences",
			icon: Settings2Icon
		},
		library: {
			name: "Library",
			icon: LibraryIcon,
			hidden: !$Session
		},
		account: {
			name: "Account",
			icon: SettingsIcon,
			hidden: !$Session
		},
		client: {
			name: "API Clients",
			icon: KeySquareIcon,
			hidden: !$Session
		}
	});

	$effect(() => {
		Session.isValid().then(() => {
			const strs = window.location.pathname.split("/");
			const k = strs[strs.length - 1];
			if (!(k in tabs) || tabs[k].hidden) {
				goto(`/settings/${Object.keys(tabs)[0]}`, { replaceState: true });
			}
		});
	});
</script>

{#snippet navs()}
	<div class="border-b-2">
		<nav class="text-muted-foreground/80 -mb-0.5 flex flex-nowrap overflow-auto whitespace-nowrap">
			{#each Object.entries(tabs) as [key, item]}
				{#if !item.hidden}
					{@const pathname = `/settings/${key}`}
					<a
						class={buttonVariants({
							class: cn("border-b-2 border-b-transparent !bg-transparent", {
								"border-accent": page.url.pathname === pathname
							}),
							variant: "ghost"
						})}
						class:!text-accent-foreground={page.url.pathname === pathname}
						href={pathname}
					>
						<item.icon />
						{item.name}
					</a>
				{/if}
			{/each}
		</nav>
	</div>
{/snippet}

<section class="mx-auto flex w-full max-w-200 flex-auto flex-col space-y-5">
	<header class="shrink-0">
		{@render navs()}
	</header>
	<div class="flex flex-auto flex-col">
		{@render children?.()}
	</div>
</section>
