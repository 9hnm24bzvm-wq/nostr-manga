<script lang="ts">
	import { goto } from "$app/navigation";
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { Constants, GroupLink, Mutex, Utils } from "$lib";
	import API from "$lib/API";
	import type { CreateGroupRequest } from "$lib/API/Group";
	import LinkGroup from "$lib/components/LinkGroup.svelte";
	import LinkGroupButton from "$lib/components/LinkGroupButton.svelte";
	import { Button } from "$lib/components/ui/button";
	import * as InputGroup from "$lib/components/ui/input-group";
	import { Label } from "$lib/components/ui/label";
	import RichTextarea from "$lib/components/ui/RichTextarea.svelte";
	import { Separator } from "$lib/components/ui/separator";
	import { Spinner } from "$lib/components/ui/spinner";
	import * as Tooltip from "$lib/components/ui/tooltip";
	import { InfoIcon, PlusIcon } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { toast } from "svelte-sonner";

	const mutex = new Mutex();
	let draft = $state<{
		name: string;
		description: string;
		links: ExternalLink[];
	}>({
		name: "",
		description: "",
		links: []
	});
	const isValid = $derived(draft?.name && draft?.links.every(link => link.valid));

	const onSubmit = (ev: SubmitEvent) => {
		ev.preventDefault();
		ev.stopPropagation();
		ev.stopImmediatePropagation();

		return mutex.exec(async () => {
			if (!isValid) {
				return;
			}

			try {
				const payload: CreateGroupRequest = <CreateGroupRequest>{};
				Object.entries(draft).forEach(([key, value]) => {
					if (key === "links") {
						(value as ExternalLink[]).forEach(({ code, link }) => {
							payload[code as keyof CreateGroupRequest] = link;
						});
					} else {
						(payload[key as keyof CreateGroupRequest] as any) = value;
					}
				});

				const res = await API.Group.Create(payload);
				if (res?.status === StatusCodes.CREATED && res.body) {
					toast.success("Group created successfully.");
					await goto(`/group/${res.body.id}`);
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.CONFLICT) {
					throw new Constants.Error.Error("Conflict", "Group name already in use.");
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
</script>

<svelte:head>
	<title>Create Group - {PUBLIC_SITE_NAME}</title>
</svelte:head>

<section class="mx-auto w-full max-w-270 flex-auto space-y-5">
	<header>
		<h2 class="text-xl font-medium">Create Group</h2>
	</header>
	<form class="space-y-5" onsubmit={onSubmit}>
		<div class="space-y-2">
			<Label class="block" for="name">Group Name<span class="text-red-500">*</span></Label>
			<InputGroup.Root class="border-none !ring-0">
				<InputGroup.Input
					class="border-none !ring-0 !ring-transparent"
					id="name"
					bind:value={draft.name}
					required
					minlength={1}
					maxlength={128}
					autocomplete="off"
					disabled={$mutex}
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
			<Label>Contact Info</Label>
			{#if draft.links.length > 0}
				<div class="space-y-1">
					<LinkGroup bind:value={draft.links} module={GroupLink} disabled={$mutex} />
				</div>
			{/if}
			<LinkGroupButton bind:links={draft.links} disabled={$mutex}>
				<PlusIcon />
				Add Contact Info
			</LinkGroupButton>
		</div>
		<Separator />
		<div class="bg-background sticky bottom-0 flex items-center justify-end gap-1 py-1">
			<Button class="w-30" type="submit" disabled={$mutex || !isValid}>
				{#if $mutex}
					<Spinner />
				{:else}
					Save
				{/if}
			</Button>
		</div>
	</form>
</section>
