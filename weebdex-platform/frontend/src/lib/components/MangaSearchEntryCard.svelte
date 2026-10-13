<script lang="ts">
	import { PUBLIC_COVER_URL } from "$env/static/public";
	import { API, Constants, Locale, Mutex, Typography, Utils } from "$lib";
	import { Session } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import { Check, Plus } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import isMobile from "is-mobile";
	import { AspectRatio } from "./ui/aspect-ratio";
	import { badgeVariants } from "./ui/badge";
	import ImageLoader from "./ui/ImageLoader.svelte";
	import { ScrollArea } from "./ui/scroll-area";
	import { Spinner } from "./ui/spinner";

	const {
		manga,
		isGrid,
		isEdit,
		onClick
	}: {
		manga: (Manga & { type?: Constants.Manga.RelationType }) | Relation;
		isGrid?: boolean;
		isEdit?: boolean;
		onClick?: (manga: Manga) => void;
	} = $props();
	const mutex = new Mutex();

	// svelte-ignore state_referenced_locally
	const cover = manga.relationships.cover
		? `${PUBLIC_COVER_URL}/${manga.id}/${manga.relationships.cover.id}.${manga.relationships.cover.dimensions[1] < manga.relationships.cover.dimensions[0] ? "512" : "256"}.webp`
		: "/images/cover.webp";
	// svelte-ignore state_referenced_locally
	const slug = Utils.makeSlug(manga.title);

	// svelte-ignore state_referenced_locally
	let isFollowed = $state($Session?.follows && manga.id in $Session.follows);
	const isPrivate = window.location.pathname.startsWith("/library");

	const follow = () =>
		mutex.exec(async () => {
			if (!$Session) {
				return;
			}
			const res = await API.MangaFollow.Update(manga.id, {
				status: Constants.Manga.ReadingStatus.Reading,
				notification: true
			});
			if (res?.status === StatusCodes.NO_CONTENT) {
				isFollowed = true;
				if ($Session) {
					Object.assign(($Session.follows ??= {}), {
						[manga.id]: [Constants.Manga.ReadingStatus.Reading, 1]
					});
				}
			}
		});

	const unfollow = () =>
		mutex.exec(async () => {
			if (!$Session) {
				return;
			}
			const res = await API.MangaFollow.Unfollow(manga.id);
			if (res?.status === StatusCodes.NO_CONTENT) {
				isFollowed = false;
				if ($Session?.follows && manga.id in $Session.follows) {
					delete $Session.follows[manga.id];
				}
			}
		});
</script>

