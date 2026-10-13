<script lang="ts">
	import { goto } from "$app/navigation";
	import { API, Constants, GroupLink, Mutex, Permission, Utils } from "$lib";
	import type { UpdateGroupRequest } from "$lib/API/Group.js";
	import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
	import EntitySearchDialog from "$lib/components/EntitySearchDialog.svelte";
	import LinkGroup from "$lib/components/LinkGroup.svelte";
	import LinkGroupButton from "$lib/components/LinkGroupButton.svelte";
	import { Button, buttonVariants, type ButtonProps } from "$lib/components/ui/button/index.js";
	import * as Dialog from "$lib/components/ui/dialog";
	import * as InputGroup from "$lib/components/ui/input-group";
	import Label from "$lib/components/ui/label/label.svelte";
	import RichTextarea from "$lib/components/ui/RichTextarea.svelte";
	import Separator from "$lib/components/ui/separator/separator.svelte";
	import { Spinner } from "$lib/components/ui/spinner/index.js";
	import { Switch } from "$lib/components/ui/switch/index.js";
	import * as Tooltip from "$lib/components/ui/tooltip";
	import { getGroup } from "$lib/Context";
	import { cn } from "$lib/Utils";
	import { ChevronUp, InfoIcon, Plus, PlusIcon, Trash, TrashIcon } from "@lucide/svelte";
	import equal from "fast-deep-equal";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";

	const { pathname } = window.location;
	const group = getGroup();
	const mutex = new Mutex();

	const initial = $derived<{
		name: string;
		description: string;
		links: ExternalLink[];

		locked: boolean;
		inactive: boolean;

		leader?: User;
		officers: User[];
		members: User[];
	}>({
		name: $group.name,
		description: $group.description ?? "",
		links: (() => {
			const links: ExternalLink[] = [];
			Object.keys(GroupLink.names).forEach(key => {
				if (key in $group) {
					links.push({
						code: key,
						link: $group[key as keyof typeof $group] as string,
						valid: true
					});
				}
			});
			return links;
		})(),

		locked: $group.locked ?? false,
		inactive: $group.inactive ?? false,

		leader: $group.relationships?.members?.find(member => member.is_leader) ?? null!,
		officers: $group.relationships?.members?.filter(member => member.is_officer) ?? [],
		members: $group.relationships?.members?.filter(member => !member.is_officer && !member.is_leader) ?? []
	});
	let draft = $state<typeof initial>();
	const isChanged = $derived(!equal(initial, draft));
	const isValid = $derived(draft?.name && draft?.links.every(link => link.valid));

	let isShowContactInfo = $state(true);
	let isShowMembers = $state(true);

	onMount(() => onReset());

	const onSubmit = (ev: Event) => {
		ev.preventDefault();
		ev.stopPropagation();
		ev.stopImmediatePropagation();

		return mutex.exec(async () => {
			if (!$group || !draft || !isChanged || !isValid) {
				return;
			}

			try {
				const payload: UpdateGroupRequest = { version: $group.version };
				Object.entries(draft).forEach(([key, value]) => {
					if (!equal(value, initial[key as keyof typeof initial])) {
						switch (key) {
							case "name":
								if (Permission.Group.canUpdateName()) {
									payload.name = (<string>value).trim();
								}
								break;
							case "leader":
								payload.leader = (<User>value)?.id ?? "";
								break;
							case "officers":
								payload.officers = (<User[]>value).map(a => a.id);
								break;
							case "members":
								payload.members = (<User[]>value).map(a => a.id);
								break;
							case "links":
								const links = (<ExternalLink[]>value).reduce(
									(acc, { code, link }) => {
										acc[code] = link;
										return acc;
									},
									{} as Record<string, string>
								);
								Object.keys(GroupLink.names).forEach(key => {
									if (!equal(links[key], $group[key as keyof typeof $group])) {
										(payload[key as keyof typeof payload] as any) = links[key] ?? "";
									}
								});
								break;
							default:
								(payload[key as keyof typeof payload] as any) = value;
								break;
						}
					}
				});
				if (Object.keys(payload).length <= 1) {
					return;
				}

				if (typeof payload.leader === "string" || payload.officers || payload.members) {
					payload.leader = payload.leader ?? $group.relationships?.members?.find(e => e.is_leader)?.id ?? "";
					payload.officers =
						payload.officers ?? $group.relationships?.members?.filter(e => e.is_officer)?.map(a => a.id) ?? [];
					payload.members =
						payload.members ??
						$group.relationships?.members?.filter(e => !e.is_leader && !e.is_officer).map(a => a.id) ??
						[];
				}

				const res = await API.Group.Update($group.id, payload);
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.OK && res.body) {
					toast.success("Your changes have been saved.");
					$group = res.body;
					onReset();
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.CONFLICT) {
					throw Constants.Error.UpdateConflict;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			}
		});
	};

	const onReset = (ev?: Event & { currentTarget: HTMLFormElement }) => {
		ev?.preventDefault();
		ev?.stopPropagation();
		ev?.stopImmediatePropagation();
		draft = structuredClone($state.snapshot(initial));
	};

	const onDelete = () =>
		mutex.exec(async () => {
			try {
				const res = await API.Group.Delete($group.id);
				if (res?.status === StatusCodes.NO_CONTENT) {
					toast.success("Group deleted successfully");
					goto("/");
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			}
		});

	const onSearch = async (query: string) => {
		const q = new URLSearchParams();
		q.set("name", query);

		const res = await API.User.Search(q);
		if (res?.status === StatusCodes.OK && res.body) {
			return res.body;
		} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
			throw Constants.Error.TooManyRequests;
		} else throw Constants.Error.Unknown;
	};
