<script lang="ts">
	import AppHeader from "$lib/components/AppHeader.svelte";
	import AppSidebar from "$lib/components/AppSidebar.svelte";
	import * as Sidebar from "$lib/components/ui/sidebar";

	const { children } = $props();
</script>

<Sidebar.Outer>
	<AppSidebar />
	<div class="flex w-0 flex-auto flex-col">
		<AppHeader />
		<main class="m-auto flex w-full max-w-400 flex-auto flex-col p-3">
			{@render children()}
		</main>
	</div>
</Sidebar.Outer>
