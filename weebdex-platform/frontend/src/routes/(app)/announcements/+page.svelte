<script lang="ts">
	import { onMount } from "svelte";
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { Session } from "$lib/stores";
	import { Button } from "$lib/components/ui/button";
	import AnnouncementEntry from "$announcements/AnnouncementEntry.svelte";
	import { announcementAPI, type AnnouncementFeed } from "$announcements/announcement-client";
	let feed = $state<AnnouncementFeed>({ total: 0, limit: 20, page: 1, data: [] });
	let loading = $state(false); let error = $state("");
	const canManage = $derived(!!$Session && Session.hasRole("staff", "sysop") && Session.hasPerm("announcement.create", "announcement.update", "announcement.delete"));
	async function load(page = 1) {
		loading = true; error = "";
		try { feed = await announcementAPI.list({ limit: 20, page, order: "desc" }); }
		catch (failure) { error = failure instanceof Error ? failure.message : "Announcements could not be loaded"; }
		finally { loading = false; }
	}
	onMount(() => { void load(); });
</script>
<svelte:head><title>Announcements - {PUBLIC_SITE_NAME}</title></svelte:head>
<div class="space-y-5">
	<div class="flex items-center justify-between gap-3"><h1 class="text-2xl font-bold">Announcements</h1>{#if canManage}<a href="/announcements/manage" class="text-sm underline">Manage announcements</a>{/if}</div>
	{#if error}<p role="alert" class="text-destructive">{error}</p><Button disabled={loading} onclick={() => load(feed.page)}>Retry</Button>{/if}
	{#if loading}<p role="status">Loading announcements…</p>{/if}
	{#each feed.data as announcement (announcement.id)}<AnnouncementEntry {announcement} />{/each}
	{#if !loading && !error && feed.total === 0}<p class="text-muted-foreground">No announcements have been published.</p>{/if}
	<div class="flex items-center gap-3"><Button variant="outline" disabled={loading || feed.page <= 1} onclick={() => load(feed.page - 1)}>Previous</Button><span class="text-sm">Page {feed.page} · {feed.total} announcements</span><Button variant="outline" disabled={loading || feed.page * feed.limit >= feed.total} onclick={() => load(feed.page + 1)}>Next</Button></div>
</div>
