<script lang="ts" module>
	let deferredPrompt = $state<Event>();
	window.addEventListener("beforeinstallprompt", e => {
		e.preventDefault();
		if (
			window.matchMedia("(display-mode: standalone)").matches ||
			window.navigator.standalone === true ||
			/iPad|iPhone|iPod/.test(navigator.userAgent)
		) {
			return;
		}
		deferredPrompt = e;
	});
	window.addEventListener("appinstalled", () => {
		deferredPrompt = undefined;
	});
</script>

<script lang="ts">
	import { page } from "$app/state";
	import SearchDialog from "$lib/components/SearchDialog.svelte";
	import { Button, buttonVariants } from "$lib/components/ui/button";
	import * as Dialog from "$lib/components/ui/dialog";
	import { useSidebar } from "$lib/components/ui/sidebar";
	import { isMobile, isShowSearchDialog } from "$lib/stores";
	import { ArrowDownToLine, Menu, PanelLeftIcon, SearchIcon } from "@lucide/svelte";

	let sidebar = useSidebar();

	$effect(() => {
		if (page.url) {
			$isShowSearchDialog = false;
			if (sidebar.isMobile) {
				sidebar.setOpenMobile(false);
			}
		}
	});
</script>

<svelte:window
	onkeydown={ev => {
		if (ev.target instanceof HTMLInputElement || !ev.ctrlKey || ev.code !== "KeyK") {
			return;
		}

		ev.preventDefault();
		ev.stopPropagation();
		ev.stopImmediatePropagation();
		$isShowSearchDialog = true;
	}}
/>

<header class="bg-background sticky top-0 z-10 h-15 backdrop-blur" id="header">
	<nav class="m-auto flex h-full items-center gap-3 p-3">
		<div class="flex shrink-0 items-center gap-3">
			<Button type="button" variant="ghost" size="icon" onclick={sidebar.toggle}>
				{#if isMobile.current}
					<Menu class="size-6" />
				{:else}
					<PanelLeftIcon class="size-5" />
				{/if}
			</Button>
			<a class="text-xl font-semibold" href="/">WeebDex</a>
		</div>
		<div class="flex flex-auto shrink-0 items-center justify-end gap-1">
			<Dialog.Root bind:open={$isShowSearchDialog}>
				{#if sidebar.isMobile}
					<Dialog.Trigger
						class={buttonVariants({
							variant: "ghost",
							size: "icon"
						})}
					>
						<SearchIcon class="size-5" />
					</Dialog.Trigger>
				{:else}
					<Dialog.Trigger
						class={buttonVariants({
							class: "!text-muted-foreground w-full max-w-125 justify-start border-none text-left",
							variant: "outline",
							size: "default"
						})}
					>
						<SearchIcon />
						<span class="w-0 flex-auto">Search...</span>
						<span class="font-mono text-xs">CTRL K</span>
					</Dialog.Trigger>
				{/if}
				<Dialog.Content
					class="flex max-h-[calc(100dvh-2rem)] !max-w-[min(40rem,calc(100dvw-2rem))] flex-col gap-0 p-0"
					showCloseButton={false}
				>
					<SearchDialog />
				</Dialog.Content>
			</Dialog.Root>
			{#if deferredPrompt}
				<Button
					class="border-none font-medium"
					variant="outline"
					onclick={() => {
						try {
							deferredPrompt?.prompt?.();
						} catch {}
					}}
				>
					<ArrowDownToLine />
					PWA
				</Button>
			{/if}
		</div>
	</nav>
</header>
