<script lang="ts">
	import { goto, replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { Constants, Kratos } from "$lib";
	import * as Card from "$lib/components/ui/card";
	import * as Empty from "$lib/components/ui/empty/index.js";
	import Spinner from "$lib/components/ui/spinner/spinner.svelte";
	import { ErrorSessionAlreadyAvailable } from "$lib/Kratos";
	import type { FlowError } from "@ory/client";
	import { AxiosError } from "axios";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";

	const { pathname } = window.location;
	let isMounted = $state(false);
	let flow = $state<FlowError>();

	onMount(async () => {
		const redirect = decodeURIComponent(page.url.searchParams.get("redirect") ?? "/");
		try {
			if (!page.url.searchParams.has("flow")) {
				return goto(redirect);
			}

			const res = await Kratos.getFlowError({ id: page.url.searchParams.get("flow")! });
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
</script>

<svelte:head>
	<title>Error - {PUBLIC_SITE_NAME}</title>
</svelte:head>

{#if isMounted}
	<Empty.Root>
		<Empty.Header>
			<Empty.Title>Error</Empty.Title>
			{#if (<any>flow?.error)?.message}
				<Empty.Description>{(<any>flow?.error)?.message}</Empty.Description>
			{:else}
				<Card.Description>{Constants.Error.Unknown.body}</Card.Description>
			{/if}
		</Empty.Header>
	</Empty.Root>
{:else}
	<Spinner class="m-auto size-10" />
{/if}
