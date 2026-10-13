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
	import type {
		FrontendApiUpdateLoginFlowRequest,
		GenericError,
		LoginFlow,
		NeedsPrivilegedSessionError,
		SuccessfulNativeLogin,
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
	let flow = $state<LoginFlow>();
	let nodes = $state<{
		identifier?: UINode;
		password?: UINode;
		csrf_token?: UINode;
		oidc?: UINode[];
	}>({});

	onMount(async () => {
		const redirect = decodeURIComponent(page.url.searchParams.get("redirect") ?? "/");

		try {
			let promise: Promise<AxiosResponse<LoginFlow>>;
			if (page.url.searchParams.has("flow")) {
				promise = Kratos.getLoginFlow({ id: page.url.searchParams.get("flow")! });
			} else {
				promise = Kratos.createBrowserLoginFlow({ returnTo: redirect, refresh: true });
			}

			const res = await promise.catch(async (err: AxiosError<LoginFlow & GenericError>) => {
				if (
					err.status === StatusCodes.BAD_REQUEST &&
					<string>err.response?.data.error?.id === ErrorSessionAlreadyAvailable
				) {
					throw ErrorSessionAlreadyAvailable;
				}
				return err.status === StatusCodes.GONE && err.response?.data
					? Kratos.createBrowserLoginFlow({ returnTo: err.response.data.return_to || redirect })
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
					if (["csrf_token", "identifier", "password"].includes(name)) {
						(<UiNode>acc[<keyof typeof nodes>name.replaceAll("traits.", "")]) = node;
					} else if (["provider", "link", "unlink"].includes(name)) {
						((<UiNode[]>acc["oidc"]) ??= []).push(node);
					}
					return acc;
				},
				{} as typeof nodes
			);
		}
	});

	const updateFlow = async (method: "password" | "oidc", body: any) => {
		if (!flow || !ref) {
			return;
		}

		const request: FrontendApiUpdateLoginFlowRequest = {
			flow: flow.id,
			updateLoginFlowBody: {
				method: method,
				csrf_token: ref.csrf_token.value,
				...body
			}
		};

		try {
			type T =
				| AxiosResponse<LoginFlow & SuccessfulNativeLogin & NeedsPrivilegedSessionError>
				| AxiosError<LoginFlow & SuccessfulNativeLogin & NeedsPrivilegedSessionError>;
			const res: T = await Kratos.updateLoginFlow(request).catch(err => err);
			if (window.location.pathname !== pathname) {
				return;
			}

			const data = res instanceof AxiosError ? res.response?.data : res.data;
			if (data?.session?.active) {
				if (flow.return_to && !flow.return_to?.startsWith(window.location.origin)) {
					window.location.href = flow.return_to;
				} else goto(flow.return_to || "/");
			} else if (data?.redirect_browser_to) {
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
			return updateFlow("password", {
				identifier: ref.identifier.value,
				password: ref.password.value
			});
		});
	};

	const onOidc = async (name: string, provider: string) =>
		mutex.exec(async () => updateFlow("oidc", { [name]: provider }));
</script>

<svelte:head>
	<title>Login - {PUBLIC_SITE_NAME}</title>
</svelte:head>

{#if isMounted}
	<Card.Root class="relative rounded">
		<Card.Header class="text-center">
			<Card.Title class="text-xl">Welcome back</Card.Title>
			<Card.Description>Enter your credentials below to login to your account</Card.Description>
		</Card.Header>
		<Card.Content>
			<form onsubmit={onSubmit} bind:this={ref}>
				<Field.Group>
					{#if nodes.identifier}
						{@const isInvalid = !!nodes.identifier.messages?.length}
						<Field.Field data-invalid={isInvalid}>
							<Field.Label for="identifier-{id}">Email or username</Field.Label>
							<Input
								class="border-none !ring-0 !ring-transparent"
								id="identifier-{id}"
								type="text"
								name="identifier"
								value={nodes.identifier.attributes.value}
								autocomplete={nodes.identifier.attributes.autocomplete}
								minlength={3}
								maxlength={320}
								required={nodes.identifier.attributes.required}
								disabled={nodes.identifier.attributes.type === "hidden" || $mutex}
								aria-invalid={isInvalid}
							/>
							{#each nodes.identifier.messages as message}
								<Field.Error>{message.text}</Field.Error>
							{/each}
						</Field.Field>
					{/if}
					{#if nodes.password}
						<Field.Field data-invalid={!!nodes.password.messages?.length}>
							<div class="flex items-center">
								<Field.Label for="password-{id}">Password</Field.Label>
								<a
									href="/auth/recovery{new URLSearchParams({ redirect: flow?.return_to || '/' }).toString()}"
									class="ms-auto text-sm underline-offset-4 hover:underline"
								>
									Forgot your password?
								</a>
							</div>
							<Input
								class="border-none !ring-0 !ring-transparent"
								id="password-{id}"
								type="password"
								name="password"
								value={nodes.password.attributes.value}
								minlength={8}
								maxlength={64}
								required={nodes.password.attributes.required}
								disabled={nodes.password.attributes.type === "hidden" || $mutex}
							/>
							{#each nodes.password.messages as message}
								<Field.Error>{message.text}</Field.Error>
							{/each}
						</Field.Field>
					{/if}
					<Field.Field>
						<Button type="submit" disabled={$mutex}>Sign in</Button>
					</Field.Field>
					{#if nodes.oidc?.length}
						<Field.Separator class="*:data-[slot=field-separator-content]:bg-card">Or continue with</Field.Separator>
					{/if}
					<Field.Field>
						{#if nodes.oidc?.length}
							{#each nodes.oidc as node (node.attributes.name)}
								<Button
									class="border-none"
									variant="outline"
									type="button"
									disabled={$mutex}
									onclick={() => onOidc(node.attributes.name, node.attributes.value)}
								>
									{node.meta.label.text}
								</Button>
							{/each}
						{/if}
						<Field.Description class="text-center">
							Don't have an account?
							<a href="/auth/register{new URLSearchParams({ redirect: flow?.return_to || '/' }).toString()}">
								Sign up
							</a>
						</Field.Description>
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
