<script lang="ts" module>
	let reasons = $state<Record<Constants.EntityType, ReportReason[]>>(<any>{});

	export let applyFilter: (report: Report) => void;
</script>

<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import { Constants } from "$lib";
	import API from "$lib/API";
	import type { SearchReportRequest } from "$lib/API/Report";
	import * as InputGroup from "$lib/components/ui/input-group";
	import * as Select from "$lib/components/ui/select";
	import * as Tooltip from "$lib/components/ui/tooltip";
	import { cn } from "$lib/Utils";
	import { CircleQuestionMark, SearchIcon } from "@lucide/svelte";
	import equal from "fast-deep-equal";
	import { StatusCodes } from "http-status-codes";
	import { onMount } from "svelte";
	import { Button } from "./ui/button";

	let {
		feed = $bindable()
	}: {
		feed: ReportFeedResponse | undefined;
	} = $props();

	const defaultSearch: SearchReportRequest = {
		category: "",
		object: "",
		reason: "",
		status: "",
		order: "desc"
	};
	let search = $state<SearchReportRequest>({
		category: page.url.searchParams.get("category") ?? "",
		object: page.url.searchParams.get("object") ?? "",
		reason: page.url.searchParams.get("reason") ?? "",
		status: page.url.searchParams.get("status") ?? "",
		order: (page.url.searchParams.get("order") as "asc" | "desc") ?? "desc"
	});

	onMount(async () => {
		const values = Object.values(Constants.EntityType);
		if (Object.keys(reasons).length !== values.length) {
			try {
				for (const t of values) {
					const res = await API.Report.Reasons(t);
					if (res?.status === StatusCodes.OK && res.body) {
						reasons[t] = res.body;
					}
				}
			} catch (e) {
				console.error(e);
			}
		}
	});

	const onSubmit = (ev?: SubmitEvent) => {
		ev?.preventDefault();
		ev?.stopPropagation();
		ev?.stopImmediatePropagation();

		const q = new URLSearchParams();
		for (const [key, value] of Object.entries(search)) {
			if (value === undefined || value === null || !value.length) {
				continue;
			}
			q.set(key, value);
		}

		if (search.order === "desc") q.delete("order");

		const target = window.location.pathname + q.toString();
		if (target !== window.location.pathname + window.location.search) {
			feed = undefined;
			goto(target);
		}
	};

	const onReset = (ev?: Event & { currentTarget: HTMLFormElement }) => {
		ev?.preventDefault();
		ev?.stopPropagation();
		ev?.stopImmediatePropagation();
		search = structuredClone(defaultSearch);
		onSubmit();
	};

	applyFilter = (report: Report) => {
		search.category = report.relationships.reason.category;
		const id = (
			report.relationships.user ??
			report.relationships.manga ??
			report.relationships.author ??
			report.relationships.chapter ??
			report.relationships.group ??
			report.relationships.thread ??
			report.relationships.reply
		)?.id;
		if (id) {
			search.object = String(id);
		}
		onSubmit();
	};
</script>

<form onsubmit={onSubmit} onreset={onReset}>
	<div class="flex flex-wrap items-center gap-1">
		<InputGroup.Root
			class={cn("border-none !ring-0 !ring-transparent", {
				"opacity-50": !(search.category! in Constants.EntityTypeText)
			})}
		>
			<InputGroup.Input
				class="truncate"
				placeholder="Search by object ID"
				bind:value={search.object}
				autocomplete="off"
				disabled={!(search.category! in Constants.EntityTypeText)}
			/>
			<InputGroup.Addon>
				<SearchIcon />
			</InputGroup.Addon>
			{#if feed?.total}
				<InputGroup.Addon align="inline-end">{feed.total.short()}</InputGroup.Addon>
			{/if}
			{#if !(search.category! in Constants.EntityTypeText)}
				<InputGroup.Addon align="inline-end">
					<Tooltip.Root>
						<Tooltip.Trigger>
							{#snippet child({ props })}
								<InputGroup.Button {...props} class="rounded-full" size="icon-xs">
									<CircleQuestionMark />
								</InputGroup.Button>
							{/snippet}
						</Tooltip.Trigger>
						<Tooltip.Content>Category is required</Tooltip.Content>
					</Tooltip.Root>
				</InputGroup.Addon>
			{/if}
		</InputGroup.Root>
		<Select.Root type="single" bind:value={search.category} allowDeselect>
			<Select.Trigger class="flex w-0 min-w-60 flex-auto border-none">
				<span class="truncate">
					Category:
					{#if search.category && search.category in Constants.EntityTypeText}
						{Constants.EntityTypeText[search.category]}
					{:else}
						Any
					{/if}
				</span>
			</Select.Trigger>
			<Select.Content>
				{#each Object.entries(Constants.EntityTypeText) as [key, value]}
					<Select.Item value={key}>{value}</Select.Item>
				{/each}
			</Select.Content>
		</Select.Root>
		<Select.Root
			type="single"
			bind:value={search.reason}
			allowDeselect
			disabled={!search.category || !(search.category in reasons)}
		>
			<Select.Trigger class="flex w-0 min-w-60 flex-auto border-none">
				<span class="truncate">
					Reason:
					{#if search.reason && search.category && search.category in reasons}
						{(<Record<string, ReportReason[]>>reasons)[search.category].find(r => r.id === search.reason)?.reason}
					{:else}
						Any
					{/if}
				</span>
			</Select.Trigger>
			<Select.Content>
				{#if search.category && search.category in reasons}
					{#each Object.values((<Record<string, ReportReason[]>>reasons)[search.category]) as reason}
						<Select.Item value={reason.id}>{reason.reason}</Select.Item>
					{/each}
				{/if}
			</Select.Content>
		</Select.Root>
		<Select.Root type="single" bind:value={search.status} allowDeselect>
			<Select.Trigger class="flex w-0 min-w-60 flex-auto border-none">
				<span class="truncate">
					Status:
					{#if search.status}
						{Constants.Report.StatusText[search.status]}
					{:else}
						Any
					{/if}
				</span>
			</Select.Trigger>
			<Select.Content>
				{#each Object.entries(Constants.Report.StatusText) as [key, value]}
					<Select.Item value={key}>{value}</Select.Item>
				{/each}
			</Select.Content>
		</Select.Root>
		<Select.Root type="single" bind:value={search.order}>
			<Select.Trigger class="text-muted-foreground flex w-0 min-w-60 flex-auto border-none">
				<span class="truncate">
					Order:
					{#if search.order === "asc"}
						Ascending
					{:else}
						Descending
					{/if}
				</span>
			</Select.Trigger>
			<Select.Content>
				<Select.Item value="asc">Ascending</Select.Item>
				<Select.Item value="desc">Descending</Select.Item>
			</Select.Content>
		</Select.Root>
	</div>
	<div class="mt-5 flex items-center gap-2">
		<div class="w-0 flex-auto"></div>
		<div class="flex shrink-0 items-center gap-1">
			<Button
				class="!text-destructive border-none"
				variant="outline"
				disabled={equal(search, defaultSearch)}
				type="reset"
			>
				Reset
			</Button>
			<Button variant="default" type="submit">
				<SearchIcon />
				Search
			</Button>
		</div>
	</div>
</form>
