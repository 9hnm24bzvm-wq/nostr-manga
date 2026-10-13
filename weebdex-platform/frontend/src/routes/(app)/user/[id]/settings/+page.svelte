<script lang="ts">
	import { API, Constants, Mutex, UserLink, Utils } from "$lib";
	import type { UpdateUserRequest } from "$lib/API/User";
	import LinkGroup from "$lib/components/LinkGroup.svelte";
	import LinkGroupButton from "$lib/components/LinkGroupButton.svelte";
	import { Button } from "$lib/components/ui/button/index.js";
	import Label from "$lib/components/ui/label/label.svelte";
	import RichTextarea from "$lib/components/ui/RichTextarea.svelte";
	import Separator from "$lib/components/ui/separator/separator.svelte";
	import { Spinner } from "$lib/components/ui/spinner/index.js";
	import { getUser } from "$lib/Context";
	import { PlusIcon } from "@lucide/svelte";
	import equal from "fast-deep-equal";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";

	const { pathname } = window.location;
	const user = getUser();
	const mutex = new Mutex();

	const initial = $derived<{
		description: string;
		links: ExternalLink[];
	}>({
		description: $user.description ?? "",
		links: (() => {
			const links: ExternalLink[] = [];
			Object.keys(UserLink.names).forEach(key => {
				if (key in $user) {
					links.push({
						code: key,
						link: $user[key as keyof typeof $user] as string,
						valid: true
					});
				}
			});
			return links;
		})()
	});
	let draft = $state<typeof initial>();
	const isChanged = $derived(!equal(initial, draft));
	const isValid = $derived(draft?.links.every(link => link.valid));

	onMount(() => onReset());

	const onSubmit = (ev: Event) => {
		ev.preventDefault();
		ev.stopPropagation();
		ev.stopImmediatePropagation();

		return mutex.exec(async () => {
			if (!$user || !draft || !isChanged || !isValid) {
				return;
			}

			try {
				const payload: UpdateUserRequest = { version: $user.version };
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
								Object.keys(UserLink.names).forEach(key => {
									if (!equal(links[key], $user[key as keyof typeof $user])) {
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

				const res = await API.User.Update($user.id, payload);
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.OK && res.body) {
					toast.success("Your changes have been saved.");
					for (const key of Object.keys(payload)) {
						if (!payload[key as keyof typeof payload]) {
							delete $user[key as keyof typeof $user];
						}
					}
					for (const key of Object.keys(res.body)) {
						($user[key as keyof typeof $user] as any) = res.body[key as keyof typeof res.body];
					}
					$user = $user;
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
</script>

{#if draft}
	<section class="mt-4">
		<form class="space-y-5" onsubmit={onSubmit} onreset={onReset}>
			<div class="space-y-2">
				<Label>Username</Label>
				<p class="text-foreground/85">
					Username & email changes are available through the <a
						class="text-foreground hover:text-accent-foreground font-semibold underline"
						href="/settings/account">Account Settings</a
					> page.
				</p>
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
						<LinkGroup bind:value={draft.links} module={UserLink} disabled={$mutex} />
					</div>
				{/if}
				<LinkGroupButton bind:links={draft.links} disabled={$mutex}>
					<PlusIcon />
					Add Contact Info
				</LinkGroupButton>
			</div>
			<Separator />
			<div class="bg-background sticky bottom-0 flex items-center justify-end gap-1 py-1">
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
		</form>
	</section>
{/if}