{#snippet followButton(className?: string)}
	<button
		class={cn(
			"bg-primary absolute flex size-7 items-center justify-center rounded-md group-hover:opacity-85 hover:opacity-100",
			className
		)}
		class:opacity-0={!isMobile() && (!isFollowed || isGrid)}
		type="button"
		onclick={e => {
			e.preventDefault();
			e.stopPropagation();
			e.stopImmediatePropagation();

			if (isFollowed) unfollow();
			else follow();
		}}
	>
		<Plus class={cn("text-primary-foreground size-5 scale-100 transition-all", ($mutex || isFollowed) && "scale-0")} />
		<Check
			class={cn("text-primary-foreground absolute size-5 scale-0 transition-all", !$mutex && isFollowed && "scale-100")}
		/>
		<Spinner class={cn("!text-primary-foreground absolute scale-0 transition-all", $mutex && "scale-100 ")} />
	</button>
{/snippet}

{#snippet list()}
	<div
		class="group list-card flex gap-4"
		class:opacity-50={isPrivate && !isFollowed}
		class:hover:opacity-100={isPrivate && !isFollowed}
	>
		<figure class="w-[35%] shrink-0 md:w-[30%] lg:w-[25%]">
			<AspectRatio
				ratio={10 / 14}
				class={cn("bg-card overflow-hidden rounded-md", {
					"ring-primary ring-2": isFollowed && !isPrivate
				})}
			>
				{#snippet child({ props })}
					<a href="/title/{manga.id}{slug}" {...props}>
						<ImageLoader class="size-full object-cover object-center" src={cover} alt={manga.title} />
						<div class="gradient pointer-events-none absolute inset-0"></div>
						<ImageLoader
							class="absolute top-0 left-0 -mt-[0.25rem] size-6"
							src="/images/flags/{Locale.to3166(manga.language)}.svg"
							alt={Locale.toLanguage(manga.language)}
						/>
						{#if $Session}
							{@render followButton("right-1 bottom-1")}
						{/if}
					</a>
				{/snippet}
			</AspectRatio>
		</figure>
		<div class="flex w-0 flex-auto flex-col">
			{#if manga.type}
				<strong class="text-muted-foreground mb-1 block text-xs font-semibold tracking-wider uppercase">
					{Constants.Manga.RelationTypeText[manga.type]}
				</strong>
			{/if}
			<h3 class="hover:text-accent-foreground mb-2 line-clamp-2 text-base/5 font-semibold">
				<a href="/title/{manga.id}{slug}">{manga.title}</a>
			</h3>
			<div class="flex h-0 flex-auto flex-col space-y-2">
				<div class="flex flex-wrap gap-1">
					<a
						class={badgeVariants({ class: "border-none px-1 py-0 text-white", variant: "outline" })}
						class:!bg-green-700={manga.status === Constants.Manga.PublicationStatus.Ongoing}
						class:!bg-sky-700={manga.status === Constants.Manga.PublicationStatus.Completed}
						class:!bg-orange-700={manga.status === Constants.Manga.PublicationStatus.Hiatus}
						class:!bg-red-700={manga.status === Constants.Manga.PublicationStatus.Cancelled}
						href="/search{new URLSearchParams({ status: manga.status }).toString()}"
					>
						{Constants.Manga.PublicationStatusText[manga.status]}</a
					>
					{#if manga.content_rating != Constants.Manga.ContentRating.Safe}
						<a
							class={badgeVariants({ class: "border-none px-1 py-0 text-white", variant: "outline" })}
							class:!bg-yellow-700={manga.content_rating === Constants.Manga.ContentRating.Suggestive}
							class:!bg-orange-700={manga.content_rating === Constants.Manga.ContentRating.Erotica}
							class:!bg-red-700={manga.content_rating === Constants.Manga.ContentRating.Pornographic}
							href="/search{new URLSearchParams({ contentRating: manga.content_rating }).toString()}"
						>
							{Constants.Manga.ContentRatingText[manga.content_rating]}
						</a>
					{/if}
					{#if manga.demographic}
						<a
							class={badgeVariants({
								class: "bg-muted text-foreground/85 border-none px-1 py-0",
								variant: "outline"
							})}
							href="/search{new URLSearchParams({ demographic: manga.demographic }).toString()}"
						>
							{Constants.Manga.DemographicText[manga.demographic]}
						</a>
					{/if}
					{#if manga.relationships.tags?.length}
						{#each manga.relationships.tags
							?.filter(tag => [Constants.Tag.Group.Content, Constants.Tag.Group.Genre].includes(tag.group))
							.slice(0, 5) as tag}
							<a
								class={badgeVariants({
									class: "bg-muted text-foreground/85 border-none px-1 py-0",
									variant: "outline"
								})}
								href="/search{new URLSearchParams({ tag: tag.id }).toString()}"
							>
								{tag.name}
							</a>
						{/each}
					{/if}
				</div>
				<ScrollArea class="h-0 flex-auto">
					<div class="text-foreground/85 typography space-y-2" {@attach Typography.exec}>
						{#if manga.description}
							{@html Utils.parseDescriptionPlain(manga.description)}
						{:else}
							<p>Oops! This manga doesn't have a description yet.</p>
						{/if}
					</div>
					<div class="description-gradient pointer-events-none absolute inset-x-0 bottom-0 h-20 sm:h-10 lg:h-20"></div>
				</ScrollArea>
			</div>
		</div>
	</div>
{/snippet}

{#snippet grid()}
	<AspectRatio ratio={10 / 14}>
		{#snippet child({ props })}
			<a
				class="grid-card bg-card group relative block overflow-hidden rounded-md"
				class:cursor-pointer={isEdit}
				class:ring-2={isFollowed && !isPrivate && !isEdit}
				class:ring-primary={isFollowed && !isPrivate && !isEdit}
				class:opacity-50={isPrivate && !isFollowed}
				class:hover:opacity-100={isPrivate && !isFollowed}
				href={isEdit ? undefined : `/title/${manga.id}${slug}`}
				title={manga.title}
				onclick={ev => {
					if (isEdit) {
						ev.preventDefault();
						ev.stopPropagation();
						ev.stopImmediatePropagation();
						onClick?.(<Manga>manga);
					}
				}}
				{...props}
			>
				<figure>
					<ImageLoader class="absolute inset-0 size-full object-cover object-center" src={cover} alt={manga.title} />
					<div class="gradient pointer-events-none absolute inset-0"></div>
				</figure>
				<div class="absolute inset-0 flex flex-col">
					<div class="h-0 flex-auto">
						<ImageLoader
							class="-mt-[0.35rem] block w-8"
							src="/images/flags/{Locale.to3166(manga.language)}.svg"
							alt={Locale.toLanguage(manga.language)}
						/>
						{#if $Session && !isEdit}
							{@render followButton("right-1 top-1")}
						{/if}
					</div>
					<div class="shrink-0 p-2">
						<h3 class="line-clamp-2 font-medium text-balance break-words text-white group-hover:line-clamp-4">
							{manga.title}
						</h3>
					</div>
				</div>
			</a>
		{/snippet}
	</AspectRatio>
	{#if manga.type && !isEdit}
		<strong
			class="text-muted-foreground mt-1.5 block text-center text-[0.8125rem] font-semibold tracking-wider uppercase"
		>
			{Constants.Manga.RelationTypeText[manga.type]}
		</strong>
	{/if}
{/snippet}

{#if isGrid}
	{@render grid()}
{:else}
	{@render list()}
{/if}

<style lang="scss">
	.list-card {
		figure .gradient {
			background-image: linear-gradient(300deg, transparent 80%, rgba(0, 0, 0, 0.5));
		}

		.description-gradient {
			background-image: linear-gradient(0deg, var(--background), transparent);
		}
	}
	.grid-card {
		figure .gradient {
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
	}
</style>
