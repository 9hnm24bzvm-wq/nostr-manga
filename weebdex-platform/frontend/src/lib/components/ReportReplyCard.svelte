<script lang="ts">
	import { Typography, Utils } from "$lib";
	import { ScrollArea } from "./ui/scroll-area";

	const { reply }: { reply: Reply } = $props();
</script>

<article class="bg-input/30 rounded-md px-3 py-4">
	<ScrollArea>
		<div class="typography comment max-h-20" {@attach Typography.exec}>
			{@html Utils.parseComment(reply.body!)}
		</div>
	</ScrollArea>
</article>
