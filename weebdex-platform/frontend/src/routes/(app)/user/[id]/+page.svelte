<script lang="ts">
	import { ExternalLink, Typography, UserLink, Utils } from "$lib";
	import { badgeVariants } from "$lib/components/ui/badge/badge.svelte";
	import { buttonVariants } from "$lib/components/ui/button";
	import ImageLoader from "$lib/components/ui/ImageLoader.svelte";
	import { getUser } from "$lib/Context";
	import { ChessRook, Crown, Mail } from "@lucide/svelte";
	import { onMount } from "svelte";

	const user = getUser();
	let isMounted = $state(false);

	onMount(async () => {
		isMounted = true;
	});
</script>

{#snippet links()}
	{@const keys = Object.keys(UserLink.names)}
	{#if keys.some(key => key in $user)}
		<section class="space-y-1">
			<header>
				<h2 class="text-base font-semibold">Where to find</h2>
			</header>
			<ul class="flex flex-wrap items-center gap-1">
				{#each keys as key}
					{#if key in $user}
						{@const url = UserLink.build(key, <string>$user[<keyof typeof $user>key])}
						{#if key === "contact_email" || Utils.isValidURL(url)}
							{@const name = UserLink.names[key]}
							<li>
								<a
									class={badgeVariants({
										class: "bg-muted border-none text-sm font-normal",
										variant: "outline"
									})}
									href={url}
									rel="nofollow noreferrer noopener"
									target="_blank"
								>
									<div class="size-3">
										{#if key === "contact_email"}
											<Mail class="size-3" />
										{:else}
											<ImageLoader
												class="rounded-full"
												src={`https://www.google.com/s2/favicons?sz=14&domain_url=${ExternalLink.base(url)}`}
												alt={name}
											/>
										{/if}
									</div>
									{name}
								</a>
							</li>
						{/if}
					{/if}
				{/each}
			</ul>
		</section>
	{/if}
{/snippet}

<section class="space-y-1">
	<header>
		<h2 class="text-base font-semibold">Information</h2>
	</header>
	<div class="typography max-w-200 space-y-2" {@attach Typography.exec}>
		{#if $user.description}
			{@html Utils.parseDescription($user.description)}
		{:else}
			<p>No biography has been provided.</p>
		{/if}
	</div>
</section>

{@render links()}

{#if $user.relationships?.groups?.length}
	<section class="space-y-1">
		<header>
			<h2 class="text-base font-semibold">Groups</h2>
		</header>
		<div class="flex flex-wrap items-center gap-1">
			{#each $user.relationships.groups as group (group)}
				<a
					class={buttonVariants({
						class: "w-0 min-w-[40%] flex-auto justify-start border-none px-3 py-4",
						variant: "outline"
					})}
					href="/group/{group.id}"
					title={group.name}
				>
					<figure class="bg-card size-5 shrink-0 overflow-hidden rounded-full">
						<ImageLoader class="size-full object-cover" src="/images/avatar.webp" />
					</figure>
					<span class="w-0 flex-auto truncate">
						{group.name}
					</span>
					{#if group.relationships?.members?.some(member => member.id === $user.id && member.is_leader)}
						<span class="shrink-0">
							<Crown />
						</span>
					{:else if group.relationships?.members?.some(member => member.id === $user.id && member.is_officer)}
						<span class="shrink-0">
							<ChessRook />
						</span>
					{/if}
				</a>
			{/each}
		</div>
	</section>
{/if}
