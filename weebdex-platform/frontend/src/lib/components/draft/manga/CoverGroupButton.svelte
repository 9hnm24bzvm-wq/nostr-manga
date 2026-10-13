<script lang="ts">
	import { cn } from "$lib/Utils";
	import { Button, type ButtonProps } from "$lib/components/ui/button";

	let {
		class: className,
		covers = $bindable(),
		language,
		children,
		...restProps
	}: {
		covers: CoverDraft[];
		language: string;
	} & ButtonProps = $props();

	const mimeTypes = ["image/jpeg", "image/jpg", "image/png", "image/gif"];
	let ref = $state<HTMLInputElement>();

	const processFiles = async (files: File[]) => {
		files = files.filter(file => file.type.startsWith("image/"));
		if (!files.length) return;

		files.sort((a, b) =>
			a.name.localeCompare(b.name, undefined, {
				numeric: true,
				sensitivity: "base"
			})
		);

		for (const file of files) {
			const blob = URL.createObjectURL(file);
			covers.push({
				id: file.name + Date.now(),
				language: language,
				main: !covers.length,
				valid: true,

				file: file,
				url: blob,
				thumbnail: blob
			});
		}
	};

	const onInputChange = async (ev: Event) => {
		const target = ev.target as HTMLInputElement;
		if (!target.files?.length) return;

		try {
			await processFiles(Array.from(target.files));
		} finally {
			target.value = "";
		}
	};
</script>

<svelte:window
	ondragover={e => {
		e.preventDefault();
	}}
	ondrop={e => {
		e.preventDefault();
		if (restProps.disabled) return;
		if (e.dataTransfer && e.dataTransfer.files) {
			const files = Array.from(e.dataTransfer.files).filter(file => mimeTypes.includes(file.type));
			processFiles(files);
		}
	}}
/>

<input class="hidden" type="file" multiple accept={mimeTypes.join(",")} onchange={onInputChange} bind:this={ref} />
<Button
	class={cn("w-full border border-dashed !bg-transparent", className)}
	variant="ghost"
	onclick={() => ref?.click()}
	{...restProps}
>
	{@render children?.()}
</Button>
