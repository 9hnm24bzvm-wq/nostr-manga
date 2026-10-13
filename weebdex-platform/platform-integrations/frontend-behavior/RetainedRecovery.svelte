<script lang="ts">
	import { goto, replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { Constants, Kratos, Mutex } from "$lib";
	import { Button } from "$lib/components/ui/button";
	import * as Card from "$lib/components/ui/card";
	import * as Field from "$lib/components/ui/field";
	import { Input } from "$lib/components/ui/input";
	import Spinner from "$lib/components/ui/spinner/spinner.svelte";
	import { ErrorSessionAlreadyAvailable, type UINode } from "$lib/Kratos";
	import { Session } from "$lib/stores";
	import type {
		FrontendApiUpdateRecoveryFlowRequest,
		GenericError,
		NeedsPrivilegedSessionError,
		RecoveryFlow,
		UiNode
	} from "@ory/client";
	import { AxiosError, type AxiosResponse } from "axios";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";
	import { fade } from "svelte/transition";

	const id = $props.id();
	const mutex = new Mutex();
	const { pathname } = window.location;

	let isMounted = $state(false);
	let ref = $state<HTMLFormElement>();
	let flow = $state<RecoveryFlow>();
	let nodes = $state<{
		csrf_token?: UINode;
		email?: UINode;
	}>({});

	onMount(async () => {
		const redirect = decodeURIComponent(page.url.searchParams.get("redirect") ?? "/");
		if (await Session.isValid()) {
			return goto(redirect);
		}

		try {
			let promise: Promise<AxiosResponse<RecoveryFlow>>;
			if (page.url.searchParams.has("flow")) {
				promise = Kratos.getRecoveryFlow({ id: page.url.searchParams.get("flow")! });
			} else {
				promise = Kratos.createBrowserRecoveryFlow({ returnTo: redirect });
			}

			const res = await promise.catch(async (err: AxiosError<RecoveryFlow & GenericError>) => {
				if (
					err.status === StatusCodes.BAD_REQUEST &&
					<string>err.response?.data.error?.id === ErrorSessionAlreadyAvailable
				) {
					throw ErrorSessionAlreadyAvailable;
				}
				return err.status === StatusCodes.GONE && err.response?.data
					? Kratos.createBrowserRecoveryFlow({ returnTo: err.response.data.return_to || redirect })
					: err;
			});
			if (window.location.pathname !== pathname) {
				return;
			}

			if (res instanceof AxiosError) {
				if (res.response?.data.error?.message) {
					toast.error(res.response.data.error.message, { duration: 10000 });
				} else throw Constants.Error.Unknown;
			} else if (res.status === StatusCodes.OK && res.data) {
				const q = new URLSearchParams(window.location.search);
				q.set("flow", res.data.id);
				q.delete("redirect");

				flow = res.data;
				replaceState(window.location.pathname + q.toString(), page.state);
			} else throw Constants.Error.Unknown;
		} catch (e) {
			if (e === ErrorSessionAlreadyAvailable) goto(redirect);
			else if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
			else {
				console.error(e);
				toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
			}
		}
		isMounted = true;
	});

	$effect(() => {
		flow?.ui.messages?.forEach(message => {
			switch (message.type) {
				case "success":
					toast.success(message.text, { duration: 10000 });
					break;
				case "info":
					toast.info(message.text, { duration: 10000 });
					break;
				default:
					toast.error(message.text, { duration: 10000 });
					break;
			}
		});
	});

	$effect(() => {
		if (flow?.ui.nodes) {
			nodes = flow.ui.nodes.reduce(
				(acc, node) => {
					const name = <string>(<any>node.attributes).name;
					if (["csrf_token", "email"].includes(name)) {
						(<UiNode>acc[<keyof typeof nodes>name.replaceAll("traits.", "")]) = node;
					}
					return acc;
				},
				{} as typeof nodes
			);
		}
	});

	const updateFlow = async (method: "link", body: any) => {
		if (!flow || !ref) {
			return;
		}

		const request: FrontendApiUpdateRecoveryFlowRequest = {
			flow: flow.id,
			updateRecoveryFlowBody: {
				method: method,
				csrf_token: ref.csrf_token.value,
				...body
			}
		};

		try {
			type T =
				| AxiosResponse<RecoveryFlow & NeedsPrivilegedSessionError>
				| AxiosError<RecoveryFlow & NeedsPrivilegedSessionError>;
			const res: T = await Kratos.updateRecoveryFlow(request).catch(err => err);
			if (window.location.pathname !== pathname) {
				return;
			}

			const data = res instanceof AxiosError ? res.response?.data : res.data;
			if (data?.redirect_browser_to) {
				window.location.href = data.redirect_browser_to;
			} else if (data?.ui) {
				flow = data;
			} else if (data?.error?.reason) {
				toast.error(data.error.reason, { duration: 10000 });
			} else throw Constants.Error.Unknown;
		} catch (e) {
			if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
			else {
				console.error(e);
				toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
			}
		}
	};

	const onSubmit = async (ev: SubmitEvent) => {
		ev.preventDefault();
		return mutex.exec(async () => {
			if (!ref) {
				return;
			}
			return updateFlow("link", {
				email: ref.email.value
			});
		});
	};
</script>

<svelte:head>
	<title>Account Recovery - {PUBLIC_SITE_NAME}</title>
</svelte:head>

{#if isMounted}
	<Card.Root class="relative rounded">
		<Card.Header class="text-center">
			<Card.Title class="text-xl">Account Recovery</Card.Title>
			<Card.Description>Enter your email address to receive a recovery link</Card.Description>
		</Card.Header>
		<Card.Content>
			<form onsubmit={onSubmit} bind:this={ref}>
				<Field.Group>
					{#if nodes.email}
						{@const isInvalid = !!nodes.email.messages?.length}
						<Field.Field data-invalid={isInvalid}>
							<Field.Label for="email-{id}">Email</Field.Label>
							<Input
								class="border-none !ring-0 !ring-transparent"
								id="email-{id}"
								type="email"
								name="email"
								value={nodes.email.attributes.value}
								autocomplete={nodes.email.attributes.autocomplete}
								minlength={3}
								maxlength={320}
								required={nodes.email.attributes.required}
								disabled={nodes.email.attributes.type === "hidden" || $mutex}
								aria-invalid={isInvalid}
							/>
							{#each nodes.email.messages as message}
								<Field.Error>{message.text}</Field.Error>
							{/each}
						</Field.Field>
					{/if}
					<Field.Field>
						<Button type="submit" disabled={$mutex}>Send Recovery Link</Button>
					</Field.Field>
				</Field.Group>
				{#if nodes.csrf_token}
					<input type="hidden" name="csrf_token" value={nodes.csrf_token.attributes.value} required />
				{/if}
			</form>
		</Card.Content>
		{#if $mutex}
			<div class="absolute inset-0 flex bg-black/25" transition:fade={{ duration: 100 }}>
				<Spinner class="m-auto size-10" />
			</div>
		{/if}
	</Card.Root>
{:else}
	<Spinner class="m-auto size-10" />
{/if}
