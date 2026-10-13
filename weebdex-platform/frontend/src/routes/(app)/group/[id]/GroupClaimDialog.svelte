<script lang="ts">
	import { Constants, Mutex, Permission } from "$lib";
	import API from "$lib/API";
	import ProtectedCondition from "$lib/components/ui/ProtectedCondition.svelte";
	import ProtectedVerified from "$lib/components/ui/ProtectedVerified.svelte";
	import { Button, buttonVariants } from "$lib/components/ui/button";
	import * as Dialog from "$lib/components/ui/dialog";
	import * as Empty from "$lib/components/ui/empty";
	import * as Field from "$lib/components/ui/field";
	import Input from "$lib/components/ui/input/input.svelte";
	import { Label } from "$lib/components/ui/label";
	import * as RadioGroup from "$lib/components/ui/radio-group";
	import Separator from "$lib/components/ui/separator/separator.svelte";
	import { Spinner } from "$lib/components/ui/spinner";
	import { Textarea } from "$lib/components/ui/textarea";
	import { EntityType } from "$lib/constants";
	import { Session } from "$lib/stores";
	import { Clipboard } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";

	let { open = $bindable(), group }: { open: boolean; group: ScanlationGroup } = $props();
	const mutex = new Mutex();
	const methods = {
		tl: "Translated Chapter (Credits Page)",
		id: "Verification Code"
	};

	let reason = $state<ReportReason>();
	let draft = $state<{
		method: keyof typeof methods;
		url: string;
		notes: string;
	}>({
		method: null!,
		url: "",
		notes: ""
	});

	let isValid = $derived(draft.method in methods && draft.url);
	let isMounted = $state(false);

	onMount(async () => {
		try {
			const res = await API.Report.Reasons(Constants.EntityType.ScanlationGroup);
			if (res?.status === StatusCodes.OK && res.body) {
				reason = res.body.find(e => e.reason === "Group claim request");
			}
		} catch (e) {
			if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
			else {
				console.error(e);
				toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
			}
		}
		isMounted = true;
	});

	const onSubmit = (ev: SubmitEvent) =>
		mutex.exec(async () => {
			if (!reason || !isValid) {
				return;
			}

			ev.preventDefault();
			ev.stopPropagation();
			ev.stopImmediatePropagation();
			try {
				const res = await API.Report.Create({
					entity_type: EntityType.ScanlationGroup,
					entity_id: group.id,
					reason_id: reason.id,
					details: `Method: ${methods[draft.method]}
URL: ${draft.url}
Notes: ${draft.notes || "N/A"}`
				});
				if (res?.status === StatusCodes.NO_CONTENT) {
					open = false;
					toast.success("Group claim request submitted successfully");
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

<Dialog.Header>
	<Dialog.Title>Group Claim Request</Dialog.Title>
</Dialog.Header>
<ProtectedVerified isAction>
	<ProtectedCondition isAllowed={!!$Session && Permission.Report.canCreate()} isAction>
		{#if isMounted}
			<form class="space-y-5" onsubmit={onSubmit}>
				<div class="space-y-1">
					<strong class="block text-base font-semibold">Why is this needed?</strong>
					<div class="typography space-y-2 text-balance">
						<p>
							Groups are not automatically owned by the person who registers them on WeebDex. Otherwise, anyone could
							register a group that isn't theirs and take ownership of it.
						</p>
						<p>
							The group claim process ensures that only the real owner or an authorized representative of a group is
							made its leader.
							<strong> This means we do require some proof of ownership. </strong>
						</p>
						<p class="rounded-md border border-dashed border-red-500/50 bg-red-500/15 p-2 text-red-300">
							<strong>"I created this group" is NOT a valid proof. Read above.</strong>
						</p>
					</div>
				</div>
				<Separator />
				<div class="typography space-y-2 text-balance">
					<p>
						Our verification method requires little to no direct communication. We only need to confirm that you are the
						owner or an authorized representative of the group. You can do this by:
					</p>
					<ul>
						<li>Linking to a publicly accessible translated chapter that the group has worked on, or</li>
						<li>
							Temporarily placing a verification code on the group's website, social media page, or another public
							platform.
						</li>
					</ul>
				</div>
				<Separator />
				<div class="space-y-2">
					<strong class="block text-base font-semibold">Verification Method</strong>
					<RadioGroup.Root class="!gap-1" bind:value={draft.method}>
						{#each Object.entries(methods) as [key, value]}
							<Field.Label class="cursor-pointer">
								<Field.Field class="!p-3" orientation="horizontal">
									<Field.Content>
										<Field.Title>{value}</Field.Title>
									</Field.Content>
									<RadioGroup.Item value={key} />
								</Field.Field>
							</Field.Label>
						{/each}
					</RadioGroup.Root>
				</div>
				{#if draft.method}
					<Separator />
					{#if draft.method === "tl"}
						<div class="space-y-2">
							<Label class="block font-semibold" for="translated-chapter">Translated Chapter</Label>
							<Input
								class="border-none !ring-0 !ring-transparent"
								id="translated-chapter"
								type="url"
								bind:value={draft.url}
								placeholder="Link to a publicly available chapter"
								autocomplete="off"
								required
							/>
						</div>
					{:else if draft.method === "id"}
						<div class="space-y-2">
							<Label class="block font-semibold">Verification Code</Label>
							<div
								class="flex items-center rounded-md border border-dashed border-green-500/50 bg-green-500/15 text-green-300"
							>
								<span class="w-0 flex-auto px-2">
									{$Session!.identity.id}+{group.id}
								</span>
								<Button
									class="shrink-0"
									variant="ghost"
									size="icon-sm"
									onclick={() => {
										navigator.clipboard.writeText(`${$Session!.identity.id}+${group.id}`);
										toast.success("Verification code has been successfully copied to the clipboard", {
											closeButton: false
										});
									}}
								>
									<Clipboard />
								</Button>
							</div>
						</div>
						<div class="space-y-2">
							<Label class="block font-semibold" for="code">Page where the code is placed</Label>
							<Input
								class="border-none !ring-0 !ring-transparent"
								id="code"
								type="url"
								bind:value={draft.url}
								placeholder="Link to MangaDex / Discord / Twitter / Facebook / Personal Website / etc"
								autocomplete="off"
								required
							/>
						</div>
					{/if}
					<div class="space-y-2">
						<Label class="block font-semibold" for="notes">Additional Information</Label>
						<Textarea
							class="max-h-60 min-h-30 border-none !ring-0 !ring-transparent"
							id="notes"
							bind:value={draft.notes}
							placeholder={draft.method === "tl"
								? "e.g.: On the credit page of chapter 27, my Discord server is listed and I'm admin there."
								: "e.g.: I've placed the code on my Discord profile / Twitter profile / MangaDex group page / etc"}
						/>
					</div>
				{/if}
				<Dialog.Footer>
					<Dialog.Close
						class={buttonVariants({ class: "border-none", variant: "outline" })}
						type="button"
						disabled={$mutex}
					>
						Cancel
					</Dialog.Close>
					<Button class="w-30" type="submit" disabled={$mutex || !isValid}>
						{#if $mutex}
							<Spinner />
						{:else}
							Save
						{/if}
					</Button>
				</Dialog.Footer>
			</form>
		{:else}
			<Empty.Root>
				<Spinner class="size-10" />
			</Empty.Root>
		{/if}
	</ProtectedCondition>
</ProtectedVerified>
