<script lang="ts">
	import { goto, replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { Constants, Kratos, Mutex } from "$lib";
	import ErrorComponent from "$lib/components/ErrorComponent.svelte";
	import { Button } from "$lib/components/ui/button";
	import * as Empty from "$lib/components/ui/empty";
	import * as Field from "$lib/components/ui/field";
	import * as InputGroup from "$lib/components/ui/input-group";
	import Input from "$lib/components/ui/input/input.svelte";
	import { Separator } from "$lib/components/ui/separator";
	import { Spinner } from "$lib/components/ui/spinner";
	import * as Tooltip from "$lib/components/ui/tooltip";
	import type { UINode } from "$lib/Kratos";
	import { InfoIcon, Link, Unlink } from "@lucide/svelte";
	import type {
		FrontendApiUpdateSettingsFlowRequest,
		GenericError,
		NeedsPrivilegedSessionError,
		SettingsFlow,
		UiNode
	} from "@ory/client";
	import { AxiosError, type AxiosResponse } from "axios";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";

	const { pathname } = window.location;
	const id = $props.id();
	const mutex = new Mutex();
	let mutexState = $state<string>();

	let isMounted = $state(false);
	let error = $state<Constants.Error.Error>();
	let csrfRef = $state<HTMLInputElement>();
	let profileFormRef = $state<HTMLFormElement>();
	let passwordFormRef = $state<HTMLFormElement>();
	let flow = $state<SettingsFlow>();
	let nodes = $state<{
		email?: UINode;
		username?: UINode;
		password?: UINode;
		csrf_token?: UINode;
		oidc?: UINode[];
	}>({});

	onMount(async () => {
		try {
			let promise: Promise<AxiosResponse<SettingsFlow>>;
			if (page.url.searchParams.has("flow")) {
				promise = Kratos.getSettingsFlow({ id: page.url.searchParams.get("flow")! });
			} else {
				promise = Kratos.createBrowserSettingsFlow();
			}

			const res = await promise.catch(async (err: AxiosError<SettingsFlow & GenericError>) => {
				if (err.status === StatusCodes.UNAUTHORIZED) {
					goto(`/auth/login?redirect=${encodeURIComponent(window.location.pathname + window.location.search)}`);
				}
				return err.status === StatusCodes.GONE && err.response?.data
					? Kratos.createBrowserSettingsFlow({ returnTo: err.response.data.return_to || window.location.pathname })
					: err;
			});
			if (window.location.pathname !== pathname) {
				return;
			}

			if (res instanceof AxiosError) {
				if (res.response?.data.error?.message) {
					toast.error(res.response.data.error.message, { duration: 10000 });
				}
				throw Constants.Error.Unknown;
			} else if (res.status === StatusCodes.OK && res.data) {
				const q = new URLSearchParams(window.location.search);
				q.set("flow", res.data.id);
				q.delete("redirect");

				flow = res.data;
				replaceState(window.location.pathname + q.toString(), page.state);
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

	const updateFlow = async (method: "profile" | "password" | "oidc", body: any, state?: string) => {
		if (!flow || !csrfRef) {
			return;
		}

		const request: FrontendApiUpdateSettingsFlowRequest = {
			flow: flow.id,
			updateSettingsFlowBody: {
				method: method,
				csrf_token: csrfRef.value,
				...body
			}
		};

		mutexState = state ?? method;
		try {
			type T =
				| AxiosResponse<SettingsFlow & NeedsPrivilegedSessionError>
				| AxiosError<SettingsFlow & NeedsPrivilegedSessionError>;
			const res: T = await Kratos.updateSettingsFlow(request).catch(err => err);
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
		} finally {
			mutexState = undefined;
		}
	};

	const onUpdateInformation = (ev: SubmitEvent) => {
		ev.preventDefault();
		ev.stopPropagation();
		ev.stopImmediatePropagation();

		return mutex.exec(async () => {
			if (!flow || !profileFormRef) {
				return;
			}
			return updateFlow("profile", {
				traits: {
					email: profileFormRef.email.value,
					username: profileFormRef.username.value
				}
			});
		});
	};

	const onUpdatePassword = (ev: SubmitEvent) => {
		ev.preventDefault();
		ev.stopPropagation();
		ev.stopImmediatePropagation();

		return mutex.exec(async () => {
			if (!flow || !passwordFormRef) {
				return;
			}
			return updateFlow("password", {
				password: passwordFormRef.password.value
			});
		});
	};

	const onOidc = (name: string, provider: string) =>
		mutex.exec(async () => {
			if (!flow) {
				return;
			}
			return updateFlow(
				"oidc",
				{
					[name]: provider
				},
				provider
			);
		});
</script>

<svelte:head>
	<title>Settings: Account - {PUBLIC_SITE_NAME}</title>
</svelte:head>

{#if error}
	<ErrorComponent {error} />
{:else if isMounted}
	{#if nodes?.csrf_token}
		<input type="hidden" name="csrf_token" value={nodes.csrf_token.attributes.value} required bind:this={csrfRef} />
	{/if}
	<section class="space-y-5">
		<form onsubmit={onUpdateInformation} bind:this={profileFormRef}>
			<Field.Set>
				<Field.Legend>Profile</Field.Legend>
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
							{#each nodes.username.messages as message}
								<Field.Error>{message.text}</Field.Error>
							{/each}
						</Field.Field>
					{/if}
					<Field.Field orientation="horizontal">
						<Button type="submit" disabled={$mutex}>Update Profile</Button>
					</Field.Field>
				</Field.Group>
			</Field.Set>
		</form>
		<Separator />
		<form onsubmit={onUpdatePassword} bind:this={passwordFormRef}>
			<Field.Set>
				<Field.Legend>Password</Field.Legend>
				<Field.Group>
					{#if nodes.password}
						{@const isInvalid = !!nodes.password.messages?.length}
						<Field.Field data-invalid={isInvalid}>
							<Field.Label for="password-{id}">New Password</Field.Label>
							<InputGroup.Root class="border-none !ring-0">
								<InputGroup.Input
									class="border-none !ring-0 !ring-transparent"
									id="password-{id}"
									type="password"
									name="password"
									value={nodes.password.attributes.value}
									autocomplete={nodes.password.attributes.autocomplete}
									minlength={8}
									maxlength={64}
									required={nodes.password.attributes.required}
									disabled={nodes.password.attributes.type === "hidden" || $mutex}
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
										<Tooltip.Content>Must be between 8 and 64 characters.</Tooltip.Content>
									</Tooltip.Root>
								</InputGroup.Addon>
							</InputGroup.Root>
							{#each nodes.password.messages as message}
								<Field.Error>{message.text}</Field.Error>
							{/each}
						</Field.Field>
					{/if}
					<Field.Field orientation="horizontal">
						<Button type="submit" disabled={$mutex}>Update Password</Button>
					</Field.Field>
				</Field.Group>
			</Field.Set>
		</form>
		<Separator />
		<Field.Set>
			<Field.Legend>Linked Accounts</Field.Legend>
			<Field.Group>
				<Field.Field orientation="horizontal">
					{#each nodes.oidc as node}
						{@const isLinked = node.attributes.name === "link"}
						<Button
							class="capitalize"
							type="submit"
							variant={isLinked ? "default" : "destructive"}
							onclick={() => onOidc(node.attributes.name, node.attributes.value)}
							disabled={$mutex}
						>
							{#if isLinked}
								<Unlink />
							{:else}
								<Link />
							{/if}
							{node.meta.label.text}
						</Button>
					{/each}
				</Field.Field>
			</Field.Group>
		</Field.Set>
	</section>
{:else}
	<Empty.Root>
		<Spinner class="size-10" />
	</Empty.Root>
{/if}
