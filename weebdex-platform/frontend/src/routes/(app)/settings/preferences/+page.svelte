<script lang="ts">
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { API, Constants, Locale, Mutex } from "$lib";
	import type { UpdateSettingsRequest } from "$lib/API/Settings";
	import EntitySearchDialog from "$lib/components/EntitySearchDialog.svelte";
	import TagComboBox from "$lib/components/TagComboBox.svelte";
	import { Button, buttonVariants } from "$lib/components/ui/button";
	import { Checkbox } from "$lib/components/ui/checkbox";
	import * as Combobox from "$lib/components/ui/combobox";
	import * as Dialog from "$lib/components/ui/dialog";
	import * as Empty from "$lib/components/ui/empty";
	import ImageLoader from "$lib/components/ui/ImageLoader.svelte";
	import { Label } from "$lib/components/ui/label";
	import * as Select from "$lib/components/ui/select";
	import { Separator } from "$lib/components/ui/separator";
	import { Spinner } from "$lib/components/ui/spinner";
	import { populateTagList, prefs, Session } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import { Plus, Trash } from "@lucide/svelte";
	import equal from "fast-deep-equal";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";

	const mutex = new Mutex();
	let draft = $state<UserPreferences>();
	const isChanged = $derived(!equal($prefs, draft));

	onMount(async () => {
		try {
			await Promise.all([Session.isValid(), populateTagList()]);
		} catch (e) {
			console.error(e);
		}
		onReset();
	});

	const onReset = () => {
		if ($prefs) {
			draft = structuredClone($prefs);
		}
	};

	const onSubmit = (ev: SubmitEvent) => {
		ev.preventDefault();
		ev.stopPropagation();
		ev.stopImmediatePropagation();
		return mutex.exec(async () => {
			if (!$prefs || !draft || !isChanged) {
				return;
			}

			try {
				if ($Session) {
					const payload: UpdateSettingsRequest = { version: $prefs.version };
					Object.entries(draft).forEach(([key, value]) => {
						if (!equal(value, $prefs![key as keyof typeof $prefs])) {
							if (["uploaders", "groups"].includes(key)) {
								(payload[key as keyof UpdateSettingsRequest] as string[]) =
									(value as { id: string }[])?.map(({ id }) => id) ?? [];
							} else if (Array.isArray(value)) {
								(payload[key as keyof UpdateSettingsRequest] as any[]) = Array.from(value as any[]);
							} else {
								(payload[key as keyof UpdateSettingsRequest] as any) = value;
							}
						}
					});
					if (Object.keys(payload).length <= 1) {
						return;
					}

					const res = await API.Settings.Update(payload);
					if (res?.status === StatusCodes.OK && res.body) {
						toast.success("Your changes have been saved.");
						$prefs = $Session.preferences = res.body;
						onReset();
					} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
						throw Constants.Error.TooManyRequests;
					} else throw Constants.Error.Unknown;
				} else {
					Object.entries(draft).forEach(([key, value]) => {
						if (!equal(value, $prefs![key as keyof typeof $prefs])) {
							if (Array.isArray(value)) {
								($prefs![key as keyof typeof $prefs] as any[]) = Array.from(value as any[]);
							} else {
								($prefs![key as keyof typeof $prefs] as any) = value;
							}
						}
					});

					toast.success("Your changes have been saved.");
					$prefs = $prefs;
					onReset();
				}
			} catch (e) {
				if (e instanceof Constants.Error.Error) {
					toast.error(e.title, { description: e.body });
				} else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			}
		});
	};
</script>

<svelte:head>
	<title>Settings: Preferences - {PUBLIC_SITE_NAME}</title>
</svelte:head>

