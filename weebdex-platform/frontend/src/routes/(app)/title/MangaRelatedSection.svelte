<script lang="ts">
	import { replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { API, Constants } from "$lib";
	import ErrorComponent from "$lib/components/ErrorComponent.svelte";
	import MangaSearchEntryCard from "$lib/components/MangaSearchEntryCard.svelte";
	import * as Empty from "$lib/components/ui/empty";
	import Grid from "$lib/components/ui/Grid.svelte";
	import { Spinner } from "$lib/components/ui/spinner";
	import ViewModeToggles from "$lib/components/ViewModeToggles.svelte";
	import { ContentRating } from "$lib/constants/Manga";
	import { getManga } from "$lib/Context";
	import { mangaView, prefs } from "$lib/stores";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";

	const manga = $derived(getManga()());
	let relations = $state<Relation[] | undefined>(manga.relationships.relations);
	const filteredRelations = $derived(
		relations?.filter(
			e =>
				(e.content_rating === ContentRating.Safe && $prefs.show_safe) ||
				(e.content_rating === ContentRating.Suggestive && $prefs.show_suggestive) ||
				(e.content_rating === ContentRating.Erotica && $prefs.show_erotic) ||
				(e.content_rating === ContentRating.Pornographic && $prefs.show_hentai)
		)
	);

	let isGrid = $derived($mangaView === "grid");
	let isMounted = $state(false);
	let error = $state<Constants.Error.Error>();

	onMount(async () => {
		const state = page.state as {
			_$Related_relations: typeof relations;
		};
		if (state._$Related_relations) {
			relations = state._$Related_relations;
		} else if (!relations?.length) {
			const { href } = window.location;
			try {
				const res = await API.Manga.Relations(manga.id);
				if (window.location.href !== href) {
					return;
				}

				if (res?.status === StatusCodes.OK && res.body) {
					relations = res.body;
					replaceState(page.url.href, {
						...page.state,
						_$Related_relations: $state.snapshot(relations)
					});
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
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
{:else if isMounted}
	{#if filteredRelations?.length}
		<section class="space-y-2">
			<header class="flex items-center gap-2">
				<ViewModeToggles />
			</header>
			<Grid class="gap-3" breakpoints={isGrid ? Constants.MangaPageGridBreakpoints : { 0: 2, 768: 1 }}>
				{#each filteredRelations as related (related)}
					<article>
						<MangaSearchEntryCard manga={related} {isGrid} />
					</article>
				{/each}
			</Grid>
		</section>
	{:else}
		<Empty.Root>
			<Empty.Header>
				<Empty.Title>No relations found</Empty.Title>
				<Empty.Description>Some relations may be hidden due to your content rating preferences.</Empty.Description>
			</Empty.Header>
		</Empty.Root>
	{/if}
{:else}
	<Empty.Root>
		<Spinner class="size-10" />
	</Empty.Root>
{/if}
