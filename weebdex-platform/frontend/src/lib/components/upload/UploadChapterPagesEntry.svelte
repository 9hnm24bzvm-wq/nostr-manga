<script lang="ts">
	import { Constants, Mutex } from "$lib";
	import API from "$lib/API";
	import { AspectRatio } from "$lib/components/ui/aspect-ratio";
	import { Button } from "$lib/components/ui/button";
	import * as Empty from "$lib/components/ui/empty";
	import ImageLoader from "$lib/components/ui/ImageLoader.svelte";
	import { Spinner } from "$lib/components/ui/spinner";
	import * as Tooltip from "$lib/components/ui/tooltip";
	import { getUploadSession } from "$lib/Context";
	import { Move, PencilIcon, TriangleAlert, XIcon } from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { toast } from "svelte-sonner";

	let {
		page = $bindable(),
		pages = $bindable(),

		targetFile = $bindable(),
		inputRef,

		isDragging,
		disabled
	}: {
		page: UploadSessionFile;
		pages: UploadSessionFile[];
		isDragging: boolean;

		targetFile: UploadSessionFile | undefined;
		inputRef: HTMLInputElement;

		disabled?: boolean;
	} = $props();

	const uploadSession = $derived(getUploadSession()());
	const mutex = new Mutex();

	const replacePage = () => {
		targetFile = page;
		inputRef.click();
	};

	const deletePage = (page: UploadSessionFile) =>
		mutex.exec(async () => {
			if (!uploadSession) {
				return;
			}

			try {
				if (!page.file) {
					const res = await API.UploadSession.DeleteFile(uploadSession.id, page.id);
					if (res?.status === StatusCodes.NO_CONTENT) {
					} else if (res?.status === StatusCodes.FORBIDDEN) {
						throw Constants.Error.Forbidden;
					} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
						throw Constants.Error.TooManyRequests;
					} else throw Constants.Error.Unknown;
				}

				const idx = pages.indexOf(page);
				if (idx !== -1) {
					if (page.thumbnail && page.thumbnail.startsWith("blob:")) {
						URL.revokeObjectURL(page.thumbnail);
					}
					pages.splice(idx, 1);
				}
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
			}
		});
</script>

<AspectRatio ratio={10 / 14} class="group bg-card overflow-hidden rounded-md">
	{#if !isDragging}
		<ImageLoader src={page.thumbnail!}>
			{#snippet child({ src })}
				<figure class="flex size-full items-center justify-center">
					<img class="size-full object-contain object-center" {src} alt={page.original_file_name} />
					<div class="absolute inset-0 flex items-center justify-center">
						{#if page.error}
							<Tooltip.Provider delayDuration={0}>
								<Tooltip.Root>
									<Tooltip.Trigger>
										<div class="rounded-full bg-red-500/50 p-2 text-red-500/75">
											<TriangleAlert class="size-8" />
										</div>
									</Tooltip.Trigger>
									<Tooltip.Content>
										<span class="font-medium">{page.error}</span>
									</Tooltip.Content>
								</Tooltip.Root>
							</Tooltip.Provider>
						{:else if !page.file_name}
							<div class="rounded-full bg-black/25 text-white/85">
								<Spinner class="size-10" />
							</div>
						{:else}
							<div class="rounded-full bg-black/25 p-1 text-white/75 group-hover:bg-black/40 group-hover:text-white/85">
								<Move class="size-6" />
							</div>
						{/if}
					</div>
					<div class="gradient pointer-events-none absolute inset-0"></div>
				</figure>
				<div class="pointer-events-none absolute inset-0 flex h-full flex-col pb-2">
					<div class="flex flex-auto"></div>
					<h3 class="line-clamp-1 shrink-0 px-2 text-xs text-balance break-words text-white group-hover:line-clamp-4">
						{page.original_file_name}
					</h3>
				</div>
			{/snippet}
			{#snippet loader()}
				<Empty.Root class="size-full">
					<Spinner class="size-10" />
				</Empty.Root>
			{/snippet}
		</ImageLoader>
	{/if}
</AspectRatio>

{#if !isDragging && (!page.file || page.error)}
	<Button
		class="bg-input text-foreground hover:bg-destructive hover:text-destructive-foreground absolute top-0 left-0 -mt-2 -ml-2 size-5.5 rounded-full"
		size="icon-sm"
		type="button"
		onclick={replacePage}
		disabled={$mutex || disabled}
	>
		<PencilIcon class="size-3" strokeWidth="3" />
	</Button>
	<Button
		class="bg-input text-foreground hover:bg-destructive hover:text-destructive-foreground absolute top-0 right-0 -mt-2 -mr-2 size-5.5 rounded-full"
		size="icon-sm"
		type="button"
		onclick={() => deletePage(page)}
		disabled={$mutex || disabled}
	>
		<XIcon class="size-3.5" strokeWidth="3" />
	</Button>
{/if}

<style lang="scss">
	.gradient {
		background-image: linear-gradient(180deg, transparent 50%, rgba(0, 0, 0, 0.5));
	}
	h3 {
		text-shadow:
			0 0 0.2rem rgba(0, 0, 0, 0.75),
			0 0 0.15rem rgba(0, 0, 0, 0.75),
			0 0 0.1rem rgba(0, 0, 0, 0.75);
	}
</style>
