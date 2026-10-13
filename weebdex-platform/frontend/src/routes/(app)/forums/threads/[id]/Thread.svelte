<script lang="ts">
	import { PUBLIC_OPENGRAPH_URL, PUBLIC_SITE_NAME } from "$env/static/public";
	import { Constants, Typography, Utils } from "$lib";
	import { useComments } from "$lib/components/comments/context.svelte";
	import AspectRatio from "$lib/components/ui/aspect-ratio/aspect-ratio.svelte";
	import * as Avatar from "$lib/components/ui/avatar";
	import * as Breadcrumb from "$lib/components/ui/breadcrumb/index.js";
	import { Share2 } from "@lucide/svelte";

	const comments = useComments();
	const author = $derived(comments.thread.relationships.user ?? comments.thread.relationships.author);

	const createdAt = $derived(new Date(comments.thread.created_at));
	const role = $derived(
		author?.roles
			? author.roles
					.slice()
					.sort((a, b) => Constants.User.RoleOrder.indexOf(a) - Constants.User.RoleOrder.indexOf(b))[0]
			: undefined
	);
</script>

<svelte:head>
	{#if comments.thread.relationships.user}
		<title>
			Forums: {author.name} - User Profiles - {PUBLIC_SITE_NAME}
		</title>
	{:else if comments.thread.relationships.manga}
		<title>
			Forums: {comments.thread.relationships.manga.title} - Series Discussions - {PUBLIC_SITE_NAME}
		</title>
	{:else if comments.thread.relationships.chapter}
		<title>
			Forums: {comments.thread.relationships.chapter.relationships.manga.title} - {Utils.chapterHeadline(
				comments.thread.relationships.chapter
			)} - Chapter Discussions - {PUBLIC_SITE_NAME}
		</title>
	{:else if comments.thread.relationships.group}
		<title>
			Forums: {comments.thread.relationships.group.name} - Group Discussions - {PUBLIC_SITE_NAME}
		</title>
	{:else}
		<title>Forums: {comments.thread.subject} - General Discussions - {PUBLIC_SITE_NAME}</title>
	{/if}
</svelte:head>

<header class="space-y-5">
	<Breadcrumb.Root>
		<Breadcrumb.List class="!gap-1 font-medium">
			<Breadcrumb.Item>
				<Breadcrumb.Link href="/" class="type">Home</Breadcrumb.Link>
			</Breadcrumb.Item>
			<Breadcrumb.Separator />
			<Breadcrumb.Item>Forums</Breadcrumb.Item>
			<Breadcrumb.Separator />
			<Breadcrumb.Item>
				{#if comments.thread.relationships.user}
					User Profiles
				{:else if comments.thread.relationships.manga}
					Series Discussions
				{:else if comments.thread.relationships.chapter}
					Chapter Discussions
				{:else if comments.thread.relationships.group}
					Scanlation Groups
				{:else}
					General Discussions
				{/if}
			</Breadcrumb.Item>
		</Breadcrumb.List>
	</Breadcrumb.Root>

	<div class="text-accent-foreground rounded-md text-center">
		{#if comments.thread.relationships.manga}
			<h1 class="text-2xl font-semibold">
				<a
					class="hover:underline"
					href="/title/{comments.thread.relationships.manga.id}{Utils.makeSlug(
						comments.thread.relationships.manga.title
					)}"
				>
					{comments.thread.relationships.manga.title}
				</a>
			</h1>
		{:else if comments.thread.relationships.chapter}
			<h1 class="text-2xl font-semibold">
				<a
					class="hover:underline"
					href="/title/{comments.thread.relationships.chapter.relationships.manga.id}{Utils.makeSlug(
						comments.thread.relationships.chapter.relationships.manga.title
					)}"
				>
					{comments.thread.relationships.chapter.relationships.manga.title}
				</a>
			</h1>
			<h2 class="font-medium">
				<a class="text-accent-foreground/85 hover:underline" href="/chapter/{comments.thread.relationships.chapter.id}">
					{Utils.chapterHeadline(comments.thread.relationships.chapter)}
				</a>
			</h2>
		{:else if comments.thread.relationships.group}
			<h1 class="text-2xl font-semibold">
				<a class="hover:underline" href="/group/{comments.thread.relationships.group.id}">
					{comments.thread.relationships.group.name}
				</a>
			</h1>
		{/if}
	</div>
	{#if comments.thread.relationships.manga || comments.thread.relationships.chapter || comments.thread.relationships.group}
		<AspectRatio ratio={1200 / 630}>
			<div class="flex items-center justify-center">
				{#if comments.thread.relationships.manga}
					<a
						href="/title/{comments.thread.relationships.manga.id}{Utils.makeSlug(
							comments.thread.relationships.manga.title
						)}"
						title={comments.thread.relationships.manga.title}
					>
						<img
							src="{PUBLIC_OPENGRAPH_URL}/manga/{comments.thread.relationships.manga.id}"
							alt={comments.thread.relationships.manga.title}
						/>
					</a>
				{:else if comments.thread.relationships.chapter}
					<a
						href="/chapter/{comments.thread.relationships.chapter.id}"
						title={Utils.chapterHeadline(comments.thread.relationships.chapter)}
					>
						<img
							src="{PUBLIC_OPENGRAPH_URL}/chapter/{comments.thread.relationships.chapter.id}"
							alt={Utils.chapterHeadline(comments.thread.relationships.chapter)}
						/>
					</a>
				{:else if comments.thread.relationships.group}
					<a
						href="/group/{comments.thread.relationships.group.id}{Utils.makeSlug(
							comments.thread.relationships.group.name
						)}"
						title={comments.thread.relationships.group.name}
					>
						<img
							src="{PUBLIC_OPENGRAPH_URL}/group/{comments.thread.relationships.group.id}"
							alt={comments.thread.relationships.group.name}
						/>
					</a>
				{/if}
			</div>
		</AspectRatio>
	{:else}
		<div
			class="bg-card ring-accent/50 flex flex-col gap-2 rounded-md p-2 transition duration-1000 md:flex-row [&[data-highlight]]:ring-2"
			class:min-h-50={comments.thread.body}
			id="t{comments.thread.id}"
		>
			{#if author}
				<div
					class="bg-background flex shrink-0 gap-4 rounded-md p-2 md:block md:w-34 md:space-y-2 md:border-r md:bg-transparent md:pr-4"
				>
					<div class="mx-auto w-24 shrink-0 md:w-[unset]">
						<AspectRatio ratio={1 / 1}>
							<a href="/user/{author.id}">
								<Avatar.Root class="size-full !rounded-md">
									<Avatar.Fallback class="!rounded-md text-2xl uppercase">
										{#if author}
											{author.name.charAt(0)}
										{:else}
											?
										{/if}
									</Avatar.Fallback>
								</Avatar.Root>
							</a>
						</AspectRatio>
					</div>
					{#if author}
						<div class="flex-auto space-y-2 text-xs md:text-center">
							<div>
								<a
									class="text-accent-foreground block text-sm font-semibold break-words hover:underline"
									href="/user/{author.id}"
								>
									{author?.name ?? "Anonymous"}
								</a>
								{#if role}
									<strong
										class="block font-medium"
										class:text-orange-600={role === Constants.User.Role.Sysop ||
											role === Constants.User.Role.Staff ||
											role === Constants.User.Role.Moderator}
										class:text-green-600={role === Constants.User.Role.PowerUploader}
										class:text-emerald-600={role === Constants.User.Role.Contributor}
										class:text-sky-600={role === Constants.User.Role.GroupLeader}
										class:text-cyan-600={role === Constants.User.Role.GroupOfficer}
										>{Constants.User.RoleText[role]}</strong
									>
								{/if}
							</div>
							{#if author.relationships?.stats}
								<div>
									{#if author.relationships.stats.chapters}
										<div class="flex items-center justify-between">
											<strong class="font-semibold">Uploads:</strong>
											<span>{author.relationships.stats.chapters.short()}</span>
										</div>
									{/if}
									<div class="flex items-center justify-between">
										<strong class="font-semibold">Messages:</strong>
										<span>{author.relationships.stats.messages.short()}</span>
									</div>
									{#if author.relationships.stats.points !== 0}
										<div class="flex items-center justify-between">
											<strong class="font-semibold">Points:</strong>
											<span>{author.relationships.stats.points.short()}</span>
										</div>
									{/if}
								</div>
							{/if}
						</div>
					{/if}
				</div>
			{/if}
			<div class="flex flex-auto flex-col md:w-0">
				<header
					class="text-muted-foreground flex shrink-0 items-center justify-between border-b pb-1 text-xs font-medium"
				>
					<div>
						<a class="hover:underline" href="/forums/threads/{comments.thread.id}">
							<time datetime={comments.thread.created_at}>
								{#if createdAt.getTime() > Date.now() - 24 * 3600 * 1000}
									{comments.thread.created_at.ago()}
								{:else}
									{createdAt.toLocaleString("en-US", { dateStyle: "full", timeStyle: "short" })}
								{/if}
							</time>
						</a>
					</div>
					<div class="flex items-center gap-2">
						<button>
							<Share2 size="14" />
						</button>
					</div>
				</header>
				<div
					class="flex flex-auto flex-col space-y-2 py-2"
					class:pb-0={!comments.thread.body}
					class:pb-5={comments.thread.body}
				>
					<div class="typography comment flex-auto" {@attach Typography.exec}>
						{#if comments.thread.relationships.user}
							{@html Utils.parseComment(
								comments.thread.relationships.user.description ?? "No biography has been provided."
							)}
						{:else}
							{@html Utils.parseComment(comments.thread.body)}
						{/if}
					</div>
				</div>
			</div>
		</div>
	{/if}
</header>
