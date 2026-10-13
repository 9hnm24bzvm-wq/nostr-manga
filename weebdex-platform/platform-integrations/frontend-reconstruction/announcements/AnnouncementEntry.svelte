<script lang="ts">
	import * as Card from "$lib/components/ui/card";
	import { parseDescription } from "$lib/Utils";
	import type { Announcement } from "./announcement-client";
	let { announcement, detail = false }: { announcement: Announcement; detail?: boolean } = $props();
</script>
<Card.Root>
	<Card.Header>
		<Card.Title><a href={`/announcements/${announcement.id}`}>{announcement.title}</a></Card.Title>
		<Card.Description>
			{announcement.relationships.author?.name ?? "Staff"}
			{#if announcement.published_at} · {new Date(announcement.published_at).toLocaleDateString()}{/if}
			{#if announcement.pinned} · Pinned{/if}
			{#if !announcement.published} · Draft{/if}
		</Card.Description>
	</Card.Header>
	<Card.Content><div class="typography">{@html parseDescription(announcement.body)}</div></Card.Content>
	{#if detail}<Card.Footer><a href="/announcements" class="text-sm underline">All announcements</a></Card.Footer>{/if}
</Card.Root>
