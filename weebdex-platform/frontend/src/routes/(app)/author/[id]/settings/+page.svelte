<script lang="ts">
	import { goto } from "$app/navigation";
	import { API, AuthorLink, Constants, Mutex, Permission, Utils } from "$lib";
	import type { UpdateAuthorRequest } from "$lib/API/Author";
	import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
	import LinkGroup from "$lib/components/LinkGroup.svelte";
	import LinkGroupButton from "$lib/components/LinkGroupButton.svelte";
	import { Button } from "$lib/components/ui/button/index.js";
	import * as InputGroup from "$lib/components/ui/input-group";
	import Label from "$lib/components/ui/label/label.svelte";
	import RichTextarea from "$lib/components/ui/RichTextarea.svelte";
	import Separator from "$lib/components/ui/separator/separator.svelte";
	import { Spinner } from "$lib/components/ui/spinner/index.js";
	import { Switch } from "$lib/components/ui/switch/index.js";
	import * as Tooltip from "$lib/components/ui/tooltip";
	import { getAuthor } from "$lib/Context";
	import { InfoIcon, PlusIcon, TrashIcon } from "@lucide/svelte";
	import equal from "fast-deep-equal";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";

	const { pathname } = window.location;
	const author = getAuthor();
	const mutex = new Mutex();

	const initial = $derived<{
		name: string;
		description: string;
		links: ExternalLink[];

		locked: boolean;
	}>({
		name: $author.name,
		description: $author.description ?? "",
		links: (() => {
			const links: ExternalLink[] = [];
			Object.keys(AuthorLink.names).forEach(key => {
				if (key in $author) {
					links.push({
						code: key,
						link: $author[key as keyof typeof $author] as string,
						valid: true
					});
				}
			});
			return links;
		})(),

		locked: $author.locked ?? false
	});
	let draft = $state<typeof initial>();
	const isChanged = $derived(!equal(initial, draft));
	const isValid = $derived(draft?.name && draft?.links.every(link => link.valid));

	onMount(() => onReset());

	const onSubmit = (ev: Event) => {
		ev.preventDefault();
		ev.stopPropagation();
		ev.stopImmediatePropagation();

		return mutex.exec(async () => {
			if (!$author || !draft || !isChanged || !isValid) {
				return;
			}

			try {
				const payload: UpdateAuthorRequest = { version: $author.version };
				Object.entries(draft).forEach(([key, value]) => {
					if (!equal(value, initial[key as keyof typeof initial])) {
						switch (key) {
							case "links":
								const links = (<ExternalLink[]>value).reduce(
									(acc, { code, link }) => {
										acc[code] = link;
										return acc;
									},
									{} as Record<string, string>
								);
								Object.keys(AuthorLink.names).forEach(key => {
									if (!equal(links[key], $author[key as keyof typeof $author])) {
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

				const res = await API.Author.Update($author.id, payload);
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.OK && res.body) {
					toast.success("Your changes have been saved.");
					$author = res.body;
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
				const res = await API.Author.Delete($author.id);
				if (res?.status === StatusCodes.NO_CONTENT) {
					toast.success("Author deleted successfully");
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
</script>

{#if draft}
	<section class="mt-4">
		<form class="space-y-5" onsubmit={onSubmit} onreset={onReset}>
			<div class="space-y-2">
				<Label for="name">Author Name</Label>
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
				<RichTextarea
					class="min-h-35"
					id="biography"
					bind:value={draft.description}
					parser={Utils.parseDescription}
					disabled={$mutex}
				/>
			</div>
			<Separator />
			<div class="space-y-2">
				<Label>
					Contact Info
					{#if draft.links.length}
						({draft.links.length})
					{/if}
				</Label>
				{#if draft.links.length > 0}
					<div class="space-y-1">
						<LinkGroup bind:value={draft.links} module={AuthorLink} disabled={$mutex} />
					</div>
				{/if}
				<LinkGroupButton bind:links={draft.links} disabled={$mutex}>
					<PlusIcon />
					Add Contact Info
				</LinkGroupButton>
			</div>
			{#if Permission.Author.canUpdateLock()}
				<Separator />
				<div class="flex items-center space-y-2">
					<Label class="w-0 flex-auto" for="permission">Edit Permissions</Label>
					<div class="flex shrink-0 items-center gap-2">
						<Label for="permission">Anyone</Label>
						<Switch id="permission" bind:checked={draft.locked} disabled={$mutex} />
						<Label for="permission">Staff</Label>
					</div>
				</div>
			{/if}
			<Separator />
			<div class="bg-background sticky bottom-0 flex items-center justify-between gap-1 py-1">
				<div>
					{#if Permission.Author.canDelete()}
						<ConfirmDialog
							title="Delete Author"
							description="Are you sure you want to delete this author?"
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
