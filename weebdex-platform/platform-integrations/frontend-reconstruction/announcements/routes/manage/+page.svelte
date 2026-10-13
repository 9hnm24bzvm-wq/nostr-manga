<script lang="ts">
	import { onDestroy, onMount } from "svelte";
	import { PUBLIC_SITE_NAME } from "$env/static/public";
	import { Session } from "$lib/stores";
	import * as Card from "$lib/components/ui/card";
	import { Button } from "$lib/components/ui/button";
	import { Input } from "$lib/components/ui/input";
	import { Textarea } from "$lib/components/ui/textarea";
	import { Checkbox } from "$lib/components/ui/checkbox";
	import { Label } from "$lib/components/ui/label";
	import { announcementAPI, type Announcement, type AnnouncementFeed } from "$announcements/announcement-client";
	import { refreshPinnedAnnouncements } from "$announcements/announcement.svelte";
	import { createMutationOwner } from "$platform/authenticated-mutation";
	import { nativeAuthority } from "$platform/mutation-authority";
	let ready = $state(false); let busy = $state(false); let error = $state(""); let notice = $state("");
	let feed = $state<AnnouncementFeed>({ total: 0, limit: 20, page: 1, data: [] });
	let editing = $state<Announcement>();
	let title = $state(""); let body = $state(""); let pinned = $state(false); let published = $state(false);
	const owner = createMutationOwner(() => JSON.stringify([editing?.id ?? "new", window.location.href]));
	let disposed = false, loadGeneration = 0;
	onDestroy(() => { disposed = true; loadGeneration++; });
	$effect.pre(() => { $nativeAuthority; owner.invalidate(); loadGeneration++; feed = { total: 0, limit: 20, page: 1, data: [] }; clear(); error = ""; notice = ""; });
	const staff = $derived(!!$Session && Session.hasRole("staff", "sysop"));
	const canCreate = $derived(staff && Session.hasPerm("announcement.create"));
	const canUpdate = $derived(staff && Session.hasPerm("announcement.update"));
	const canDelete = $derived(staff && Session.hasPerm("announcement.delete"));
	const canList = $derived(canCreate || canUpdate || canDelete);
	function clear() { editing = undefined; title = ""; body = ""; pinned = false; published = false; }
	function edit(item: Announcement) { editing = item; title = item.title; body = item.body; pinned = item.pinned; published = item.published; notice = ""; error = ""; }
	async function load(page = feed.page) { const generation = ++loadGeneration, authority = $nativeAuthority; const result = await announcementAPI.list({ ...(canCreate || canUpdate ? { include_drafts: 1 } : {}), limit: 20, page, order: "desc" }); if (!disposed && generation === loadGeneration && authority === $nativeAuthority) feed = result; }
	async function changePage(page: number) {
		busy = true; error = "";
		try { await load(page); } catch (failure) { error = failure instanceof Error ? failure.message : "Announcements could not be loaded"; }
		finally { busy = false; }
	}
	async function save(event: SubmitEvent) {
		event.preventDefault(); if (busy || !(editing ? canUpdate : canCreate)) return;
		const intent = owner.capture(), target = editing ? { id: editing.id, version: editing.version } : undefined, input = { title, body, pinned, published };
		busy = true; error = ""; notice = "";
		try {
			intent.assert();
			if (target) await announcementAPI.update(target.id, { ...input, version: target.version }, intent);
			else await announcementAPI.create(input, intent);
			intent.assert();
			clear(); notice = "Announcement saved.";
			try { await load(1); } catch { error = "The announcement was saved, but the list could not be refreshed. Reload to see the current version."; }
			void refreshPinnedAnnouncements();
		} catch (failure) { if (intent.alive()) error = failure instanceof Error ? failure.message : "Announcement could not be saved"; }
		finally { busy = false; intent.release(); }
	}
	async function remove(item: Announcement) {
		if (busy || !canDelete || !window.confirm(`Delete announcement “${item.title}”?`)) return;
		const intent = owner.capture(), target = { id: item.id, version: item.version };
		busy = true; error = ""; notice = "";
		try {
			intent.assert(); await announcementAPI.remove(target.id, target.version, intent); intent.assert();
			if (editing?.id === item.id) clear(); notice = "Announcement deleted.";
			try { await load(1); } catch { error = "The announcement was deleted, but the list could not be refreshed."; }
			void refreshPinnedAnnouncements();
		} catch (failure) { if (intent.alive()) error = failure instanceof Error ? failure.message : "Announcement could not be deleted"; }
		finally { busy = false; intent.release(); }
	}
	onMount(() => { void (async () => {
		try { await Session.initialize(); }
		catch (failure) { error = failure instanceof Error ? failure.message : "Your session could not be checked"; }
		finally { if (!disposed) ready = true; }
		if (disposed) return;
		if (Session.hasRole("staff", "sysop") && Session.hasPerm("announcement.create", "announcement.update", "announcement.delete")) await changePage(1);
	})(); });
