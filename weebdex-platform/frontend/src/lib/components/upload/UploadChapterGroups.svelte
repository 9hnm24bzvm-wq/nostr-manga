<script lang="ts">
	import { API, Constants } from "$lib";
	import EntitySearchDialog from "$lib/components/EntitySearchDialog.svelte";
	import { Button } from "$lib/components/ui/button";
	import * as Dialog from "$lib/components/ui/dialog";
	import * as InputGroup from "$lib/components/ui/input-group";
	import { Label } from "$lib/components/ui/label";
	import { Session } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import { ChessRook, CrownIcon, LockIcon, PlusIcon, SearchIcon, TrashIcon } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";

	let {
		user,
		groups = $bindable(),
		enabled,
		disabled
	}: {
		user: User;
		groups: ScanlationGroup[];
		enabled?: boolean;
		disabled?: boolean;
	} = $props();

	const onSearch = async (query: string) => {
		const q = new URLSearchParams();
		q.set("name", query);
		q.set("limit", "100");

		const res = await API.Group.Search(q);
		if (res?.status === StatusCodes.OK && res.body) {
			return res.body;
		} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
			throw Constants.Error.TooManyRequests;
		} else throw Constants.Error.Unknown;
	};
</script>

{#snippet button(group: ScanlationGroup, highlight = true)}
	{@const selectedIdx = groups.findIndex(e => e.id === group.id)}
	<Button
		class={cn("w-0 min-w-[40%] flex-auto border-none", {
			"!text-accent-foreground": selectedIdx >= 0 && highlight,
			"pointer-events-none": disabled
		})}
		type="button"
		variant="outline"
		title={group.name}
		onclick={() => {
			if (disabled) return;
			if (selectedIdx >= 0) groups.splice(selectedIdx, 1);
			else groups.push(group);
		}}
	>
		<span class="w-0 flex-auto truncate text-left">{group.name}</span>
		{#if highlight}
			{#if group.relationships?.members?.some(e => e.id === user.id && e.is_leader)}
				<CrownIcon class="shrink-0" />
			{:else if group.relationships?.members?.some(e => e.id === user.id && e.is_officer)}
				<ChessRook class="shrink-0" />
			{/if}
		{:else if !disabled}
			<TrashIcon class="shrink-0" />
		{/if}
	</Button>
{/snippet}

{#snippet searchButton(group: ScanlationGroup)}
	{@const selectedIdx = groups.findIndex(e => e.id === group.id)}
	<Button
		class={cn("w-0 min-w-[40%] flex-auto  border-none", {
			"!text-accent-foreground": selectedIdx >= 0
		})}
		type="button"
		variant="outline"
		title={group.name}
		onclick={() => {
			if (selectedIdx >= 0) groups.splice(selectedIdx, 1);
			else groups.push(group);
		}}
		disabled={group.locked &&
			!Session.hasRole(Constants.User.Role.Sysop, Constants.User.Role.Staff, Constants.User.Role.Moderator)}
	>
		<span class="w-0 flex-auto truncate text-left">{group.name}</span>
		{#if group.locked && !Session.hasRole(Constants.User.Role.Sysop, Constants.User.Role.Staff, Constants.User.Role.Moderator)}
			<LockIcon class="shrink-0" />
		{:else if selectedIdx >= 0}
			<TrashIcon class="shrink-0" />
		{:else}
			<PlusIcon class="shrink-0" />
		{/if}
	</Button>
{/snippet}

<div class="space-y-2">
	<Label>Groups</Label>
	<div class="ml-2 flex flex-wrap gap-1 border-l pl-2">
		{#if groups.length}
			{#each groups as group}
				{@render button(group, false)}
			{/each}
		{:else}
			<p class="text-muted-foreground">No groups selected</p>
		{/if}
	</div>
	{#if enabled}
		<Dialog.Root>
			<Dialog.Trigger type="button" {disabled}>
				{#snippet child({ props: { class: _, ...props } })}
					<InputGroup.Root
						class={cn("hover:!bg-input/50 cursor-pointer border-none !ring-0", {
							"pointer-events-none": disabled
						})}
						{...props}
					>
						<InputGroup.Text class="px-2">Search...</InputGroup.Text>
						<InputGroup.Addon>
							<SearchIcon />
						</InputGroup.Addon>
					</InputGroup.Root>
				{/snippet}
			</Dialog.Trigger>
			{#if user.relationships?.groups?.length}
				<div class="ml-2 space-y-2 border-l pl-2">
					<Label>Your Groups</Label>
					<div class="flex flex-wrap gap-1">
						{#each user.relationships.groups as group}
							{@render button(group)}
						{/each}
					</div>
				</div>
			{/if}
			<Dialog.Content
				class="flex max-h-[calc(100dvh-2rem)] !max-w-[min(40rem,calc(100dvw-2rem))] flex-col gap-0 p-0"
				showCloseButton={false}
			>
				<EntitySearchDialog {onSearch}>
					{#snippet children({ data }: { data: ScanlationGroup[] })}
						<div class="flex flex-wrap items-center gap-1">
							{#each data as group}
								{@render searchButton(group)}
							{/each}
						</div>
					{/snippet}
				</EntitySearchDialog>
			</Dialog.Content>
		</Dialog.Root>
	{/if}
</div>
