<script lang="ts" module>
	const maxSessionSize = 500 * 1024 * 1024;
	const maxFileSize = 25 * 1024 * 1024;
	const maxImageWidth = 10000;
	const maxImageHeight = 10000;
	const maxChunkSize = 50 * 1024 * 1024;
	const maxChunkFiles = 5;

	const imageMimeTypes = ["image/jpeg", "image/jpg", "image/png", "image/gif"];
	const archiveMimeTypes = [
		"application/zip",
		"application/x-zip-compressed",
		"application/vnd.comicbook+zip",
		"application/x-cbz"
	];

	const extractArchive = async (file: File) => {
		const files: File[] = [];
		try {
			const zip = await JSZip.loadAsync(file);
			for (const e of Object.values(zip.files)
				.filter(e => !e.dir && /\.(jpg|jpeg|png|gif)$/i.test(e.name))
				.sort((a, b) =>
					a.name.localeCompare(b.name, undefined, {
						numeric: true,
						sensitivity: "base"
					})
				)) {
				const blob = await e.async("blob");
				if (blob.size > maxFileSize) {
					toast.error(e.name, { description: "File too large" });
					continue;
				}
				files.push(new File([blob], e.name));
			}
		} catch (e) {
			if (e instanceof Constants.Error.Error) throw e;
			else new Constants.Error.Error(file.name, "Failed to extract archive");
		}

		return files;
	};

	const splitImage = async (image: HTMLImageElement, name: string) => {
		if (image.width > image.height) {
			const slices = Math.ceil(image.width / maxImageWidth);
			const sliceWidth = Math.ceil(image.width / slices);

			const images = [];
			for (let i = 0; i < slices; i++) {
				const x = sliceWidth * i;
				const remainingWidth = image.width - x;

				const canvas = document.createElement("canvas");
				canvas.width = Math.min(sliceWidth, remainingWidth);
				canvas.height = image.height;

				const ctx = canvas.getContext("2d")!;
				ctx.drawImage(
					image,
					x,
					0,
					Math.min(sliceWidth, remainingWidth),
					image.height,
					0,
					0,
					Math.min(sliceWidth, remainingWidth),
					image.height
				);

				const blob = await new Promise<Blob | null>(resolve => canvas.toBlob(resolve, "image/png", 100));
				if (blob) images.push(new File([blob], name + "_" + i, { type: "image/png" }));
			}

			return images;
		}

		const slices = Math.ceil(image.height / maxImageHeight);
		const sliceHeight = Math.ceil(image.height / slices);

		const images = [];
		for (let i = 0; i < slices; i++) {
			const y = sliceHeight * i;
			const remainingHeight = image.height - y;

			const canvas = document.createElement("canvas");
			canvas.width = image.width;
			canvas.height = Math.min(sliceHeight, remainingHeight);

			const ctx = canvas.getContext("2d")!;
			ctx.drawImage(
				image,
				0,
				y,
				image.width,
				Math.min(sliceHeight, remainingHeight),
				0,
				0,
				image.width,
				Math.min(sliceHeight, remainingHeight)
			);

			const blob = await new Promise<Blob | null>(resolve => canvas.toBlob(resolve, "image/png", 100));
			if (blob) images.push(new File([blob], name + "_" + i, { type: "image/png" }));
		}
		return images;
	};

	const breakpoints: Record<number, number> = {
		0: 5,
		960: 4,
		770: 3,
		580: 2
	};
	const calcColumns = () => {
		for (const k in breakpoints) {
			const w = Number(k);
			const v = breakpoints[w];

			if (window.innerWidth <= w) {
				return v;
			}
		}
		return breakpoints[0];
	};
</script>

