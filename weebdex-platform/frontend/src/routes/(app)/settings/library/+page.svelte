<script lang="ts">
	import { PUBLIC_API_URL, PUBLIC_SITE_NAME } from "$env/static/public";
	import { API, Constants, Mutex } from "$lib";
	import { Button } from "$lib/components/ui/button";
	import { Separator } from "$lib/components/ui/separator";
	import { ReadingStatus } from "$lib/constants/Manga";
	import Papa from "papaparse";
	import { toast } from "svelte-sonner";

	const mutex = new Mutex();
	let importRef = $state<HTMLInputElement>();
	let importType = $state<string>();

	let isImporting = $state(false);
	let isExporting = $state(false);

	// csv format: weebdex_id, title, reading_status
	const importWeebDex = async (file: File) => {
		const csvString = await new Promise<string>((resolve, reject) => {
			const reader = new FileReader();
			reader.onload = e => {
				const result = e.target?.result;
				if (typeof result === "string") resolve(result);
				else reject(new Constants.Error.Error("Failed to parse list", "The file is empty"));
			};
			reader.onerror = () => reject(new Constants.Error.Error("Error", "Failed to read file"));
			reader.readAsText(file);
		});
		const entries = await new Promise<{ id: string; status: Constants.Manga.ReadingStatus }[]>(resolve => {
			Papa.parse(csvString, {
				complete: results => {
					resolve(
						results.data
							.slice(1)
							.map(([id, , status]: any) => ({ id, status }))
							.filter(e => e.id.length === 10 && e.status in Constants.Manga.ReadingStatusText)
					);
				}
			});
		});
		if (!entries.length) {
			throw new Constants.Error.Error("Failed to import list", "No entries found");
		}

		const chunks: Record<string, ReadingStatus>[] = [];
		for (let i = 0; i < entries.length; i += 400) {
			const chunk = entries.slice(i, i + 400).reduce<Record<string, ReadingStatus>>((acc, e) => {
				if (!e.id || !e.status) {
					return acc;
				}

				acc[e.id] = e.status;
				return acc;
			}, {});
			chunks.push(chunk);
		}

		for (const chunk of chunks) {
			await API.MangaFollow.Import({ weebdex: chunk });
		}
		toast.success("Library imported successfully.");
	};

	const importMAL = async (file: File) => {
		const xmlString = await new Promise<string>((resolve, reject) => {
			const reader = new FileReader();
			reader.onload = e => {
				const result = e.target?.result;
				if (typeof result === "string") resolve(result);
				else reject(new Constants.Error.Error("Failed to parse list", "The file is empty"));
			};
			reader.onerror = () => reject(new Constants.Error.Error("Error", "Failed to read file"));
			reader.readAsText(file);
		});

		const xmlDoc = new DOMParser().parseFromString(xmlString, "application/xml");
		const entries = Array.from(xmlDoc.querySelectorAll("manga"))
			.map(m => ({
				id: m.querySelector("manga_mangadb_id")?.textContent,
				status: m.querySelector("my_status")?.textContent
			}))
			.filter(e => e.id && e.status);
		if (!entries.length) {
			throw new Constants.Error.Error("Failed to import list", "No entries found");
		}

		const chunks: Record<string, ReadingStatus>[] = [];
		for (let i = 0; i < entries.length; i += 400) {
			const chunk = entries.slice(i, i + 400).reduce<Record<string, ReadingStatus>>((acc, e) => {
				if (!e.id || !e.status) {
					return acc;
				}

				switch (e.status) {
					case "Completed":
						acc[e.id] = ReadingStatus.Completed;
						break;
					case "On-Hold":
						acc[e.id] = ReadingStatus.OnHold;
						break;
					case "Dropped":
						acc[e.id] = ReadingStatus.Dropped;
						break;
					case "Plan to Read":
						acc[e.id] = ReadingStatus.PlanToRead;
						break;
					default:
						acc[e.id] = ReadingStatus.Reading;
						break;
				}
				return acc;
			}, {});
			chunks.push(chunk);
		}

		for (const chunk of chunks) {
			await API.MangaFollow.Import({ myanimelist: chunk });
		}
		toast.success("Library imported successfully.");
	};

	const onImportChange = async () =>
		mutex.exec(async () => {
			try {
				const file = importRef?.files?.[0];
				if (!file) {
					return;
				}

				switch (importType) {
					case "weebdex":
						await importWeebDex(file);
						break;
					case "mal":
						await importMAL(file);
						break;
				}
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			} finally {
				if (importRef) {
					importRef.value = "";
				}
				isImporting = false;
			}
		});

	const onExport = () =>
		mutex.exec(() => {
			isExporting = true;
			try {
				const anchor = document.createElement("a");
				anchor.href = `${PUBLIC_API_URL}/user/follows/manga/export`;
				anchor.download = "true";
				anchor.click();
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			} finally {
				isExporting = false;
			}
		});
</script>

<svelte:head>
	<title>Settings: Library - {PUBLIC_SITE_NAME}</title>
</svelte:head>

<input class="hidden" type="file" bind:this={importRef} onchange={onImportChange} />

<section class="space-y-5">
	<div class="space-y-3">
		<div class="flex-auto space-y-1">
			<strong class="block font-semibold">List Importer</strong>
			<p class="text-foreground/85 max-w-md">Add your library from external services.</p>
		</div>
		<div class="flex shrink-0 flex-wrap items-center gap-1">
			<Button
				type="button"
				onclick={() => {
					if (importRef) {
						importRef.accept = ".csv";
						importRef?.click();
						importType = "weebdex";
					}
				}}
				disabled={$mutex}>Import from WeebDex (.csv)</Button
			>
			<Button
				type="button"
				onclick={() => {
					if (importRef) {
						importRef.accept = ".xml";
						importRef?.click();
						importType = "mal";
					}
				}}
				disabled={$mutex}>Import from MyAnimeList (.xml)</Button
			>
		</div>
	</div>
	<Separator />
	<div class="space-y-3">
		<div class="flex-auto space-y-1">
			<strong class="block font-semibold">List Exporter</strong>
			<p class="text-foreground/85 max-w-md">Download your library for backup or use with other services.</p>
		</div>
		<div class="shrink-0">
			<Button type="button" onclick={onExport} disabled={$mutex}>Generate .csv</Button>
		</div>
	</div>
</section>
