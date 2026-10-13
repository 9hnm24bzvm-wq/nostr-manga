<script lang="ts">
	import { page } from "$app/state";
	import { onDestroy } from "svelte";
	import { nativeAuthority } from "$platform/mutation-authority";
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { Button } from "$lib/components/ui/button";
	import AnnouncementEntry from "$announcements/AnnouncementEntry.svelte";
	import { announcementAPI, type Announcement } from "$announcements/announcement-client";
	let disposed = false; onDestroy(() => { disposed = true; generation++; });
	let announcement = $state<Announcement>(); let loading = $state(false); let error = $state(""); let generation = 0;
	async function load(id: string) {
		const current = ++generation; loading = true; error = ""; announcement = undefined;
		try { const result = await announcementAPI.get(id); if (!disposed && current === generation) announcement = result; }
		catch (failure) { if (!disposed && current === generation) error = failure instanceof Error ? failure.message : "Announcement could not be loaded"; }
		finally { if (!disposed && current === generation) loading = false; }
	}
	$effect.pre(() => { $nativeAuthority; const id = page.params.id; if (id) void load(id); });
</script>
<svelte:head><title>{announcement?.title ?? "Announcement"} - {PUBLIC_SITE_NAME}</title></svelte:head>
<div class="space-y-5">
	{#if loading}<p role="status">Loading announcement…</p>{/if}
	{#if error}<p role="alert" class="text-destructive">{error}</p><Button onclick={() => load(page.params.id!)}>Retry</Button>{/if}
	{#if announcement}<AnnouncementEntry {announcement} detail />{/if}
	<a href="/announcements" class="text-sm underline">All announcements</a>
</div>
