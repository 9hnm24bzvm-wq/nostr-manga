<script lang="ts">
	import Mutex from "$lib/Mutex";
	import { tick } from "svelte";
	import { useReader } from "./context.svelte";

	const mutex = new Mutex();
	const processMutex = new Mutex();
	const reader = useReader();
	const readerPrefs = reader.prefs;

	let queue = $state<number[]>([]);
	const updateQueue = async (..._: any) =>
		mutex.exec(async () => {
			await tick();
			if (!reader.pages.length || !reader.currentPage || reader.pages.every(e => e.isLoaded)) {
				return;
			}

			console.info("[Reader] Updating queue");
			const clone = Array.from(queue).filter(e => !reader.pages[e].isLoaded && !reader.pages[e].isFailed);
			for (let i = 1; i < $readerPrefs.preload + 1; i++) {
				if (reader.currentPage.index < reader.pages.length) {
					const next = reader.pages[reader.currentPage.index + i];
					if (next && !next.isLoaded && !next.isFailed && !clone.includes(next.index)) {
						clone.push(next.index);
					}
				}
				if (reader.currentPage.index > 0) {
					const prev = reader.pages[reader.currentPage.index - i];
					if (prev && !prev.isLoaded && !prev.isFailed && !clone.includes(prev.index)) {
						clone.push(prev.index);
					}
				}
			}

			if (!reader.currentPage.isLoaded && !reader.currentPage.isFailed) {
				if (clone.includes(reader.currentPage.index) && clone.indexOf(reader.currentPage.index) !== 0) {
					clone.splice(clone.indexOf(reader.currentPage.index), 1);
				}
				clone.unshift(reader.currentPage.index);
			}

			await tick();
			if (queue.length !== clone.length || queue.some((e, i) => e !== clone[i])) {
				queue = clone;
				await tick();
			}
		});

	let parallel = $state(0);
	const processQueue = (..._: any) =>
		processMutex.exec(async () => {
			await tick();
			await mutex.wait(() => {});
			if (!reader.pages.length || !queue.length) {
				return;
			}

			console.info("[Reader] Processing queue");
			if (parallel >= $readerPrefs.parallel) {
				console.info("[Reader] Parallel limit reached");
				return;
			}

			for (let i = 0; i < queue.length && parallel < $readerPrefs.parallel; i++) {
				const page = reader.pages[queue[i]];
				await mutex.wait(async () => {
					await tick();
					if (page.isLoading || page.isLoaded || page.isFailed) {
						return;
					}

					parallel = parallel + 1;
					page.isLoading = true;

					reader.pages = reader.pages;
					reader.spreads = reader.spreads;

					console.info(`[Reader] Loading Page ${page.index + 1} | Parallel: ${parallel} | Queue: ${queue.length}`);
					load(page).finally(() =>
						mutex.wait(async () => {
							await tick();
							page.isLoading = false;

							reader.pages = reader.pages;
							reader.spreads = reader.spreads;

							parallel = parallel - 1;
							queue = queue.filter(e => e !== page.index);

							if (page.isLoaded) {
								console.info(`[Reader] Loaded Page ${page.index + 1} | Parallel: ${parallel} | Queue: ${queue.length}`);
							} else if (page.isFailed) {
								console.info(
									`[Reader] Failed to load Page ${page.index + 1} | Parallel: ${parallel} | Queue: ${queue.length}`
								);
							}
							await tick();
						})
					);
				});
			}
		});

	$effect(() => {
		updateQueue(reader.currentPage, reader.pages, $readerPrefs.parallel, $readerPrefs.preload);
	});

	$effect(() => {
		processQueue(queue);
	});

	const requests: XMLHttpRequest[] = [];
	const maxAttempts = 3;

	const load = async (page: ReaderPage, attempts = 0) => {
		return new Promise<void>(resolve => {
			const xhr = new XMLHttpRequest();
			requests.push(xhr);

			xhr.responseType = "blob";
			xhr.open("GET", `${reader.chapter.node}/data/${reader.chapter.id}/${page.data.name}`);

			xhr.addEventListener("load", () => {
				const idx = requests.indexOf(xhr);
				if (idx >= 0) requests.splice(idx, 1);

				if (page.blob) URL.revokeObjectURL(page.blob);
				page.blob = URL.createObjectURL(xhr.response);
				page.isLoaded = true;
				resolve();
			});

			xhr.addEventListener("error", () => {
				const idx = requests.indexOf(xhr);
				if (idx >= 0) requests.splice(idx, 1);

				if (attempts < maxAttempts) {
					window.setTimeout(() => load(page, attempts + 1).then(resolve), 1000);
				} else {
					page.isFailed = true;
					resolve();
				}
			});
			xhr.send();
		});
	};
</script>
