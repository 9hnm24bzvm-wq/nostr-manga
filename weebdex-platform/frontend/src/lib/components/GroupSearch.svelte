<script lang="ts">
	import { page } from "$app/state";
	import { Constants } from "$lib";
	import * as Empty from "$lib/components/ui/empty";
	import { onMount } from "svelte";
	import ErrorComponent from "./ErrorComponent.svelte";
	import Grid from "./ui/Grid.svelte";
	import { buttonVariants } from "./ui/button";
	import Spinner from "./ui/spinner/spinner.svelte";

	let {
		feed = $bindable(),
		onSearch
	}: {
		feed: ScanlationGroupFeedResponse | undefined;
		onSearch: () => Promise<void>;
	} = $props();

	let isMounted = $state(false);
	let error = $state<Constants.Error.Error>();

	onMount(async () => {
		const state = page.state as {
			_$GroupSearch_feed: typeof feed;
		};
		if (state._$GroupSearch_feed) {
			feed = state._$GroupSearch_feed;
		} else {
			try {
				await onSearch();
			} catch (e) {
				if (e instanceof Constants.Error.Error) error = e;
				else {
					console.error(e);
					error = Constants.Error.Unknown;
				}
			}
		}
		isMounted = true;
	});
</script>

{#if error}
	<ErrorComponent {error} />
{:else if isMounted && feed}
	<Grid class="gap-1" breakpoints={{ 0: 4, 1368: 3, 768: 2, 525: 1 }}>
		{#each feed.data as group}
			<article>
				<a
					class={buttonVariants({
						class: "w-full justify-start border-none",
						variant: "outline"
					})}
					href="/group/{group.id}"
					title={group.name}
				>
					<span class="truncate">
						{group.name}
					</span>
				</a>
			</article>
		{/each}
	</Grid>
{:else}
	<Empty.Root>
		<Spinner class="size-10" />
	</Empty.Root>
{/if}
