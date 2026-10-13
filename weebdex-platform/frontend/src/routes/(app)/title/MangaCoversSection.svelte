<script lang="ts">
	import { PUBLIC_COVER_URL } from "$env/static/public";
	import { API, Constants, Locale } from "$lib";
	import ErrorComponent from "$lib/components/ErrorComponent.svelte";
	import AspectRatio from "$lib/components/ui/aspect-ratio/aspect-ratio.svelte";
	import * as Empty from "$lib/components/ui/empty";
	import Grid from "$lib/components/ui/Grid.svelte";
	import ImageLoader from "$lib/components/ui/ImageLoader.svelte";
	import { useLightbox } from "$lib/components/ui/lightbox";
	import * as Select from "$lib/components/ui/select";
	import { Spinner } from "$lib/components/ui/spinner";
	import { getManga } from "$lib/Context";
	import { Expand } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";

	const lightbox = useLightbox();
	const manga = $derived(getManga()());
	let covers = $state<Cover[]>();
	let availableLocales = $state<string[]>([]);
	let selectedLocales = $state<string[]>([]);

	const filteredCovers = $derived(
		selectedLocales.length ? covers?.filter(v => selectedLocales.includes(v.language)) : covers
	);
	const coverUrls = $derived(filteredCovers?.map(v => `${PUBLIC_COVER_URL}/${manga.id}/${v.id}${v.ext}`));

	let isMounted = $state(false);
	let error = $state<Constants.Error.Error>();

	onMount(async () => {
		const { href } = window.location;
		try {
			const res = await API.Cover.List(manga.id);
			if (window.location.href !== href) {
				return;
			}

			if (res?.status === StatusCodes.OK) {
				covers = res.body ?? [];
				if (res.body) {
					availableLocales = Array.from(new Set(res.body.map(v => v.language)));
				}
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
		isMounted = true;
	});
</script>

{#snippet entry(cover: Cover, i: number)}
	{@const title = cover.volume ? `Volume ${cover.volume}` : "No Volume"}
	<article>
		<a
			class="group relative block overflow-hidden rounded-md"
			href={coverUrls![i]}
			onclick={e => {
				if (!e.ctrlKey) {
					e.preventDefault();
					e.stopPropagation();
					e.stopImmediatePropagation();

					lightbox.set(...coverUrls!);
					lightbox.open(i);
				}
			}}
		>
			<ImageLoader src="{PUBLIC_COVER_URL}/{manga.id}/{cover.id}.512.webp" alt={title}>
				{#snippet child({ src })}
					<figure>
						<img {src} alt={title} />
						<div
							class="absolute inset-0 flex items-center justify-center bg-black/40 opacity-0 transition-opacity group-hover:opacity-100"
						>
							<Expand class="text-white/85" size="40" />
						</div>
						<div class="gradient pointer-events-none absolute inset-0"></div>
					</figure>
					<div class="pointer-events-none absolute inset-0 flex h-full flex-col pb-2">
						<div class="flex flex-auto">
							<ImageLoader
								class="-mt-[0.35rem] size-8"
								src="/images/flags/{Locale.to3166(cover.language)}.svg"
								alt={Locale.toLanguage(cover.language)}
							/>
						</div>
						<h3 class="line-clamp-2 shrink-0 px-2 text-balance break-words text-white group-hover:line-clamp-4">
							{title}
						</h3>
					</div>
				{/snippet}
				{#snippet loader()}
					<AspectRatio ratio={10 / 14} class="bg-card overflow-hidden rounded-md" />
				{/snippet}
			</ImageLoader>
		</a>
	</article>
{/snippet}

{#if error}
	<ErrorComponent {error} />
{:else if isMounted && filteredCovers?.length && coverUrls?.length}
	<section class="space-y-2">
		<header>
			<Select.Root type="multiple" bind:value={selectedLocales}>
				<Select.Trigger class="w-50 border-none">
					{#if selectedLocales.length}
						{selectedLocales.map(v => Locale.toLanguage(v)).join(", ")}
					{:else}
						Any Locale
					{/if}
				</Select.Trigger>
				<Select.Content>
					{#each availableLocales as locale}
						<Select.Item value={locale}>
							<span class="size-5 shrink-0">
								<ImageLoader
									class="size-5"
									src="/images/flags/{Locale.to3166(locale)}.svg"
									alt={Locale.toLanguage(locale)}
								/>
							</span>
							{Locale.toLanguage(locale)}
						</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
		</header>
		<Grid class="gap-2" breakpoints={Constants.MangaPageGridBreakpoints}>
			{#each filteredCovers as cover, i (cover)}
				{@render entry(cover, i)}
			{/each}
		</Grid>
	</section>
{:else}
	<Empty.Root>
		<Spinner class="size-10" />
	</Empty.Root>
{/if}

<style lang="scss">
	.gradient {
		background-image:
			linear-gradient(180deg, transparent 50%, rgba(0, 0, 0, 0.5)),
			linear-gradient(300deg, transparent 80%, rgba(0, 0, 0, 0.5));
	}
	h3 {
		text-shadow:
			0 0 0.2rem rgba(0, 0, 0, 0.75),
			0 0 0.15rem rgba(0, 0, 0, 0.75),
			0 0 0.1rem rgba(0, 0, 0, 0.75);
	}
</style>