{#snippet userButton(user: User)}
	{@const selectedIdx = draft!.uploaders.findIndex(u => u.id === user.id)}
	<Button
		class={cn("w-0 min-w-[40%] flex-auto justify-start border-none", {
			"!text-destructive line-through": selectedIdx >= 0
		})}
		type="button"
		variant="outline"
		title={user.name}
		disabled={selectedIdx < 0 && draft!.uploaders.length >= 25}
		onclick={() => {
			if (selectedIdx >= 0) draft!.uploaders.splice(selectedIdx, 1);
			else draft!.uploaders.push(user);
		}}
	>
		<span class="flex-auto truncate text-left">
			{user.name}
		</span>
		{#if selectedIdx >= 0}
			<Trash />
		{:else}
			<Plus />
		{/if}
	</Button>
{/snippet}

{#snippet groupButton(group: ScanlationGroup)}
	{@const selectedIdx = draft!.groups.findIndex(u => u.id === group.id)}
	<Button
		class={cn("w-0 min-w-[40%] flex-auto justify-start border-none", {
			"!text-destructive line-through": selectedIdx >= 0
		})}
		type="button"
		variant="outline"
		title={group.name}
		disabled={selectedIdx < 0 && draft!.groups.length >= 25}
		onclick={() => {
			if (selectedIdx >= 0) draft!.groups.splice(selectedIdx, 1);
			else draft!.groups.push(group);
		}}
	>
		<span class="flex-auto truncate text-left">
			{group.name}
		</span>
		{#if selectedIdx >= 0}
			<Trash />
		{:else}
			<Plus />
		{/if}
	</Button>
{/snippet}

{#if draft}
	<section>
		<form class="space-y-5" onsubmit={onSubmit} onreset={onReset}>
			<div class="flex flex-col gap-2 md:flex-row md:gap-8">
				<div class="flex-auto space-y-1">
					<strong class="block font-semibold">Original Language Filter</strong>
					<p class="text-foreground/85 max-w-md">
						Only show titles originally published in these languages. If no languages are selected, no filter will be
						applied.
					</p>
				</div>
				<div class="shrink-0">
					<Combobox.Root type="multiple" bind:value={draft.languages} disabled={$mutex}>
						<Combobox.Trigger class="flex min-w-50 border-none">
							{#if draft.languages?.length}
								<span class="flex w-0 flex-auto flex-nowrap items-center gap-1 overflow-hidden text-left">
									{#if draft.languages.length > 1}
										{#each draft.languages as code}
											<ImageLoader
												class="size-5 shrink-0"
												src="/images/flags/{Locale.to3166(code)}.svg"
												alt={Locale.toLanguage(code)}
											/>
										{/each}
									{:else}
										<ImageLoader
											class="size-5 shrink-0"
											src="/images/flags/{Locale.to3166(draft.languages[0])}.svg"
											alt={Locale.toLanguage(draft.languages[0])}
										/>
										{Locale.toLanguage(draft.languages[0])}
									{/if}
								</span>
								<strong class="shrink-0 font-medium text-green-600">+{draft.languages.length}</strong>
							{:else}
								Original Languages
							{/if}
						</Combobox.Trigger>
						<Combobox.Content>
							{#each Object.entries(Locale.Languages) as [key, value]}
								<Combobox.Item value={key}>
									<span class="size-5 shrink-0">
										<ImageLoader class="size-5" src="/images/flags/{Locale.to3166(key)}.svg" alt={value} />
									</span>
									{value}
								</Combobox.Item>
							{/each}
						</Combobox.Content>
					</Combobox.Root>
				</div>
			</div>
			<Separator />
			<div class="flex flex-col gap-2 md:flex-row md:gap-8">
				<div class="flex-auto space-y-1">
					<strong class="block font-semibold">Chapter Language Filter</strong>
					<p class="text-foreground/85 max-w-md">
						The default language the filter for chapter list is set to. Changing the filter in chapter list will not
						modify this value.
					</p>
				</div>
				<div class="shrink-0">
					<Combobox.Root type="multiple" bind:value={draft.translated_languages} disabled={$mutex}>
						<Combobox.Trigger class="flex min-w-50 border-none">
							{#if draft.translated_languages?.length}
								<span class="flex w-0 flex-auto flex-nowrap items-center gap-1 overflow-hidden text-left">
									{#if draft.translated_languages.length > 1}
										{#each draft.translated_languages as code}
											<ImageLoader
												class="size-5 shrink-0"
												src="/images/flags/{Locale.to3166(code)}.svg"
												alt={Locale.toLanguage(code)}
											/>
										{/each}
									{:else}
										<ImageLoader
											class="size-5 shrink-0"
											src="/images/flags/{Locale.to3166(draft.translated_languages[0])}.svg"
											alt={Locale.toLanguage(draft.translated_languages[0])}
										/>
										{Locale.toLanguage(draft.translated_languages[0])}
									{/if}
								</span>
								<strong class="shrink-0 font-medium text-green-600">+{draft.translated_languages.length}</strong>
							{:else}
								Translated Languages
							{/if}
						</Combobox.Trigger>
						<Combobox.Content>
							{#each Object.entries(Locale.Languages) as [key, value]}
								<Combobox.Item value={key}>
									<span class="size-5 shrink-0">
										<ImageLoader class="size-5" src="/images/flags/{Locale.to3166(key)}.svg" alt={value} />
									</span>
									{value}
								</Combobox.Item>
							{/each}
						</Combobox.Content>
					</Combobox.Root>
				</div>
			</div>
			<Separator />
			<div class="flex flex-col gap-2 md:flex-row md:gap-8">
				<div class="flex-auto space-y-1">
					<strong class="block font-semibold">Items Per Page</strong>
					<p class="text-foreground/85 max-w-md">
						How many items to load per page. A lower value may reduce the time taken on slower networks and devices.
					</p>
				</div>
				<div class="shrink-0">
					<Select.Root type="single" bind:value={draft.items_per_page} disabled={$mutex}>
						<Select.Trigger class="text-muted-foreground flex w-0 min-w-50 flex-auto border-none">
							Items Per Page: {draft.items_per_page}
						</Select.Trigger>
						<Select.Content>
							{#each [28, 42, 56, 70, 84, 98] as value}
								<Select.Item value={value.toString()}>
									{value}
								</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
			</div>
			{#if $Session}
				<Separator />
				<div class="flex flex-col gap-2 md:flex-row md:gap-8">
					<div class="flex-auto space-y-1">
						<strong class="block font-semibold">Content Rating</strong>
						<p class="text-foreground/85 max-w-md">Choose how this site displays explicit material.</p>
					</div>
					<div class="flex shrink-0 flex-wrap gap-2 md:flex-col md:gap-1">
						<div class="flex items-center">
							<Checkbox id="safe" bind:checked={draft.show_safe} disabled={$mutex} />
							<Label class="cursor-pointer pl-1" for="safe">Safe</Label>
						</div>
						<div class="flex items-center">
							<Checkbox id="suggestive" bind:checked={draft.show_suggestive} disabled={$mutex} />
							<Label class="cursor-pointer pl-1" for="suggestive">Suggestive</Label>
						</div>
						<div class="flex items-center">
							<Checkbox id="erotic" bind:checked={draft.show_erotic} disabled={$mutex} />
							<Label class="cursor-pointer pl-1" for="erotic">Erotica</Label>
						</div>
						<div class="flex items-center">
							<Checkbox id="hentai" bind:checked={draft.show_hentai} disabled={$mutex} />
							<Label class="cursor-pointer pl-1" for="hentai">Pornography</Label>
						</div>
					</div>
				</div>
			{/if}
			{#if $Session}
				{#if Session.hasRole(Constants.User.Role.Verified)}
					<Separator />
					<div class="space-y-3">
						<div class="flex flex-col gap-2 md:flex-row md:gap-8">
							<div class="flex-auto space-y-1">
								<strong class="block font-semibold">Uploader Block</strong>
								<p class="text-foreground/85 max-w-md">
									Blacklist for any users that you don't wish to see chapters from. Limit is 25 users.
								</p>
							</div>
							<div class="shrink-0">
								<Dialog.Root>
									<Dialog.Trigger
										class={buttonVariants({
											class: "border-none",
											variant: "outline"
										})}
										type="button"
										disabled={$mutex}
									>
										Edit Blacklist
										{#if draft.uploaders.length}
											: {draft.uploaders.length}
										{/if}
									</Dialog.Trigger>
									<Dialog.Content
										class="flex max-h-[calc(100dvh-2rem)] !max-w-[min(40rem,calc(100dvw-2rem))] flex-col gap-0 p-0"
										showCloseButton={false}
									>
										<EntitySearchDialog
											onSearch={async query => {
												const q = new URLSearchParams();
												q.set("name", query);
												q.set("limit", "100");

												const res = await API.User.Search(q);
												if (res?.status === StatusCodes.OK && res.body) {
													return res.body;
												} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
													throw Constants.Error.TooManyRequests;
												} else throw Constants.Error.Unknown;
											}}
										>
											{#snippet children({ data }: { data: User[] })}
												<div class="flex flex-wrap items-center gap-1">
													{#each data as user}
														{@render userButton(user)}
													{/each}
												</div>
											{/snippet}
										</EntitySearchDialog>
									</Dialog.Content>
								</Dialog.Root>
							</div>
						</div>
						{#if draft.uploaders.length}
							<div class="flex flex-wrap items-center gap-1">
								{#each draft.uploaders as user}
									{@render userButton(user)}
								{/each}
							</div>
						{/if}
					</div>
				{/if}
				<Separator />
				<div class="space-y-3">
					<div class="flex flex-col gap-2 md:flex-row md:gap-8">
						<div class="flex-auto space-y-1">
							<strong class="block font-semibold">Group Block</strong>
							<p class="text-foreground/85 max-w-md">
								Blacklist for any groups that you don't wish to see chapters from. Limit is 25 groups.
							</p>
						</div>
						<div class="shrink-0">
							<Dialog.Root>
								<Dialog.Trigger
									class={buttonVariants({
										class: "border-none",
										variant: "outline"
									})}
									type="button"
									disabled={$mutex}
								>
									Edit Blacklist
									{#if draft.groups.length}
										: {draft.groups.length}
									{/if}
								</Dialog.Trigger>
								<Dialog.Content
									class="flex max-h-[calc(100dvh-2rem)] !max-w-[min(40rem,calc(100dvw-2rem))] flex-col gap-0 p-0"
									showCloseButton={false}
								>
									<EntitySearchDialog
										onSearch={async query => {
											const q = new URLSearchParams();
											q.set("name", query);
											q.set("limit", "100");

											const res = await API.Group.Search(q);
											if (res?.status === StatusCodes.OK && res.body) {
												return res.body;
											} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
												throw Constants.Error.TooManyRequests;
											} else throw Constants.Error.Unknown;
										}}
									>
										{#snippet children({ data }: { data: ScanlationGroup[] })}
											<div class="flex flex-wrap items-center gap-1">
												{#each data as group}
													{@render groupButton(group)}
												{/each}
											</div>
										{/snippet}
									</EntitySearchDialog>
								</Dialog.Content>
							</Dialog.Root>
						</div>
					</div>
					{#if draft.groups.length}
						<div class="flex flex-wrap items-center gap-1">
							{#each draft.groups as group}
								{@render groupButton(group)}
							{/each}
						</div>
					{/if}
				</div>
			{/if}
			<Separator />
			<div class="space-y-4">
				<div class="space-y-1">
					<strong class="block font-semibold">Tag Filter</strong>
					<p class="text-foreground/85 max-w-md">
						Prevent showing titles that contain any of the selected tags. Titles that you follow with these tags will
						still appear in your Library and Updates.
					</p>
				</div>
				<TagComboBox bind:value={draft.tags} variant="destructive" disabled={$mutex} />
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
				<Button class="w-30" type="submit" disabled={$mutex || !isChanged}>
					{#if $mutex}
						<Spinner />
					{:else}
						Save
					{/if}
				</Button>
			</div>
		</form>
	</section>
{:else}
	<Empty.Root>
		<Spinner class="size-10" />
	</Empty.Root>
{/if}
