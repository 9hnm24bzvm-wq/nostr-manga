<script lang="ts">
	import { API, Constants, Mutex } from "$lib";
	import { Button, buttonVariants } from "$lib/components/ui/button";
	import * as Dialog from "$lib/components/ui/dialog";
	import * as InputGroup from "$lib/components/ui/input-group";
	import { Spinner } from "$lib/components/ui/spinner";
	import * as Tooltip from "$lib/components/ui/tooltip";
	import { InfoIcon } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { toast } from "svelte-sonner";

	let {
		clients = $bindable(),
		isDialogOpen = $bindable()
	}: {
		clients: APIClient[];
		isDialogOpen: boolean;
	} = $props();

	const mutex = new Mutex();
	let name = $state<string>("");

	const onSubmit = (ev: SubmitEvent) =>
		mutex.exec(async () => {
			ev.preventDefault();
			ev.stopPropagation();
			ev.stopImmediatePropagation();

			const { pathname } = window.location;
			try {
				const res = await API.Client.Create(name);
				if (window.location.pathname !== pathname) {
					return;
				}

				if (res?.status === StatusCodes.CREATED && res.body) {
					isDialogOpen = false;
					clients.push(res.body);
					clients = clients;
					toast.success("Client has been successfully created");
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
	<Dialog.Title>Create Client</Dialog.Title>
</Dialog.Header>
<form class="space-y-5" onsubmit={onSubmit}>
	<InputGroup.Root class="border-none !ring-0">
		<InputGroup.Input
			placeholder="Client Name (required)"
			bind:value={name}
			minlength={5}
			maxlength={255}
			autocomplete="off"
			required
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
				<Tooltip.Content>Must be between 5 and 255 characters.</Tooltip.Content>
			</Tooltip.Root>
		</InputGroup.Addon>
	</InputGroup.Root>
	<Dialog.Footer>
		<Dialog.Close class={buttonVariants({ class: "border-none", variant: "outline" })} type="button" disabled={$mutex}>
			Cancel
		</Dialog.Close>
		<Button class="w-30" type="submit" disabled={$mutex || name.length < 5 || name.length > 255}>
			{#if $mutex}
				<Spinner />
			{:else}
				Create
			{/if}
		</Button>
	</Dialog.Footer>
</form>
