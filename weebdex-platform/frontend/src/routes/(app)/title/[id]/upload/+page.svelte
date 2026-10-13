<script lang="ts">
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { Constants } from "$lib";
	import API from "$lib/API";
	import ErrorComponent from "$lib/components/ErrorComponent.svelte";
	import * as Empty from "$lib/components/ui/empty";
	import { Spinner } from "$lib/components/ui/spinner";
	import UploadChapter from "$lib/components/upload/UploadChapter.svelte";
	import { Session } from "$lib/stores";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";

	const { data } = $props();
	let user = $state<User>();
	let isMounted = $state(false);
	let error = $state<Constants.Error.Error>();

	onMount(async () => {
		try {
			await Promise.all([getUser(), deleteExistingUploadSession()]);
		} catch (e) {
			if (e instanceof Constants.Error.Error) error = e;
			else {
				console.error(e);
				error = Constants.Error.Unknown;
			}
		}
		isMounted = true;
	});

	const getUser = async () => {
		if (!$Session) return;

		const res = await API.User.Get($Session.identity.id);
		if (res?.status === StatusCodes.OK && res.body) {
			user = res.body;
		} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
			throw Constants.Error.TooManyRequests;
		} else throw Constants.Error.Unknown;
	};

	const deleteExistingUploadSession = async () => {
		const res = await API.UploadSession.Get();
		if (res?.status === StatusCodes.OK && res.body?.id) {
			await API.UploadSession.Delete(res.body.id);
		} else if (res?.status === StatusCodes.FORBIDDEN) {
			throw Constants.Error.Forbidden;
		} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
			throw Constants.Error.TooManyRequests;
		} else if (res?.status !== StatusCodes.NO_CONTENT) {
			throw Constants.Error.Unknown;
		}
	};
</script>

<svelte:head>
	{#if data.manga}
		<title>Upload: {data.manga.title} - {PUBLIC_SITE_NAME}</title>
	{:else}
		<title>Upload - {PUBLIC_SITE_NAME}</title>
	{/if}
</svelte:head>

{#if error}
	<ErrorComponent {error} />
{:else if isMounted && data.manga && user}
	<UploadChapter manga={data.manga} {user} />
{:else}
	<Empty.Root>
		<Spinner class="size-10" />
	</Empty.Root>
{/if}
