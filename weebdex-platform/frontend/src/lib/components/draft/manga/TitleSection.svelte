<script lang="ts">
	import { Utils } from "$lib";
	import TitleGroup from "$lib/components/draft/manga/TitleGroup.svelte";
	import TitleGroupButton from "$lib/components/draft/manga/TitleGroupButton.svelte";
	import Input from "$lib/components/ui/input/input.svelte";
	import Label from "$lib/components/ui/label/label.svelte";
	import RichTextarea from "$lib/components/ui/RichTextarea.svelte";
	import { Separator } from "$lib/components/ui/separator";
	import { PlusIcon } from "@lucide/svelte";

	let {
		draft = $bindable(),
		disabled
	}: {
		draft: MangaDraft;
		disabled: boolean;
	} = $props();
</script>

<div class="space-y-2">
	<Label class="block" for="title">Title<span class="text-red-500">*</span></Label>
	<div class="space-y-1">
		<Input
			class="truncate border-none !ring-0 !ring-transparent"
			id="title"
			type="text"
			bind:value={draft.title}
			autocomplete="off"
			maxlength={1024}
			required
			{disabled}
		/>
		<p class="text-foreground/85">
			The main title of this entry. For Japanese-origin titles, always use the romanization of the original title.
		</p>
	</div>
</div>
<Separator />
<div class="space-y-2">
	<div class="space-y-1">
		<Label for="alt_titles">
			Alternative Titles
			{#if draft.alt_titles.length}
				({draft.alt_titles.length})
			{/if}
		</Label>
		<p class="text-foreground/85">
			Other titles to add to this entry. This includes translated titles, the title in the original language and script,
			short titles, etc.
		</p>
	</div>
	{#if draft.alt_titles.length > 0}
		<div class="space-y-1">
			<TitleGroup bind:value={draft.alt_titles} {disabled} />
		</div>
	{/if}
	<TitleGroupButton bind:titles={draft.alt_titles} {disabled}>
		<PlusIcon />
		Add Alternative Title
	</TitleGroupButton>
</div>
<Separator />
<div class="space-y-2">
	<div class="space-y-1">
		<Label for="synopsis">Synopsis</Label>
		<p class="text-foreground/85">
			No synopsis is better than incomprehensible MTL. Do not add a bunch of links in there.
		</p>
	</div>
	<div class="space-y-1">
		<RichTextarea
			id="synopsis"
			bind:value={draft.description!}
			maxlength={30000}
			parser={Utils.parseDescription}
			{disabled}
		/>
	</div>
</div>
