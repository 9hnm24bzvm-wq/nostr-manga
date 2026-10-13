<script lang="ts">
	import { API, Constants, Mutex, Permission } from "$lib";
	import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
	import { Button } from "$lib/components/ui/button";
	import { Clipboard, RefreshCcw, Trash } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { toast } from "svelte-sonner";

	let {
		clients = $bindable(),
		client = $bindable()
	}: {
		clients: APIClient[];
		client: APIClient;
	} = $props();

	let isRegenerating = $state(false);
	let isDeleting = $state(false);

	const copy = () => {
		if (client.key) {
			navigator.clipboard.writeText(client.key);
			toast.success("Client secret has been successfully copied to the clipboard");
		}
	};

	const onRegenerate = (mutex: Mutex) =>
		mutex.exec(async () => {
			const { pathname } = window.location;
			isRegenerating = true;
			try {
				const res = await API.Client.RegenerateSecret(client.id);
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.OK && res.body) {
					client = res.body;
					clients = clients;
					toast.success("Client secret has been successfully regenerated");
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
			} finally {
				isRegenerating = false;
			}
		});

	const onDelete = (mutex: Mutex) =>
		mutex.exec(async () => {
			const { pathname } = window.location;
			isDeleting = true;
			try {
				const res = await API.Client.Delete(client.id);
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.NO_CONTENT) {
					const idx = clients.findIndex(e => e.id === client.id);
					if (idx >= 0) {
						clients.splice(idx, 1);
						clients = clients;
					}
					toast.success("Client has been successfully deleted");
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
			} finally {
				isDeleting = false;
			}
		});
</script>

<div class="bg-card flex overflow-hidden rounded-md p-2">
	<div class="w-0 flex-auto">
		<div>
			<strong class="font-semibold">Client ID:</strong>
			<span>{client.id}</span>
		</div>
		<div>
			<strong class="font-semibold">Name:</strong>
			<span>{client.name}</span>
		</div>
		<div>
			<strong class="font-semibold">Created Date:</strong>
			<time datetime={client.created_at}>
				{new Date(client.created_at).toLocaleString("en-US", { dateStyle: "short", timeStyle: "short" })}
			</time>
		</div>
	</div>
	<div class="flex shrink-0 gap-1">
		{#if client.key}
			<Button title="Copy Client Secret" type="button" variant="ghost" size="icon-sm" onclick={copy}>
				<Clipboard class="size-5" />
			</Button>
		{:else if Permission.Client.canUpdate()}
			<ConfirmDialog
				title="Regenerate Client Secret"
				description="Are you sure you want to regenerate this client secret?"
				confirmText="Regenerate"
				variant="default"
				onConfirm={onRegenerate}
			>
				{#snippet child({ props })}
					<Button title="Regenerate Client Secret" type="button" variant="ghost" size="icon-sm" {...props}>
						<RefreshCcw class="size-5" />
					</Button>
				{/snippet}
			</ConfirmDialog>
		{/if}
		{#if Permission.Client.canDelete()}
			<ConfirmDialog
				title="Delete Client"
				description="Are you sure you want to delete this client?"
				confirmText="Delete"
				variant="destructive"
				onConfirm={onDelete}
			>
				{#snippet child({ props })}
					<Button title="Delete Client" type="button" variant="ghost" size="icon-sm" {...props}>
						<Trash class="size-5" />
					</Button>
				{/snippet}
			</ConfirmDialog>
		{/if}
	</div>
</div>
