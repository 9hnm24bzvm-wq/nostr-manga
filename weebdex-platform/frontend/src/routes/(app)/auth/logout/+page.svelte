<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { Constants, Kratos } from "$lib";
	import * as Empty from "$lib/components/ui/empty";
	import { Spinner } from "$lib/components/ui/spinner";
	import { Session } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import { Check } from "@lucide/svelte";
	import { onMount } from "svelte";
	import { toast } from "svelte-sonner";

	let isMounted = $state(false);
	onMount(async () => {
		const redirect = decodeURIComponent(page.url.searchParams.get("redirect") ?? "/");
		if (!(await Session.isValid())) {
			return goto(redirect);
		}
		isMounted = true;

		try {
			const res = await Kratos.createBrowserLogoutFlow();
			if (res.data) {
				await Kratos.updateLogoutFlow({ token: res.data.logout_token });
				$Session = undefined;

				goto(redirect);
				toast.success("You have been logged out");
			}
		} catch (e) {
			if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
			else {
				console.error(e);
				toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
			}
		}
	});
</script>

<svelte:head>
	<title>Logout - {PUBLIC_SITE_NAME}</title>
</svelte:head>

{#if isMounted}
	<Empty.Root>
		<Empty.Header>
			<Empty.Media>
				<Check class={cn("size-10 scale-100 transition-all", $Session && "scale-0")} />
				<Spinner class={cn("absolute size-10 scale-0 transition-all", $Session && "scale-100 ")} />
			</Empty.Media>
			<Empty.Title>Logging out...</Empty.Title>
		</Empty.Header>
	</Empty.Root>
{/if}
