<script>
	import Skeleton from "./ui/skeleton/skeleton.svelte";
</script>

<div class="mt-2 space-y-1.5 pb-5">
	<div class="flex items-center gap-1.5">
		<Skeleton class="bg-card h-4.5 w-4.5 shrink-0" />
		<Skeleton class="bg-card h-4.5 w-0 flex-auto" />
	</div>
	<Skeleton class="bg-card h-3.75 w-3/4" />
	<Skeleton class="bg-card h-3.75 w-1/2" />
</div>
