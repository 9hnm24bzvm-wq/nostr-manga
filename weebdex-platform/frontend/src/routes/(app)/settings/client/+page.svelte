<script lang="ts">
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { API, Constants, Permission } from "$lib";
	import ErrorComponent from "$lib/components/ErrorComponent.svelte";
	import { buttonVariants } from "$lib/components/ui/button/button.svelte";
	import * as Dialog from "$lib/components/ui/dialog";
	import * as Empty from "$lib/components/ui/empty";
	import { Spinner } from "$lib/components/ui/spinner";
	import * as Tooltip from "$lib/components/ui/tooltip";
	import { PlusIcon } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import ClientCreateDialog from "./ClientCreateDialog.svelte";
	import ClientEntry from "./ClientEntry.svelte";

	let clients = $state<APIClient[]>([]);
	let isMounted = $state(false);
	let isDialogOpen = $state(false);
	let error = $state<Constants.Error.Error>();

	onMount(async () => {
		const { pathname } = window.location;
		try {
			const res = await API.Client.List();
			if (window.location.pathname !== pathname) {
				return;
			}

			if (res?.status === StatusCodes.OK && res.body) {
				clients = res.body;
			} else if (res?.status === StatusCodes.FORBIDDEN) {
				throw Constants.Error.Forbidden;
			} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
				throw Constants.Error.TooManyRequests;
			} else throw Constants.Error.Unknown;
		} catch (e) {
			if (e instanceof Constants.Error.Error) error = e;
			else {
				console.error(e);
				error = Constants.Error.Unknown;
			}
		}
		isMounted = true;
	});
</script>

<svelte:head>
	<title>Settings: Client - {PUBLIC_SITE_NAME}</title>
</svelte:head>

{#if error}
	<ErrorComponent {error} />
{:else if isMounted}
	<section>
		<div class="space-y-2">
			<div
				class="rounded-md border border-dashed border-yellow-500/50 bg-yellow-500/15 p-3 text-center text-yellow-300"
			>
				<p>Client secret is only visible once after creation.</p>
				<p>Please copy it and store it somewhere safe.</p>
				<p>If you lose your client secret, you will need to regenerate it.</p>
			</div>
			<Dialog.Root bind:open={isDialogOpen}>
				{#if clients.length}
					{#if Permission.Client.canCreate()}
						<Dialog.Trigger
							class={buttonVariants({
								class:
									"w-full border border-dashed border-green-500/50 bg-green-500/15 !p-3 text-green-300 hover:!bg-green-500/20",
								variant: "ghost"
							})}
							type="button"
						>
							<PlusIcon />
							Add New Client
						</Dialog.Trigger>
					{/if}
					<Tooltip.Provider>
						<div class="space-y-1">
							{#each clients as client, i (client)}
								<ClientEntry bind:clients bind:client={clients[i]} />
							{/each}
						</div>
					</Tooltip.Provider>
				{:else}
					<Empty.Root>
						<Empty.Header>
							<Empty.Title>No clients found</Empty.Title>
							<Empty.Description>No clients have been added to your account.</Empty.Description>
						</Empty.Header>
						{#if Permission.Client.canCreate()}
							<Empty.Content>
								<Dialog.Trigger class={buttonVariants()} type="button">Add New Client</Dialog.Trigger>
							</Empty.Content>
						{/if}
					</Empty.Root>
				{/if}
				<Dialog.Content
					class="flex max-h-[calc(100dvh-2rem)] !max-w-[min(30rem,calc(100dvw-2rem))] flex-col overflow-auto"
				>
					<ClientCreateDialog bind:clients bind:isDialogOpen />
				</Dialog.Content>
			</Dialog.Root>
		</div>
	</section>
{:else}
	<Empty.Root>
		<Spinner class="size-10" />
	</Empty.Root>
{/if}
