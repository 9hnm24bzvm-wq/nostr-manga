<script lang="ts">
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { API, Constants, Utils } from "$lib";
	import MangaEdit from "$lib/components/draft/manga/MangaEdit.svelte";
	import ErrorComponent from "$lib/components/ErrorComponent.svelte";
	import { buttonVariants } from "$lib/components/ui/button";
	import * as Empty from "$lib/components/ui/empty";
	import NotFound from "$lib/components/ui/NotFound.svelte";
	import { Spinner } from "$lib/components/ui/spinner";
	import { from } from "$lib/stores";
	import { ArrowLeftIcon } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";

	const { data } = $props();
	let covers = $state<Cover[]>([]);

	let isMounted = $state(false);
	let error = $state<Constants.Error.Error>();

	onMount(async () => {
		if (data.manga) {
			try {
				await Promise.all([getCovers()]);
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

	const getCovers = async () => {
		if (!data.manga?.id) return;

		const res = await API.Cover.List(data.manga.id);
		if (res?.status === StatusCodes.OK && res.body) {
			covers = res.body;
		}
	};
</script>

<svelte:head>
	{#if data.manga}
		<title>Edit: {data.manga.title} - {PUBLIC_SITE_NAME}</title>
	{/if}
</svelte:head>

{#if error}
	<ErrorComponent {error} />
{:else if isMounted}
	{#if data.manga}
		<section class="mx-auto flex w-full max-w-270 flex-auto flex-col space-y-5">
			<header class="flex items-center gap-2">
				{#if data.manga.state === Constants.State.Published}
					<a
						class={buttonVariants({ class: "rounded-full", variant: "ghost", size: "icon" })}
						href={$from && !$from.pathname.startsWith("/create") && !$from.pathname.endsWith("/edit")
							? $from.href
							: `/title/${data.manga.id}${Utils.makeSlug(data.manga.title)}`}
					>
						<ArrowLeftIcon class="size-5" />
					</a>
				{:else}
					<a
						class={buttonVariants({ class: "rounded-full", variant: "ghost", size: "icon" })}
						href={$from && !$from.pathname.startsWith("/create") && !$from.pathname.endsWith("/edit")
							? $from.href
							: "/drafts/title"}
					>
						<ArrowLeftIcon class="size-5" />
					</a>
				{/if}
				<h2 class="text-xl font-medium">Edit Title</h2>
			</header>
			<MangaEdit data={{ manga: data.manga, covers }} />
		</section>
	{:else}
		<NotFound />
	{/if}
{:else}
	<Empty.Root>
		<Spinner class="size-10" />
	</Empty.Root>
{/if}