</script>

{#snippet userButton({ selected, user, ...restProps }: ButtonProps & { user: User; selected?: boolean })}
	<Button
		class="w-0 min-w-[40%] flex-auto justify-start border-none"
		type="button"
		variant="outline"
		title={user.name}
		disabled={$mutex}
		{...restProps}
	>
		<span class="w-0 flex-auto truncate text-left">
			{user.name}
		</span>
		{#if selected}
			<Trash />
		{:else}
			<Plus />
		{/if}
	</Button>
{/snippet}

{#if draft}
	<section class="mt-4">
		<form class="space-y-5" onsubmit={onSubmit} onreset={onReset}>
			<div class="space-y-2">
				<Label for="name">Group Name</Label>
				<div class="space-y-1">
					<InputGroup.Root class="border-none !ring-0">
						<InputGroup.Input
							class="border-none !ring-0 !ring-transparent"
							id="name"
							bind:value={draft.name}
							required
							minlength={1}
							maxlength={128}
							autocomplete="off"
							disabled={$mutex || !Permission.Group.canUpdateName()}
						/>
						<InputGroup.Addon align="inline-end">
							<Tooltip.Root>
								<Tooltip.Trigger>
									{#snippet child({ props })}
										<InputGroup.Button {...props} class="rounded-full" size="icon-xs">
											<InfoIcon />
										</InputGroup.Button>
									{/snippet}
								</Tooltip.Trigger>
								<Tooltip.Content>Must be between 1 and 128 characters.</Tooltip.Content>
							</Tooltip.Root>
						</InputGroup.Addon>
					</InputGroup.Root>
					<p class="text-foreground/85">
						Name changes can only be made by staff. Please report the group to request a name change.
					</p>
				</div>
			</div>
			<div class="space-y-2">
				<Label for="biography">Biography</Label>
				<div class="space-y-1">
					<RichTextarea
						class="min-h-35"
						id="biography"
						bind:value={draft.description}
						parser={Utils.parseDescription}
						disabled={$mutex}
					/>
				</div>
			</div>
			<Separator />
			<div class="space-y-2">
				<div class="relative flex h-5 items-center gap-2">
					<Label>
						Contact Info
						{#if draft.links.length}
							({draft.links.length})
						{/if}
					</Label>
					<Button variant="ghost" size="icon-sm" onclick={() => (isShowContactInfo = !isShowContactInfo)}>
						<ChevronUp class={cn("size-5 transition-all", !isShowContactInfo && "-rotate-180")} />
					</Button>
				</div>
				{#if isShowContactInfo}
					{#if draft.links.length > 0}
						<div class="space-y-1">
							<LinkGroup bind:value={draft.links} module={GroupLink} disabled={$mutex} />
						</div>
					{/if}
					<LinkGroupButton bind:links={draft.links} disabled={$mutex}>
						<PlusIcon />
						Add Contact Info
					</LinkGroupButton>
				{/if}
			</div>
			<div class="space-y-2">
				<div class="relative flex h-5 items-center gap-2">
					<Label>
						Group Members
						{#if draft.leader || draft.officers.length || draft.members.length}
							({(draft.leader ? 1 : 0) + draft.officers.length + draft.members.length})
						{/if}
					</Label>
					<Button variant="ghost" size="icon-sm" onclick={() => (isShowMembers = !isShowMembers)}>
						<ChevronUp class={cn("size-5 transition-all", !isShowMembers && "-rotate-180")} />
					</Button>
				</div>
				{#if isShowMembers}
					<div class="ml-2 space-y-2 border-l pl-2">
						<div class="space-y-2">
							<Label>Leader</Label>
							<div class="flex flex-wrap items-center gap-1">
								{#if draft.leader}
									{@render userButton({
										user: draft.leader,
										selected: true,
										onclick: () => {
											draft!.leader = null!;
										}
									})}
								{:else}
									<Dialog.Root>
										<Dialog.Trigger
											class={buttonVariants({
												class: "w-0 min-w-[40%] flex-auto border-none",
												variant: "outline"
											})}
											type="button"
											disabled={$mutex}
										>
											<PlusIcon />
										</Dialog.Trigger>
										<Dialog.Content
											class="flex max-h-[calc(100dvh-2rem)] !max-w-[min(40rem,calc(100dvw-2rem))] flex-col gap-0 p-0"
											showCloseButton={false}
										>
											<EntitySearchDialog {onSearch}>
												{#snippet children({ data }: { data: User[] })}
													<div class="flex flex-wrap items-center gap-1">
														{#each data as user}
															{@render userButton({
																user,
																selected: draft?.leader?.id === user.id,
																disabled:
																	draft!.members.some(e => e.id === user.id) ||
																	draft!.officers.some(e => e.id === user.id),
																onclick: () => {
																	if (draft!.leader?.id === user.id) {
																		draft!.leader = null!;
																	} else draft!.leader = user;
																}
															})}
														{/each}
													</div>
												{/snippet}
											</EntitySearchDialog>
										</Dialog.Content>
									</Dialog.Root>
								{/if}
							</div>
						</div>
						<Separator />
						<div class="space-y-2">
							<div class="space-y-1">
								<Label>
									Officers
									{#if draft.officers.length}
										({draft.officers.length})
									{/if}
								</Label>
								<p class="text-foreground/85">
									Officers have the right to edit any chapters associated with this group.
								</p>
							</div>
							<div class="flex flex-wrap items-center gap-1">
								{#each draft.officers as officer}
									{@render userButton({
										user: officer,
										selected: true,
										onclick: () => {
											const idx = draft!.officers.indexOf(officer);
											if (idx > -1) draft!.officers.splice(idx, 1);
										}
									})}
								{/each}
								{#if draft.officers.length < 3}
									<Dialog.Root>
										<Dialog.Trigger
											class={buttonVariants({
												class: "w-0 min-w-[40%] flex-auto border-none",
												variant: "outline"
											})}
											type="button"
											disabled={$mutex}
										>
											<PlusIcon />
										</Dialog.Trigger>
										<Dialog.Content
											class="flex max-h-[calc(100dvh-2rem)] !max-w-[min(40rem,calc(100dvw-2rem))] flex-col gap-0 p-0"
											showCloseButton={false}
										>
											<EntitySearchDialog {onSearch}>
												{#snippet children({ data }: { data: User[] })}
													<div class="flex flex-wrap items-center gap-1">
														{#each data as user}
															{@const idx = draft!.officers.findIndex(e => e.id === user.id)}
															{@render userButton({
																user,
																selected: idx >= 0,
																disabled: draft!.leader?.id === user.id || draft!.members.some(e => e.id === user.id),
																onclick: () => {
																	if (idx >= 0) draft!.officers.splice(idx, 1);
																	else draft!.officers.push(user);
																}
															})}
														{/each}
													</div>
												{/snippet}
											</EntitySearchDialog>
										</Dialog.Content>
									</Dialog.Root>
								{/if}
							</div>
						</div>
						<Separator />
						<div class="space-y-2">
							<Label>
								Members
								{#if draft.members.length}
									({draft.members.length})
								{/if}
							</Label>
							<div class="flex flex-wrap items-center gap-1">
								{#if draft.members.length}
									{#each draft.members as officer}
										{@render userButton({
											user: officer,
											selected: true,
											onclick: () => {
												const idx = draft!.members.indexOf(officer);
												if (idx > -1) draft!.members.splice(idx, 1);
											}
										})}
									{/each}
								{/if}
								{#if draft.members.length < 50}
									<Dialog.Root>
										<Dialog.Trigger
											class={buttonVariants({
												class: "w-0 min-w-[40%] flex-auto border-none",
												variant: "outline"
											})}
											type="button"
											disabled={$mutex}
										>
											<PlusIcon />
										</Dialog.Trigger>
										<Dialog.Content
											class="flex max-h-[calc(100dvh-2rem)] !max-w-[min(40rem,calc(100dvw-2rem))] flex-col gap-0 p-0"
											showCloseButton={false}
										>
											<EntitySearchDialog {onSearch}>
												{#snippet children({ data }: { data: User[] })}
													<div class="flex flex-wrap items-center gap-1">
														{#each data as user}
															{@const idx = draft!.members.findIndex(e => e.id === user.id)}
															{@render userButton({
																user,
																selected: idx >= 0,
																disabled: draft!.leader?.id === user.id || draft!.officers.some(e => e.id === user.id),
																onclick: () => {
																	if (idx >= 0) draft!.members.splice(idx, 1);
																	else draft!.members.push(user);
																}
															})}
														{/each}
													</div>
												{/snippet}
											</EntitySearchDialog>
										</Dialog.Content>
									</Dialog.Root>
								{/if}
							</div>
						</div>
					</div>
				{/if}
			</div>
			<Separator />
			<div class="space-y-2">
				<div class="flex items-center space-y-2">
					<Label class="w-0 flex-auto" for="permission">Upload Permissions</Label>
					<div class="flex shrink-0 items-center gap-2">
						<Label for="permission">Anyone</Label>
						<Switch id="permission" bind:checked={draft.locked} disabled={$mutex} />
						<Label for="permission">Members Only</Label>
					</div>
				</div>
				<div class="flex items-center space-y-2">
					<Label class="w-0 flex-auto" for="activity">Group Status</Label>
					<div class="flex shrink-0 items-center gap-2">
						<Label for="activity">Active</Label>
						<Switch id="activity" bind:checked={draft.inactive} disabled={$mutex} />
						<Label for="activity">Inactive</Label>
					</div>
				</div>
			</div>
			<Separator />
			<div class="bg-background sticky bottom-0 flex items-center justify-between gap-1 py-1">
				<div>
					{#if Permission.Group.canDelete()}
						<ConfirmDialog
							title="Delete Group"
							description="Are you sure you want to delete this group?"
							confirmText="Delete"
							variant="destructive"
							onConfirm={e => e.exec(onDelete)}
							disabled={$mutex}
						>
							{#snippet child({ props })}
								<Button class="w-24" variant="destructive" {...props}>
									<TrashIcon />
									Delete
								</Button>
							{/snippet}
						</ConfirmDialog>
					{/if}
				</div>
				<div class="flex items-center gap-1">
					<Button
						class="!text-destructive w-24 border-none"
						type="reset"
						variant="outline"
						disabled={$mutex || !isChanged}
					>
						Revert
					</Button>
					<Button class="w-30" type="submit" disabled={$mutex || !isChanged || !isValid}>
						{#if $mutex}
							<Spinner />
						{:else}
							Save
						{/if}
					</Button>
				</div>
			</div>
		</form>
	</section>
{/if}