<script lang="ts">
	import { PUBLIC_API_URL } from "$env/static/public";
	import { API, Constants, HTTP } from "$lib";
	import AspectRatio from "$lib/components/ui/aspect-ratio/aspect-ratio.svelte";
	import { Button } from "$lib/components/ui/button";
	import Label from "$lib/components/ui/label/label.svelte";
	import { Progress } from "$lib/components/ui/progress/index.js";
	import { Spinner } from "$lib/components/ui/spinner";
	import { getMutex, getUploadSession } from "$lib/Context";
	import { PlusIcon } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import JSZip from "jszip";
	import { onDestroy } from "svelte";
	import { dndzone, SHADOW_ITEM_MARKER_PROPERTY_NAME, type DndEvent } from "svelte-dnd-action";
	import { toast } from "svelte-sonner";
	import { flip } from "svelte/animate";
	import { fade } from "svelte/transition";
	import UploadChapterPagesEntry from "./UploadChapterPagesEntry.svelte";

	let {
		pages = $bindable(),
		disabled
	}: {
		pages: UploadSessionFile[];
		disabled?: boolean;
	} = $props();

	const uploadSession = $derived(getUploadSession()());
	const mutex = getMutex();

	let ref = $state<HTMLInputElement>();
	let columns = $derived<number>(calcColumns());
	let progress = $state({ total: 0, loaded: 0 });
	let progressPercentage = $derived(progress.total > 0 ? Math.round((progress.loaded / progress.total) * 100) : 0);
	let isDragDisabled = $state(true);

	onDestroy(() => {
		pages.forEach(revokeThumbnail);
	});

	const createPage = (file: File) => {
		return <UploadSessionFile>{
			id: crypto.randomUUID(),
			original_file_name: file.name,

			file: file,
			thumbnail: URL.createObjectURL(file)
		};
	};

	const revokeThumbnail = (page: UploadSessionFile) => {
		if (page.thumbnail && page.thumbnail.startsWith("blob:")) {
			URL.revokeObjectURL(page.thumbnail);
		}
	};

	const uploadChunk = async (chunk: UploadSessionFile[], onProgress?: (delta: number) => void) => {
		if (!uploadSession) {
			return;
		}

		const form = new FormData();
		chunk.forEach(e => form.append("file", e.file!));

		const http = new HTTP<UploadSessionFile[]>(`${PUBLIC_API_URL}/upload/${uploadSession.id}`, form);
		http.setWithCredentials(true);

		if (onProgress) {
			let lastLoaded = 0;
			http.addUploadListener("progress", e => {
				if (e.lengthComputable) {
					const delta = e.loaded - lastLoaded;
					lastLoaded = e.loaded;
					onProgress(delta);
				}
			});
		}

		const res = await http.POST();
		if (res?.status === StatusCodes.OK && res.body) {
			res.body.forEach((remote, i) => {
				const idx = pages.findIndex(e => e.id === chunk[i].id);
				if (idx >= 0) {
					const local = pages[idx];
					remote.id = remote.id ?? local.id;
					remote.file = remote.error ? local.file : undefined;
					remote.thumbnail = local.thumbnail;
					pages[idx] = remote;
				}
			});
			pages = pages;
		}
	};

	let targetFile = $state<UploadSessionFile>();
	const replaceFile = async (file: File) => {
		if (!targetFile || !uploadSession) return;

		const idx = pages.indexOf(targetFile);
		if (idx < 0) return;

		const newFile = createPage(file);
		if (!targetFile.file && !targetFile.error) {
			const res = await API.UploadSession.DeleteFile(uploadSession.id, targetFile.id);
			if (res?.status === StatusCodes.NO_CONTENT) {
			} else if (res?.status === StatusCodes.FORBIDDEN) {
				throw Constants.Error.Forbidden;
			} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
				throw Constants.Error.TooManyRequests;
			} else throw Constants.Error.Unknown;
		}

		revokeThumbnail(targetFile);
		pages[idx] = newFile;
		progress.total += newFile.file!.size;

		await uploadChunk([newFile], delta => {
			progress.loaded += delta;
		});

		if (progress.loaded >= progress.total) {
			setTimeout(() => {
				progress = { total: 0, loaded: 0 };
			}, 1000);
		}
	};

	const processFiles = async (files: File[]) => {
		const processedFiles = (
			await Promise.all(
				files.map(async file => {
					if (imageMimeTypes.includes(file.type)) {
						if (file.size > maxFileSize) {
							toast.error(file.name, { description: "File too large" });
							return undefined;
						}

						const image = await new Promise<HTMLImageElement>(resolve => {
							const image = new Image();
							image.onload = () => resolve(image);
							image.src = URL.createObjectURL(file);
						});
						return image.width > maxImageWidth || image.height > maxImageHeight ? splitImage(image, file.name) : file;
					} else if (archiveMimeTypes.includes(file.type) || file.name.endsWith(".cbz")) {
						return extractArchive(file);
					}
				})
			)
		)
			.flat()
			.filter(e => e instanceof File);
		if (!processedFiles.length) return;

		const totalSize = processedFiles.reduce<number>((acc, file) => acc + file.size, 0);
		const sessionSize = pages.reduce<number>((acc, file) => acc + Number(file.file_size), 0);

		if (totalSize + sessionSize >= maxSessionSize) {
			toast.error("Maximum size for this upload session exceeded.");
			return;
		}
		progress.total += totalSize;

		processedFiles.sort((a, b) =>
			a.name.localeCompare(b.name, undefined, {
				numeric: true,
				sensitivity: "base"
			})
		);

		const newFiles = processedFiles.map(createPage);
		pages.push(...newFiles);

		while (newFiles.length) {
			const chunk: UploadSessionFile[] = [];
			for (
				let j = 0;
				j < maxChunkFiles && (chunk.length ? chunk.reduce((acc, file) => acc + file.file!.size, 0) : 0) < maxChunkSize;
				j++
			) {
				const file = newFiles.shift();
				if (!file) break;
				chunk.push(file);
			}
			await uploadChunk(chunk, delta => {
				progress.loaded += delta;
			});
		}

		if (progress.loaded >= progress.total) {
			setTimeout(() => {
				progress = { total: 0, loaded: 0 };
			}, 1000);
		}
	};

	const scanFiles = async (entry?: FileSystemEntry | null): Promise<File[]> => {
		if (entry?.isFile) {
			return new Promise<File[]>(resolve => (<FileSystemFileEntry>entry).file(file => resolve([file])));
		} else if (entry?.isDirectory) {
			const reader = (entry as FileSystemDirectoryEntry).createReader();
			const entries = await new Promise<FileSystemEntry[]>((resolve, reject) => reader.readEntries(resolve, reject));
			return (await Promise.all(entries.map(scanFiles))).flat();
		}
		return [];
	};

	const onInputChange = (ev: Event) =>
		mutex.exec(async () => {
			const target = ev.target as HTMLInputElement;
			try {
				if (target.files?.length) {
					if (targetFile) await replaceFile(target.files[0]);
					else await processFiles(Array.from(target.files));
				}
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			} finally {
				targetFile = undefined;
				target.value = "";
			}
		});

	const onDrop = (ev: DragEvent) => {
		ev.preventDefault();
		return mutex.exec(async () => {
			try {
				if (ev.dataTransfer?.items.length) {
					const files = await Promise.all(Array.from(ev.dataTransfer.items).map(e => scanFiles(e.webkitGetAsEntry())));
					await processFiles(files.flat().filter(e => e instanceof File));
				} else if (ev.dataTransfer?.files.length) {
					await processFiles(Array.from(ev.dataTransfer.files));
				}
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			}
		});
	};

	let isDeletingPages = $state(false);
	const deletePages = () =>
		mutex.exec(async () => {
			if (!uploadSession) {
				return;
			}

			isDeletingPages = true;
			try {
				const ids = pages.filter(e => !e.file && !e.error).map(e => e.id);
				if (ids.length) {
					const res = await API.UploadSession.DeleteFiles(uploadSession.id, ids);
					if (res?.status === StatusCodes.NO_CONTENT) {
					} else if (res?.status === StatusCodes.FORBIDDEN) {
						throw Constants.Error.Forbidden;
					} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
						throw Constants.Error.TooManyRequests;
					} else throw Constants.Error.Unknown;
				}

				pages.forEach(revokeThumbnail);
				pages = [];
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			} finally {
				isDeletingPages = false;
			}
		});

	let isDeletingDuplicate = $state(false);
	const deleteDuplicate = () =>
		mutex.exec(async () => {
			if (!uploadSession) {
				return;
			}

			isDeletingDuplicate = true;
			try {
				const unique = new Map<string, UploadSessionFile>();
				const duplicateIds: string[] = [];

				pages.forEach(e => {
					const key = e.file_hash ?? e.original_file_name!;
					if (unique.has(key)) {
						if (!e.file && !e.error) {
							duplicateIds.push(e.id);
						}
						revokeThumbnail(e);
					} else {
						unique.set(key, e);
					}
				});

				if (duplicateIds.length) {
					const res = await API.UploadSession.DeleteFiles(uploadSession.id, duplicateIds);
					if (res?.status === StatusCodes.NO_CONTENT) {
					} else if (res?.status === StatusCodes.FORBIDDEN) {
						throw Constants.Error.Forbidden;
					} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
						throw Constants.Error.TooManyRequests;
					} else throw Constants.Error.Unknown;
				}

				pages = Array.from(unique.values());
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else {
					console.error(e);
					toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
				}
			} finally {
				isDeletingDuplicate = false;
			}
		});

	const handleDragAndDropEvent = (e: CustomEvent<DndEvent<UploadSessionFile>>) => {
		pages = e.detail.items;
		isDragDisabled = true;
	};

	const createDragToggleHandler = (shouldEnable: boolean) => () => (isDragDisabled = !shouldEnable);
