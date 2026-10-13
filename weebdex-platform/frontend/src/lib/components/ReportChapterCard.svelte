<script lang="ts">
	import { Constants, Locale, Utils } from "$lib";
	import { cn } from "$lib/Utils";
	import { Clock, User, Users } from "@lucide/svelte";

	const { class: className, chapter }: { class?: string; chapter: Chapter } = $props();
	const title = Utils.chapterHeadline(chapter);
	const role = chapter.relationships?.uploader?.roles
		? chapter.relationships.uploader.roles
				.slice()
				.sort((a, b) => Constants.User.RoleOrder.indexOf(a) - Constants.User.RoleOrder.indexOf(b))[0]
		: undefined;
</script>

<article class={cn("bg-input/30 hover:bg-input/50 relative rounded-md px-2.5 py-1.5", className)}>
	<a class="absolute inset-0" href="/chapter/{chapter.id}" {title}></a>
	<div class="flex items-center gap-2">
		<div class="flex w-0 flex-auto items-center gap-2">
			<img
				class="size-4 shrink-0"
				src="/images/flags/{Locale.to3166(chapter.language)}.svg"
				alt={Locale.toLanguage(chapter.language)}
			/>
			<h3 class="w-0 flex-auto truncate font-semibold">
				{title}
			</h3>
		</div>
		{#if chapter.published_at}
			<div class="flex w-32 shrink-0 items-center gap-2">
				{#if chapter.published_at}
					<div class="flex w-0 flex-auto items-center gap-1">
						<Clock class="shrink-0" size="14" />
						<time class="truncate">{chapter.published_at?.ago()}</time>
					</div>
				{/if}
			</div>
		{/if}
	</div>
	<div class="flex items-center gap-2">
		<div class="w-0 flex-auto">
			<div class="flex items-center gap-2">
				<Users class="ml-0.5 shrink-0" size="14" />
				{#if chapter.relationships.groups?.length}
					{#each chapter.relationships.groups as group}
						<a class="hover:text-accent-foreground relative truncate" href="/group/{group.id}" title={group.name}>
							{group.name}
						</a>
					{/each}
				{:else}
					<em class="truncate">No Group</em>
				{/if}
			</div>
		</div>
		{#if chapter.relationships?.uploader}
			<div class="flex w-32 shrink-0 items-center gap-2">
				{#if chapter.relationships.uploader}
					<div class="flex w-0 flex-auto items-center gap-1">
						<User class="shrink-0" size="14" />
						<a
							class="hover:text-accent-foreground relative w-0 flex-auto truncate"
							class:text-orange-600={role === Constants.User.Role.Sysop ||
								role === Constants.User.Role.Staff ||
								role === Constants.User.Role.Moderator}
							class:hover:!text-orange-500={role === Constants.User.Role.Sysop ||
								role === Constants.User.Role.Staff ||
								role === Constants.User.Role.Moderator}
							class:text-green-600={role === Constants.User.Role.PowerUploader}
							class:hover:!text-green-500={role === Constants.User.Role.PowerUploader}
							class:text-emerald-600={role === Constants.User.Role.Contributor}
							class:hover:!text-emerald-500={role === Constants.User.Role.Contributor}
							class:text-sky-600={role === Constants.User.Role.GroupLeader}
							class:hover:!text-sky-500={role === Constants.User.Role.GroupLeader}
							class:text-cyan-600={role === Constants.User.Role.GroupOfficer}
							class:hover:!text-cyan-500={role === Constants.User.Role.GroupOfficer}
							href="/user/{chapter.relationships.uploader.id}"
							title={chapter.relationships.uploader.name}
						>
							{chapter.relationships.uploader.name}
						</a>
					</div>
				{/if}
			</div>
		{/if}
	</div>
</article>
