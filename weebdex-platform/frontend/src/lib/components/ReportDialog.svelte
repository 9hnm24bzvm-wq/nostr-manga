<script lang="ts" module>
	let isOpen = $state<boolean>(false);
	let entity = $state<User | Manga | Author | Chapter | ScanlationGroup | Thread | Reply>();
	let entityType = $state<Constants.EntityType>();

	export const set = (e: typeof entity, type: typeof entityType) => {
		entity = e;
		entityType = type;
	};

	export const open = () => (isOpen = true);
	export const close = () => (isOpen = false);
	export const getIsOpen = () => isOpen;
</script>

<script lang="ts">
	import { page } from "$app/state";
	import { API, Constants, Mutex, Permission } from "$lib";
	import * as Combobox from "$lib/components/ui/combobox";
	import ProtectedCondition from "$lib/components/ui/ProtectedCondition.svelte";
	import { Textarea } from "$lib/components/ui/textarea/index.js";
	import { EntityType } from "$lib/constants";
	import { Session } from "$lib/stores";
	import { StatusCodes } from "http-status-codes";
	import { toast } from "svelte-sonner";
	import ReportAuthorCard from "./ReportAuthorCard.svelte";
	import ReportChapterCard from "./ReportChapterCard.svelte";
	import ReportGroupCard from "./ReportGroupCard.svelte";
	import ReportMangaCard from "./ReportMangaCard.svelte";
	import ReportReplyCard from "./ReportReplyCard.svelte";
	import ReportUserCard from "./ReportUserCard.svelte";
	import { Button, buttonVariants } from "./ui/button";
	import * as Dialog from "./ui/dialog";
	import Protected from "./ui/Protected.svelte";
	import { Separator } from "./ui/separator";
	import { Spinner } from "./ui/spinner";

	const mutex = new Mutex();
	let reasons = $state<ReportReason[]>();
	let selectedReasonId = $state<string>();
	let additionalExplanation = $state<string>();

	const currentReason = $derived(reasons?.find(reason => reason.id === selectedReasonId));
	const isValid = $derived(selectedReasonId && (!currentReason?.details_required || !!additionalExplanation));

	$effect(() => {
		if (page.url) {
			isOpen = false;
		}
	});

	$effect(() => {
		if (!isOpen) {
			window.setTimeout(() => {
				reasons = undefined;
				selectedReasonId = undefined;
				additionalExplanation = undefined;
				entity = undefined;
				entityType = undefined;
			}, 200);
			return;
		}

		if (!reasons?.length && entityType) {
			(async () => {
				const { href } = window.location;

				try {
					const res = await API.Report.Reasons(entityType);
					if (window.location.href !== href) {
						return;
					}

					if (res?.status === StatusCodes.OK && res.body) {
						reasons = res.body;
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
			})();
		}
	});

	const onSubmit = (e?: SubmitEvent) => {
		e?.preventDefault();
		e?.stopPropagation();
		e?.stopImmediatePropagation();

		return mutex.exec(async () => {
			if (!entity || !entityType || !reasons?.length || !selectedReasonId || !isValid) {
				return;
			}

			try {
				const res = await API.Report.Create({
					entity_type: entityType,
					entity_id: typeof entity.id === "string" ? entity.id : entity.id.toString(),
					reason_id: selectedReasonId,
					details: additionalExplanation
				});
				if (res?.status === StatusCodes.NO_CONTENT) {
					isOpen = false;
					toast.success("Report submitted successfully");
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
	};
</script>

<Dialog.Root bind:open={isOpen}>
	<Dialog.Content class="flex max-h-[calc(100dvh-2rem)] !max-w-[min(40rem,calc(100dvw-2rem))] flex-col overflow-auto">
		<Dialog.Header>
			<Dialog.Title>
				Report
				{#if entityType === EntityType.User}
					User
				{:else if entityType === EntityType.Manga}
					Manga
				{:else if entityType === EntityType.Author}
					Author
				{:else if entityType === EntityType.Chapter}
					Chapter
				{:else if entityType === EntityType.ScanlationGroup}
					Group
				{/if}
			</Dialog.Title>
		</Dialog.Header>
		<Protected isAction>
			<ProtectedCondition isAllowed={!!$Session && Permission.Report.canCreate()} isAction>
				<form class="space-y-5" onsubmit={onSubmit}>
					<div class="space-y-2">
						{#if entity}
							<div class="space-y-1">
								<strong class="block font-semibold">Subject</strong>
								{#if entityType === EntityType.User}
									<ReportUserCard user={entity as User} />
								{:else if entityType === EntityType.Manga}
									<ReportMangaCard manga={entity as Manga} />
								{:else if entityType === EntityType.Author}
									<ReportAuthorCard author={entity as Author} />
								{:else if entityType === EntityType.Chapter}
									{#if (entity as Chapter).relationships.manga}
										<ReportMangaCard manga={(entity as Chapter).relationships.manga} />
									{/if}
									<div
										class:ml-1={!!(entity as Chapter).relationships.manga}
										class:border-l-1={!!(entity as Chapter).relationships.manga}
										class:pl-1={!!(entity as Chapter).relationships.manga}
									>
										<ReportChapterCard chapter={entity as Chapter} />
									</div>
								{:else if entityType === EntityType.ScanlationGroup}
									<ReportGroupCard group={entity as ScanlationGroup} />
								{:else if entityType === EntityType.Reply}
									{#if (entity as Reply).relationships.author}
										<ReportUserCard user={(entity as Reply).relationships.author!} />
									{/if}
									<div
										class:ml-1={!!(entity as Reply).relationships.author}
										class:border-l-1={!!(entity as Reply).relationships.author}
										class:pl-1={!!(entity as Reply).relationships.author}
									>
										<ReportReplyCard reply={entity as Reply} />
									</div>
								{/if}
							</div>
							<Separator />
						{/if}
						<Combobox.Root type="single" bind:value={selectedReasonId} disabled={$mutex} required>
							<Combobox.Trigger class="w-full border-none">
								{#if currentReason}
									{currentReason.reason}
								{:else}
									Select a reason
								{/if}
							</Combobox.Trigger>
							<Combobox.Content>
								{#if reasons?.length}
									{#each reasons as reason}
										<Combobox.Item value={reason.id}>{reason.reason}</Combobox.Item>
									{/each}
								{/if}
							</Combobox.Content>
						</Combobox.Root>
						<Textarea
							class="min-h-35 border-none !ring-0"
							placeholder="Additional Explanation {currentReason?.details_required ? '(required)' : ''}"
							bind:value={additionalExplanation}
							required={currentReason?.details_required}
							disabled={$mutex}
						/>
					</div>
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
								Send
							{/if}
						</Button>
					</Dialog.Footer>
				</form>
			</ProtectedCondition>
		</Protected>
	</Dialog.Content>
</Dialog.Root>
