<script lang="ts">
	import API from "$lib/API";
	import { createEditorCoverReader } from "./editor-cover-read";
	import { onDestroy, onMount } from "svelte";
	import { Constants } from "$lib";
	import RetainedMangaEdit from "$lib/components/draft/manga/MangaEdit.svelte";
	import ErrorComponent from "$lib/components/ErrorComponent.svelte";
	import { Button } from "$lib/components/ui/button";
	import * as Card from "$lib/components/ui/card";
	type Receipt = Manga & { publication_scope?: string; publication_status?: string; protocol_event_id?: string; is_unavailable?: boolean; can_mutate?: boolean; can_cancel?: boolean; can_retry_publication?: boolean; publication_retry_version?: number };
	const { data }: { data: { manga: Receipt; covers: Cover[] } } = $props();
	let current = $state<Receipt>(data.manga);
	let covers = $state<Cover[]>(data.covers);
	let available = $state(true), busy = $state(false), uncertain = $state(false), failure = $state<Constants.Error.Error>();
	let targetGeneration = 0, disposed = false, activeTarget = data.manga.id;
	$effect(() => { const incoming = data.manga; if (incoming.id !== activeTarget) { activeTarget = incoming.id; targetGeneration++; busy = false; } current = incoming; covers = data.covers; available = true; uncertain = false; failure = undefined; });
	onDestroy(() => { disposed = true; targetGeneration++; });
	const editorCoverReader = createEditorCoverReader(() => data.manga.id);
	const captureTarget = () => ({ id: current.id, generation: targetGeneration });
	const alive = (target: ReturnType<typeof captureTarget>) => !disposed && target.id === data.manga.id && target.id === current.id && target.generation === targetGeneration;
	onMount(() => {
		const reconcile = (event: Event) => { if ((event as CustomEvent).detail?.id === current.id) { uncertain = true; void refresh(); } };
		window.addEventListener("weebdex-title-publication-changed", reconcile);
		return () => window.removeEventListener("weebdex-title-publication-changed", reconcile);
	});
	const prepared = $derived(current.publication_status === "prepared_original_command");
	const full = $derived(available && !uncertain && current.can_mutate !== false && !!current.relationships && typeof current.language === "string");
	const canRetry = $derived(available && !uncertain && prepared && current.can_retry_publication === true && Number.isInteger(current.publication_retry_version) && (current.publication_retry_version ?? 0) > 0);
	async function retry() {
		if (busy || !canRetry || !confirm("Retry this exact saved original publication command? No edited title or new command will be submitted.")) return;
		const target = captureTarget(), version = current.publication_retry_version!;
		busy = true; failure = undefined;
		try {
			const result = await API.Manga.Publish(target.id, version);
			if (!alive(target)) return;
			if (result?.status === 200 && result.body) { current = result.body as Receipt; uncertain = false; }
			else failure = new Constants.Error.Error("Saved publication not confirmed", "The original command was not acknowledged. Saved state is retained; refresh current authority/status before any further retry.");
		} catch (error) { if (!alive(target)) return; failure = error instanceof Constants.Error.Error ? error : new Constants.Error.Error("Publication outcome unknown", "The saved command may have been accepted. Use read-only refresh to reconcile; no automatic replay occurred."); }
		finally { if (alive(target)) busy = false; }
	}
	async function refresh() {
		if (busy || disposed) return;
		const target = captureTarget();
		let ownedRead: ReturnType<typeof editorCoverReader.capture>;
		try { ownedRead = editorCoverReader.capture(); }
		catch (cause) { failure = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; return; }
		busy = true; failure = undefined;
		try {
			const next = await editorCoverReader.currentTitle(target.id, ownedRead); if (!alive(target)) return;
			let nextCovers: Cover[] = [];
			if (next.relationships && typeof next.language === "string") {
				nextCovers = await editorCoverReader.covers(next, ownedRead); if (!alive(target)) return;
			}
			ownedRead.assert(); current = next; covers = nextCovers; uncertain = false;
		} catch (error) {
			if (!alive(target)) return;
			if (error instanceof Constants.Error.Error && "code" in error && [403, 404].includes(Number(error.code))) available = false;
			failure = error instanceof Constants.Error.Error ? error : new Constants.Error.Error("Receipt refresh failed", "No publication or cover state was confirmed. Try this read-only refresh again; no mutation was replayed.");
		} finally { ownedRead.release(); if (alive(target)) busy = false; }
	}

</script>
{#if !available}
	{#if failure}<ErrorComponent error={failure} />{/if}
{:else if full}
	{#key current.id}
		<RetainedMangaEdit data={{ manga: current, covers }} />
	{/key}
{:else}
	<Card.Root>
		<Card.Header><Card.Title>{uncertain ? "Publication state needs confirmation" : prepared ? "Original publication saved" : "Signed title accepted"}</Card.Title><Card.Description>{uncertain ? "The last publication attempt may have changed durable state. Editing remains blocked until a fresh authoritative read confirms it. No automatic publication replay occurs." : prepared ? "An immutable original publication command is saved. Acceptance is not yet confirmed. Editing and cancellation are fenced; only an authorized retry of that exact command is available." : "The current private receipt is accepted but is waiting for local catalogue processing. Public title detail is not available yet."}</Card.Description></Card.Header>
		<Card.Content class="space-y-2">{#if !uncertain}<p>{current.title}</p>{/if}<p>Instance title {current.id} · Version {current.version}</p>{#if !uncertain && current.protocol_event_id}<p class="break-all">Original signed event: {current.protocol_event_id}</p>{/if}{#if failure}<ErrorComponent error={failure} />{/if}</Card.Content>
		<Card.Footer><Button variant="outline" disabled={busy} onclick={refresh}>{busy ? "Refreshing…" : "Refresh publication receipt"}</Button>{#if canRetry}<Button disabled={busy} onclick={retry}>Retry saved original publication</Button>{/if}</Card.Footer>
	</Card.Root>
{/if}
