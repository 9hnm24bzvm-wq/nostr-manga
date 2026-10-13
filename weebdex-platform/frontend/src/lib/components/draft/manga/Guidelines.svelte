<script lang="ts">
	export const Guidelines = {
		doNot: [
			"Western comics (e.g., Marvel, DC, etc.).",
			"Comics you made yourself.",
			"Duplicates of an already existing title (Search for it first)."
		],
		general: [
			"At least one cover must be in the language of origin of the title.",
			"If your role allows you to choose the main cover, use the cover of the latest volume, unless it either does not fit the overall art style/theme of the title or contains major spoilers.",
			"Duplicate entries are only allowed for officially colored and fan-colored versions of a series.",
			"Attempting to create troll or spam entries will result in permissions being revoked.",
			"Remember to submit your entry once you are finished."
		],
		cover: [
			"Allowed image formats: JPEG, PNG, GIF.",
			"A single image may be at most 10,000 x 10,000 pixels.",
			"The maximum size per image file is 20 MB.",
			"Portrait-oriented covers are preferred whenever possible."
		]
	};
</script>

<div class="typography space-y-2">
	<div>
		<strong class="text-red-400">Do not create entries for:</strong>
		<ul>
			{#each Guidelines.doNot as item}
				<li>{item}</li>
			{/each}
		</ul>
	</div>
	<div>
		<strong>General:</strong>
		<ul>
			{#each Guidelines.general as item}
				<li>{item}</li>
			{/each}
		</ul>
	</div>
	<div>
		<strong>Cover file(s) requirements and limits:</strong>
		<ul>
			{#each Guidelines.cover as item}
				<li>{item}</li>
			{/each}
		</ul>
	</div>
</div>