</script>

<svelte:window
	onresize={() => (columns = calcColumns())}
	ondrop={onDrop}
	ondragover={ev => ev.preventDefault()}
	ondragleave={ev => ev.preventDefault()}
/>

<input class="hidden" type="file" multiple accept={imageMimeTypes.join(",")} onchange={onInputChange} bind:this={ref} />
{#if progressPercentage > 0}
	<div class="bg-background sticky top-15 z-10 -mx-4 px-4 pb-2" transition:fade={{ duration: 200 }}>
		<Progress value={progressPercentage} />
	</div>
{/if}

<div class="space-y-4">
	<Label>
		Pages
		{#if pages.length}
			({pages.length})
		{/if}
	</Label>
	<div
		class="grid grid-cols-[repeat(var(--columns),minmax(0,1fr))] gap-8 select-none"
		use:dndzone={{
			items: pages,
			flipDurationMs: 100,
			dragDisabled: isDragDisabled,
			dropTargetStyle: { outline: "none" }
		}}
		onconsider={handleDragAndDropEvent}
		onfinalize={handleDragAndDropEvent}
		style="--columns: {columns}"
	>
		{#each pages as page, i (page)}
			{@const isDragging = (page as any)[SHADOW_ITEM_MARKER_PROPERTY_NAME]}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="!visible relative !border-none !ring-0"
				class:dragging={isDragging}
				animate:flip={{ duration: 100 }}
				onmouseenter={createDragToggleHandler(!$mutex)}
				onmouseleave={createDragToggleHandler(false)}
				onmousedown={createDragToggleHandler(false)}
				onmouseup={createDragToggleHandler(!$mutex)}
				ontouchstart={createDragToggleHandler(false)}
				ontouchend={createDragToggleHandler(!$mutex)}
			>
				<UploadChapterPagesEntry
					bind:page={pages[i]}
					bind:pages
					bind:targetFile
					inputRef={ref}
					{isDragging}
					disabled={$mutex || disabled}
				/>
			</div>
		{/each}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div>
			<AspectRatio ratio={10 / 14}>
				<Button
					class="size-full rounded-md border border-dashed !bg-transparent"
					variant="outline"
					onclick={() => ref?.click()}
					disabled={$mutex || disabled}
				>
					<PlusIcon class="size-6" />
				</Button>
			</AspectRatio>
		</div>
	</div>
	{#if pages.length > 1}
		<div class="flex flex-wrap items-center gap-2">
			<Button
				class="border-none"
				variant="outline"
				type="button"
				onclick={() =>
					pages.sort((a, b) =>
						(a.original_file_name ?? a.file_name).localeCompare(b.original_file_name ?? b.file_name, undefined, {
							numeric: true,
							sensitivity: "base"
						})
					)}
				disabled={$mutex || disabled}
			>
				Quick Sort
			</Button>
			<Button
				class="w-40 border-none"
				variant="outline"
				type="button"
				onclick={deletePages}
				disabled={$mutex || disabled}
			>
				{#if $mutex && isDeletingPages}
					<Spinner />
				{:else}
					Remove all pages
				{/if}
			</Button>
			<Button
				class="w-40 border-none"
				variant="outline"
				type="button"
				onclick={deleteDuplicate}
				disabled={$mutex || disabled}
			>
				{#if $mutex && isDeletingDuplicate}
					<Spinner />
				{:else}
					Remove duplicates
				{/if}
			</Button>
		</div>
	{/if}
</div>

<style>
	.dragging {
		animation: bounce 0.425s infinite;
	}

	@keyframes bounce {
		0%,
		100% {
			transform: rotate(-2deg);
			animation-timing-function: linear;
		}
		50% {
			transform: rotate(2deg);
			animation-timing-function: linear;
		}
	}
</style>
