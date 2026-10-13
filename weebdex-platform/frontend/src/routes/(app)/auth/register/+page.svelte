<script lang="ts">
	import { goto, replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { Constants, Kratos, Mutex } from "$lib";
	import { Button } from "$lib/components/ui/button";
	import * as Card from "$lib/components/ui/card";
	import * as Field from "$lib/components/ui/field";
	import { Input } from "$lib/components/ui/input";
	import * as InputGroup from "$lib/components/ui/input-group";
	import Spinner from "$lib/components/ui/spinner/spinner.svelte";
	import * as Tooltip from "$lib/components/ui/tooltip";
	import { ErrorSessionAlreadyAvailable, type UINode } from "$lib/Kratos";
	import { Session } from "$lib/stores";
	import { InfoIcon } from "@lucide/svelte";
	import type {
		FrontendApiUpdateRegistrationFlowRequest,
		GenericError,
		NeedsPrivilegedSessionError,
		RegistrationFlow,
		SuccessfulNativeRegistration,
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
	let flow = $state<RegistrationFlow>();
	let nodes = $state<{
		identifier?: UINode;
		email?: UINode;
		username?: UINode;
		password?: UINode;
		csrf_token?: UINode;
		oidc?: UINode[];
	}>({});

	onMount(async () => {
		const redirect = decodeURIComponent(page.url.searchParams.get("redirect") ?? "/");
		if (await Session.isValid()) {
			return goto(redirect);
		}

		try {
			let promise: Promise<AxiosResponse<RegistrationFlow>>;
			if (page.url.searchParams.has("flow")) {
				promise = Kratos.getRegistrationFlow({ id: page.url.searchParams.get("flow")! });
			} else {
				promise = Kratos.createBrowserRegistrationFlow({ returnTo: redirect });
			}

			const res = await promise.catch(async (err: AxiosError<RegistrationFlow & GenericError>) => {
				if (
					err.status === StatusCodes.BAD_REQUEST &&
					<string>err.response?.data.error?.id === ErrorSessionAlreadyAvailable
				) {
					throw ErrorSessionAlreadyAvailable;
				}
				return err.status === StatusCodes.GONE && err.response?.data
					? Kratos.createBrowserRegistrationFlow({ returnTo: err.response.data.return_to || redirect })
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
					if (["csrf_token", "traits.email", "traits.username", "password"].includes(name)) {
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

		const request: FrontendApiUpdateRegistrationFlowRequest = {
			flow: flow.id,
			updateRegistrationFlowBody: {
				method: method,
				csrf_token: ref.csrf_token.value,
				...body
			}
		};

		try {
			type T =
				| AxiosResponse<RegistrationFlow & SuccessfulNativeRegistration & NeedsPrivilegedSessionError>
				| AxiosError<RegistrationFlow & SuccessfulNativeRegistration & NeedsPrivilegedSessionError>;
			const res: T = await Kratos.updateRegistrationFlow(request).catch(err => err);
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
				password: ref.password.value,
				traits: {
					email: ref.email.value,
					username: ref.username.value
				}
			});
		});
	};

	const onOidc = async (name: string, provider: string) =>
		mutex.exec(async () => updateFlow("oidc", { [name]: provider }));
</script>

<svelte:head>
	<title>Register - {PUBLIC_SITE_NAME}</title>
</svelte:head>

{#if isMounted}
	<Card.Root class="relative rounded">
		<Card.Header class="text-center">
			<Card.Title class="text-xl">Create your account</Card.Title>
			<Card.Description>Fill in the form below to create your account</Card.Description>
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
							{#if nodes.email.messages?.length}
								{#each nodes.email.messages as message}
									<Field.Error>{message.text}</Field.Error>
								{/each}
							{:else}
								<Field.Description>
									For account verification and password resets.<br />Can be a disposable email and can be changed at any
									time.
								</Field.Description>
							{/if}
						</Field.Field>
					{/if}
					{#if nodes.username}
						{@const isInvalid = !!nodes.username.messages?.length}
						<Field.Field data-invalid={isInvalid}>
							<Field.Label for="username-{id}">Username</Field.Label>
							<InputGroup.Root class="border-none !ring-0">
								<InputGroup.Input
									class="border-none !ring-0 !ring-transparent"
									id="username-{id}"
									type="text"
									name="username"
									value={nodes.username.attributes.value}
									autocomplete={nodes.username.attributes.autocomplete}
									minlength={3}
									maxlength={32}
									required={nodes.username.attributes.required}
									disabled={nodes.username.attributes.type === "hidden" || $mutex}
									aria-invalid={isInvalid}
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
										<Tooltip.Content>Must be between 3 and 32 characters.</Tooltip.Content>
									</Tooltip.Root>
								</InputGroup.Addon>
							</InputGroup.Root>
							{#if nodes.username.messages?.length}
								{#each nodes.username.messages as message}
									<Field.Error>{message.text}</Field.Error>
								{/each}
							{:else}
								<Field.Description
									>Must be between 3 and 32 characters.<br />Can be changed at anytime.</Field.Description
								>
							{/if}
						</Field.Field>
					{/if}
					<Field.Field data-invalid={!!nodes.password?.messages?.length}>
						<div class="flex items-center">
							<Field.Label for="password-{id}">Password</Field.Label>
						</div>

						<InputGroup.Root class="border-none !ring-0">
							<InputGroup.Input
								class="border-none !ring-0 !ring-transparent"
								id="password-{id}"
								type="password"
								name="password"
								value={nodes.password?.attributes.value}
								minlength={8}
								maxlength={64}
								required={nodes.password?.attributes.required}
								disabled={nodes.password?.attributes.type === "hidden" || $mutex}
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
									<Tooltip.Content>Must be between 8 and 64 characters.</Tooltip.Content>
								</Tooltip.Root>
							</InputGroup.Addon>
						</InputGroup.Root>
						{#if nodes.password?.messages?.length}
							{#each nodes.password?.messages as message}
								<Field.Error>{message.text}</Field.Error>
							{/each}
						{:else}
							<Field.Description>Must be between 8 and 64 characters.</Field.Description>
						{/if}
					</Field.Field>
					<Field.Field>
						<Button type="submit" disabled={$mutex}>Create Account</Button>
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
							Already have an account?
							<a href="/auth/login{new URLSearchParams({ redirect: flow?.return_to || '/' }).toString()}">Sign in</a>
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