</script>
<svelte:head><title>Manage announcements - {PUBLIC_SITE_NAME}</title></svelte:head>
<div class="space-y-5">
	<div class="flex items-center justify-between gap-3"><h1 class="text-2xl font-bold">Manage announcements</h1><a href="/announcements" class="text-sm underline">Public announcements</a></div>
	{#if !ready}<p role="status">Checking your session…</p>
	{:else if !canList}<p role="alert">Staff announcement permissions are required.</p>
	{:else}
		{#if error}<p role="alert" class="text-destructive">{error}</p>{/if}{#if notice}<p role="status">{notice}</p>{/if}
		{#if canCreate || editing}
			<Card.Root><Card.Header><Card.Title>{editing ? "Edit announcement" : "New announcement"}</Card.Title></Card.Header><Card.Content>
				<form class="space-y-4" onsubmit={save}>
					<div class="space-y-2"><Label for="announcement-title">Title</Label><Input id="announcement-title" bind:value={title} required maxlength={300} disabled={busy} /></div>
					<div class="space-y-2"><Label for="announcement-body">Body (Markdown)</Label><Textarea id="announcement-body" bind:value={body} required maxlength={100000} rows={8} disabled={busy} /></div>
					<div class="flex items-center gap-2"><Checkbox id="announcement-pinned" bind:checked={pinned} disabled={busy} /><Label for="announcement-pinned">Pin on the homepage</Label></div>
					<div class="flex items-center gap-2"><Checkbox id="announcement-published" bind:checked={published} disabled={busy} /><Label for="announcement-published">Published</Label></div>
					<div class="flex gap-2"><Button type="submit" disabled={busy || !(editing ? canUpdate : canCreate)}>{busy ? "Saving…" : "Save announcement"}</Button>{#if editing}<Button type="button" variant="outline" disabled={busy} onclick={clear}>Cancel edit</Button>{/if}</div>
				</form>
			</Card.Content></Card.Root>
		{/if}
		{#each feed.data as item (item.id)}
			<Card.Root><Card.Header><Card.Title><a href={`/announcements/${item.id}`}>{item.title}</a></Card.Title><Card.Description>{item.published ? "Published" : "Draft"}{item.pinned ? " · Pinned" : ""} · Version {item.version}</Card.Description></Card.Header><Card.Footer class="gap-2">{#if canUpdate}<Button variant="outline" disabled={busy} onclick={() => edit(item)}>Edit</Button>{/if}{#if canDelete}<Button variant="destructive" disabled={busy} onclick={() => remove(item)}>Delete</Button>{/if}</Card.Footer></Card.Root>
		{/each}
		{#if !busy && feed.total === 0}<p class="text-muted-foreground">No announcements yet.</p>{/if}
		<div class="flex items-center gap-3"><Button variant="outline" disabled={busy || feed.page <= 1} onclick={() => changePage(feed.page - 1)}>Previous</Button><span class="text-sm">Page {feed.page} · {feed.total} announcements</span><Button variant="outline" disabled={busy || feed.page * feed.limit >= feed.total} onclick={() => changePage(feed.page + 1)}>Next</Button><Button variant="outline" disabled={busy} onclick={() => changePage(feed.page)}>Refresh</Button></div>
	{/if}
</div>
