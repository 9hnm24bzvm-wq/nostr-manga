<script lang="ts">
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { Constants, Utils } from "$lib";
	import API from "$lib/API";
	import ErrorComponent from "$lib/components/ErrorComponent.svelte";
	import * as Empty from "$lib/components/ui/empty";
	import { Spinner } from "$lib/components/ui/spinner";
	import UploadChapterEdit from "$lib/components/upload/UploadChapterEdit.svelte";
	import { Session } from "$lib/stores";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";

	const { data } = $props();
	let chapter = $derived(data.chapter);
	let user = $state<User>();
	let isMounted = $state(false);
	let error = $state<Constants.Error.Error>();

	onMount(async () => {
		try {
			await Promise.all([getUser()]);
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
</script>

<svelte:head>
	<title>Edit: {Utils.chapterHeadline(chapter)} - {PUBLIC_SITE_NAME}</title>
</svelte:head>

{#if error}
	<ErrorComponent {error} />
{:else if isMounted && user}
	<UploadChapterEdit bind:chapter manga={chapter.relationships.manga} {user} />
{:else}
	<Empty.Root>
		<Spinner class="size-10" />
	</Empty.Root>
{/if}
