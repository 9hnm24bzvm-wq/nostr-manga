> Currentness: frozen UI17 source has533 definitions/542 exact anchors across105 original paths. Authoritative inventory: `test-evidence/frontend/integration-source-manifest.json`; finite owner receipt: `test-evidence/frontend/round17-owner-source-repairs/summary.json`. Build18/source533 passed; Build17/source516 is historical. Source compilation passed93 Svelte components/56 scripts; no application/browser runtime journey has executed. Detailed earlier repair sections below are historical.

# Reviewed frontend script overlays

All original frontend attachment files remain unchanged. The pre-Svelte build transform validates each complete original SHA-256 and each exact anchor count. Most repairs affect scripts. Exact approved handler, validation, key/guard and additive control/panel exceptions are explicitly manifested; unrelated DOM/classes/styles remain strict byte comparisons. Global rendered HTML parity is not claimed. Composed source compilation passed; the current bundle is recorded separately; browser behavior remains unverified until the final test phase.

## weebdex import acknowledges persisted result

Source: `src/routes/(app)/settings/library/+page.svelte`; original SHA-256 `7ca6181b34ecb147d3d079beed6f309575b459c04b988f46d3bea3a63dc3ed88`; anchor count 1.

Exact before:
```ts
await API.MangaFollow.Import({ weebdex: chunk });
```

Exact after:
```ts
const result = await API.MangaFollow.Import({ weebdex: chunk });
			if (result?.status !== 204) {
				throw new Constants.Error.Error("Library import failed", `The server rejected an import batch (${result?.status ?? "network error"}). Earlier successful batches remain imported.`);
			}
```

## myanimelist import acknowledges persisted result

Source: `src/routes/(app)/settings/library/+page.svelte`; original SHA-256 `7ca6181b34ecb147d3d079beed6f309575b459c04b988f46d3bea3a63dc3ed88`; anchor count 1.

Exact before:
```ts
await API.MangaFollow.Import({ myanimelist: chunk });
```

Exact after:
```ts
const result = await API.MangaFollow.Import({ myanimelist: chunk });
			if (result?.status !== 204) {
				throw new Constants.Error.Error("Library import failed", `The server rejected an import batch (${result?.status ?? "network error"}). Earlier successful batches remain imported.`);
			}
```

## invalid archive surfaces failure

Source: `src/lib/components/upload/UploadChapterPages.svelte`; original SHA-256 `b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66`; anchor count 1.

Exact before:
```ts
else new Constants.Error.Error(file.name, "Failed to extract archive");
```

Exact after:
```ts
else throw new Constants.Error.Error(file.name, "Failed to extract archive");
```

## corrupt images release mutex and temporary URL

Source: `src/lib/components/upload/UploadChapterPages.svelte`; original SHA-256 `b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66`; anchor count 1.

Exact before:
```ts
const image = await new Promise<HTMLImageElement>(resolve => {
							const image = new Image();
							image.onload = () => resolve(image);
							image.src = URL.createObjectURL(file);
						});
```

Exact after:
```ts
const image = await new Promise<HTMLImageElement>((resolve, reject) => {
							const image = new Image();
							const objectUrl = URL.createObjectURL(file);
							const timeout = window.setTimeout(() => {
								URL.revokeObjectURL(objectUrl);
								reject(new Constants.Error.Error(file.name, "Image could not be decoded in time"));
							}, 30000);
							image.onload = () => {
								window.clearTimeout(timeout);
								URL.revokeObjectURL(objectUrl);
								resolve(image);
							};
							image.onerror = () => {
								window.clearTimeout(timeout);
								URL.revokeObjectURL(objectUrl);
								reject(new Constants.Error.Error(file.name, "Image is invalid or unsupported"));
							};
							image.src = objectUrl;
						});
```

## directory drops read every browser batch

Source: `src/lib/components/upload/UploadChapterPages.svelte`; original SHA-256 `b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66`; anchor count 1.

Exact before:
```ts
const entries = await new Promise<FileSystemEntry[]>((resolve, reject) => reader.readEntries(resolve, reject));
			return (await Promise.all(entries.map(scanFiles))).flat();
```

Exact after:
```ts
const entries: FileSystemEntry[] = [];
			while (true) {
				const batch = await new Promise<FileSystemEntry[]>((resolve, reject) => reader.readEntries(resolve, reject));
				if (!batch.length) break;
				entries.push(...batch);
				if (entries.length > 10000) throw new Constants.Error.Error("Upload folder is too large", "Choose a smaller folder");
			}
			return (await Promise.all(entries.map(scanFiles))).flat();
```

## page replacement refreshes chapter version

Source: `src/lib/components/upload/UploadChapterEdit.svelte`; original SHA-256 `4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c`; anchor count 1.

Exact before:
```ts
					uploadSession = undefined;
				} else if (res?.status === StatusCodes.FORBIDDEN)
```

Exact after:
```ts
					chapter = res.body;
					chapter.relationships.groups ??= [];
					uploadSession = undefined;
					onReset();
				} else if (res?.status === StatusCodes.FORBIDDEN)
```

## all unchecked content preferences encode empty discovery

Source: `src/lib/stores/prefs.ts`; original SHA-256 `cd441d7ca0bddd4ac0e2c8d60b40f073143b0c3b47084455dc2bb2c80acb64b6`; anchor count 1.

Exact before:
```ts
if (prefs.show_hentai) searchParams.append("contentRating", Constants.Manga.ContentRating.Pornographic);
				}
```

Exact after:
```ts
if (prefs.show_hentai) searchParams.append("contentRating", Constants.Manga.ContentRating.Pornographic);
					if (!(prefs.show_safe || prefs.show_suggestive || prefs.show_erotic || prefs.show_hentai)) searchParams.append("contentRating", "__none__");
				}
```

## clipboard success follows actual copy

Source: `src/routes/(app)/settings/client/ClientEntry.svelte`; original SHA-256 `cc119d9bd6ce9ee00809e83b5d9292d53704953695459b82e08bc931647415da`; anchor count 1.

Exact before:
```ts
const copy = () => {
		if (client.key) {
			navigator.clipboard.writeText(client.key);
			toast.success("Client secret has been successfully copied to the clipboard");
		}
	};
```

Exact after:
```ts
const copy = async () => {
		if (!client.key) return;
		try {
			await navigator.clipboard.writeText(client.key);
			toast.success("Client secret has been successfully copied to the clipboard");
		} catch {
			toast.error("Client secret could not be copied", { description: "Allow clipboard access and try again." });
		}
	};
```

## formatting an empty biography has no undefined prefix

Source: `src/lib/components/ui/RichTextarea.svelte`; original SHA-256 `ed7a0573680a285cc64a8a7ee90ccbda7358fe1c734eb5487d327e6acb099fd6`; anchor count 1.

Exact before:
```ts
value = value?.slice(0, start) + replacement + value?.slice(end);
```

Exact after:
```ts
value = (value ?? "").slice(0, start) + replacement + (value ?? "").slice(end);
```

## embedding into empty biography starts with an image

Source: `src/lib/components/ui/RichTextarea.svelte`; original SHA-256 `ed7a0573680a285cc64a8a7ee90ccbda7358fe1c734eb5487d327e6acb099fd6`; anchor count 1.

Exact before:
```ts
value += `![](${url})`;
```

Exact after:
```ts
value = (value ?? "") + `![](${url})`;
```

## public search reset preserves configured ratings

Source: `src/lib/components/MangaSearchFilter.svelte`; original SHA-256 `f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7`; anchor count 1.

Exact before:
```ts
		contentRating: [],
```

Exact after:
```ts
		contentRating: isLibrary ? [] : [
				...($prefs.show_safe ? [Constants.Manga.ContentRating.Safe] : []),
				...($prefs.show_suggestive ? [Constants.Manga.ContentRating.Suggestive] : []),
				...($prefs.show_erotic ? [Constants.Manga.ContentRating.Erotica] : []),
				...($prefs.show_hentai ? [Constants.Manga.ContentRating.Pornographic] : [])
			],
```

## initial rating selection is not duplicated

Source: `src/lib/components/MangaSearchFilter.svelte`; original SHA-256 `f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7`; anchor count 1.

Exact before:
```ts
				if ($prefs.show_safe) init.contentRating!.push(Constants.Manga.ContentRating.Safe);
				if ($prefs.show_suggestive) init.contentRating!.push(Constants.Manga.ContentRating.Suggestive);
				if ($prefs.show_erotic) init.contentRating!.push(Constants.Manga.ContentRating.Erotica);
				if ($prefs.show_hentai) init.contentRating!.push(Constants.Manga.ContentRating.Pornographic);

```

Exact after:
```ts

```

## empty rating sentinel renders as no selected options

Source: `src/lib/components/MangaSearchFilter.svelte`; original SHA-256 `f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7`; anchor count 1.

Exact before:
```ts
init[key as keyof SearchMangaRequest] = value;
```

Exact after:
```ts
init[key as keyof SearchMangaRequest] = key === "contentRating" && Array.isArray(value) ? value.filter(v => v !== "__none__") : value;
```

## explicitly clearing public ratings yields empty results

Source: `src/lib/components/MangaSearchFilter.svelte`; original SHA-256 `f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7`; anchor count 1.

Exact before:
```ts
if (!search.hasChapters) q.delete("hasChapters");
```

Exact after:
```ts
if (!isLibrary && !q.has("contentRating")) q.set("contentRating", "__none__");
			if (!search.hasChapters) q.delete("hasChapters");
```

## tag selectors load the complete paginated catalog

Source: `src/lib/stores/global.ts`; original SHA-256 `cb8a1eda7fae685c3fd62164080e3852a074406e98d1477acd494f76f0483671`; anchor count 1.

Exact before:
```ts
export const populateTagList = async () => {
	if (get(tagList)?.length) {
		return;
	}

	const res = await API.Tag.Search(new URLSearchParams());
	if (res?.status === StatusCodes.OK && res.body?.data) {
		tagList.set(Utils.groupTags(res.body.data));
	}
};
```

Exact after:
```ts
export const populateTagList = async () => {
	if (Object.keys(get(tagList) ?? {}).length) return;
	const tags: Tag[] = [];
	for (let page = 1; page <= 100; page++) {
		const query = new URLSearchParams();
		query.set("limit", "100");
		query.set("page", page.toString());
		const res = await API.Tag.Search(query);
		if (res?.status !== StatusCodes.OK || !res.body?.data) throw Constants.Error.Unknown;
		if (res.body.total > 10000) throw new Constants.Error.Error("Too many tags", "The tag catalog exceeds the selector budget");
		tags.push(...res.body.data);
		if (tags.length >= res.body.total) {
			tagList.set(Utils.groupTags(tags));
			return;
		}
		if (!res.body.data.length) throw Constants.Error.Unknown;
	}
	throw Constants.Error.Unknown;
};
```

## LatestFeed honors followed-tag preference exemption

Source: `src/lib/API/MangaFollow.ts`; original SHA-256 `f97aa38ea7465c95a9e0667b41c3cc85df7e5ba3483c121c2a99e2f971df357b`; anchor count 1.

Exact before:
```ts
export const LatestFeed = (searchParams: URLSearchParams) => {
```

Exact after:
```ts
export const LatestFeed = (searchParams: URLSearchParams) => {
	searchParams = new URLSearchParams(searchParams);
	searchParams.delete("tagx");
	searchParams.delete("txmod");
```

## LatestUpdates honors followed-tag preference exemption

Source: `src/lib/API/MangaFollow.ts`; original SHA-256 `f97aa38ea7465c95a9e0667b41c3cc85df7e5ba3483c121c2a99e2f971df357b`; anchor count 1.

Exact before:
```ts
export const LatestUpdates = (searchParams: URLSearchParams) => {
```

Exact after:
```ts
export const LatestUpdates = (searchParams: URLSearchParams) => {
	searchParams = new URLSearchParams(searchParams);
	searchParams.delete("tagx");
	searchParams.delete("txmod");
```

## queued mutex wait preserves ownership and always releases

Source: `src/lib/Mutex.ts`; original SHA-256 `e2b96e03790cba7460ed48e023264e75f3924250b097799837bb57ff8df9f3c3`; anchor count 1.

Exact before:
```ts
	wait = async <T>(callback: () => Promise<T> | T, onBusy?: () => void) => {
		await this.tryLock(onBusy);
		const r = await callback();
		this.unlock();
		return r;
	};

```

Exact after:
```ts
	wait = async <T>(callback: () => Promise<T> | T, onBusy?: () => void) => {
		const previous = this.pending;
		let release!: () => void;
		this.pending = new Promise<void>(resolve => { release = resolve; });
		await previous;
		await this.tryLock(onBusy);
		try {
			return await callback();
		} finally {
			this.unlock();
			release();
		}
	};

```

## mutex queue is independent of observable locked state

Source: `src/lib/Mutex.ts`; original SHA-256 `e2b96e03790cba7460ed48e023264e75f3924250b097799837bb57ff8df9f3c3`; anchor count 1.

Exact before:
```ts
readonly subscribe = this.state.subscribe;
```

Exact after:
```ts
readonly subscribe = this.state.subscribe;
	private pending: Promise<void> = Promise.resolve();
```

## mutex exec releases on callback rejection

Source: `src/lib/Mutex.ts`; original SHA-256 `e2b96e03790cba7460ed48e023264e75f3924250b097799837bb57ff8df9f3c3`; anchor count 1.

Exact before:
```ts
		this.lock();
		const r = await callback();
		this.unlock();
		return r;
```

Exact after:
```ts
		this.lock();
		try {
			return await callback();
		} finally {
			this.unlock();
		}
```

## account preference hydration reads scoped store directly

Source: `src/lib/stores/Session.ts`; original SHA-256 `8baf2edac726343a1f1445c53afa7f9a5338c66fad41ffbbe6ac7285d5a25498`; anchor count 1.

Exact before:
```ts
import Mutex from "$lib/Mutex";
```

Exact after:
```ts
import Mutex from "$lib/Mutex";
import { prefs } from "./prefs";
```

## session initialization supports explicit post-auth refresh

Source: `src/lib/stores/Session.ts`; original SHA-256 `8baf2edac726343a1f1445c53afa7f9a5338c66fad41ffbbe6ac7285d5a25498`; anchor count 1.

Exact before:
```ts
const initialize = () =>
	mutex.exec(async () => {
		if (get(store)) return;
```

Exact after:
```ts
const initialize = (force = false) =>
	mutex.wait(async () => {
		if (get(store) && !force) return;
```

## hydrate only authoritative account preferences without changing browser library

Source: `src/lib/stores/Session.ts`; original SHA-256 `8baf2edac726343a1f1445c53afa7f9a5338c66fad41ffbbe6ac7285d5a25498`; anchor count 1.

Exact before:
```ts
			store.set(res.body);
		}
```

Exact after:
```ts
			store.set(res.body);
			const account = res.body.preferences;
			if (account) {
				const allowed = ["items_per_page", "languages", "show_safe", "show_suggestive", "show_erotic", "show_hentai", "tags", "translated_languages", "uploaders", "groups", "data_saver", "version"];
				const patch = Object.fromEntries(Object.entries(account).filter(([key]) => allowed.includes(key)));
				prefs.update(current => ({ ...current, ...patch }));
			}
		} else if (res?.status === StatusCodes.UNAUTHORIZED) {
			store.set(undefined);
		}
```

## login accesses session refresh seam

Source: `src/routes/(app)/auth/login/+page.svelte`; original SHA-256 `0f352782af49530504142a2e6dc79e621f9433a53e581710917b386bd279c8c5`; anchor count 1.

Exact before:
```ts
import { ErrorSessionAlreadyAvailable, type UINode } from "$lib/Kratos";
```

Exact after:
```ts
import { ErrorSessionAlreadyAvailable, type UINode } from "$lib/Kratos";
	import { Session } from "$lib/stores";
```

## successful authentication refreshes account controls before SPA navigation

Source: `src/routes/(app)/auth/login/+page.svelte`; original SHA-256 `0f352782af49530504142a2e6dc79e621f9433a53e581710917b386bd279c8c5`; anchor count 1.

Exact before:
```ts
if (data?.session?.active) {
```

Exact after:
```ts
if (data?.session?.active) {
				await Session.initialize(true);
```

## successful authentication refreshes account controls before SPA navigation

Source: `src/routes/(app)/auth/register/+page.svelte`; original SHA-256 `e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c`; anchor count 1.

Exact before:
```ts
if (data?.session?.active) {
```

Exact after:
```ts
if (data?.session?.active) {
				await Session.initialize(true);
```

## account settings refreshes authoritative identity after updates

Source: `src/routes/(app)/settings/account/+page.svelte`; original SHA-256 `ff92851ff500757d698401a06c0f213092bbb853219bd7992546cae427d3646c`; anchor count 1.

Exact before:
```ts
import { InfoIcon, Link, Unlink } from "@lucide/svelte";
```

Exact after:
```ts
import { InfoIcon, Link, Unlink } from "@lucide/svelte";
	import { Session } from "$lib/stores";
```

## settings response rehydrates current session before flow rendering

Source: `src/routes/(app)/settings/account/+page.svelte`; original SHA-256 `ff92851ff500757d698401a06c0f213092bbb853219bd7992546cae427d3646c`; anchor count 1.

Exact before:
```ts
} else if (data?.ui) {
				flow = data;
```

Exact after:
```ts
} else if (data?.ui) {
				await Session.initialize(true);
				flow = data;
```

## reader queue cancels requests when closed or retried

Source: `src/lib/components/reader/reader-queue.svelte`; original SHA-256 `a40d89e88715e94cf4270dd7cfa9f4ef43ea1da33711cf12fda3ed06876dce03`; anchor count 1.

Exact before:
```ts
import { tick } from "svelte";
```

Exact after:
```ts
import { onDestroy, tick } from "svelte";
```

## reader HTTP failures expose real retry and bounded requests

Source: `src/lib/components/reader/reader-queue.svelte`; original SHA-256 `a40d89e88715e94cf4270dd7cfa9f4ef43ea1da33711cf12fda3ed06876dce03`; anchor count 1.

Exact before:
```ts
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
```

Exact after:
```ts
	const requests: XMLHttpRequest[] = [];
	const maxAttempts = 3;
	const cancellations = new Map<XMLHttpRequest, () => void>();
	const temporaryUrls = new Set<string>();
	let disposed = false;
	onDestroy(() => {
		disposed = true;
		for (const request of [...requests]) { request.abort(); cancellations.get(request)?.(); }
		for (const url of temporaryUrls) URL.revokeObjectURL(url);
		temporaryUrls.clear();
	});

	const load = async (page: ReaderPage, attempts = 0): Promise<void> => {
		if (disposed) return;
		return new Promise<void>(resolve => {
			const xhr = new XMLHttpRequest();
			requests.push(xhr);
			xhr.responseType = "blob";
			xhr.timeout = 60000;
			xhr.open("GET", `${reader.chapter.node}/data/${reader.chapter.id}/${page.data.name}`);
			let settled = false;
			const cleanup = () => {
				const idx = requests.indexOf(xhr);
				if (idx >= 0) requests.splice(idx, 1);
				xhr.onload = xhr.onerror = xhr.ontimeout = xhr.onabort = null;
				cancellations.delete(xhr);
			};
			const failed = () => {
				if (settled) return;
				settled = true;
				cleanup();
				if (disposed) return resolve();
				if (attempts < maxAttempts) {
					window.setTimeout(() => {
						if (disposed) resolve();
						else load(page, attempts + 1).then(resolve);
					}, 1000);
				} else {
					page.isLoaded = false;
					page.isFailed = true;
					resolve();
				}
			};
			xhr.onload = async () => {
				const contentType = xhr.getResponseHeader("Content-Type")?.split(";")[0]?.trim();
				if (![200, 206].includes(xhr.status) || !["image/jpeg", "image/png", "image/webp", "image/gif"].includes(contentType ?? "") || !(xhr.response instanceof Blob) || !xhr.response.size) return failed();
				const objectUrl = URL.createObjectURL(xhr.response);
				temporaryUrls.add(objectUrl);
				let decodeTimer: number | undefined;
				try {
					const image = new Image();
					image.src = objectUrl;
					await Promise.race([image.decode(), new Promise<void>((_, reject) => { decodeTimer = window.setTimeout(() => reject(new Error("Image decode timed out")), 30000); })]);
					window.clearTimeout(decodeTimer);
					temporaryUrls.delete(objectUrl);
					if (disposed || settled) { URL.revokeObjectURL(objectUrl); return resolve(); }
					settled = true;
					cleanup();
					if (page.blob) URL.revokeObjectURL(page.blob);
					page.blob = objectUrl;
					page.isFailed = false;
					page.isLoaded = true;
					resolve();
				} catch {
					window.clearTimeout(decodeTimer);
					temporaryUrls.delete(objectUrl);
					URL.revokeObjectURL(objectUrl);
					failed();
				}
			};
			xhr.onerror = xhr.ontimeout = failed;
			xhr.onabort = () => {
				if (settled) return;
				settled = true;
				cleanup();
				resolve();
			};
			cancellations.set(xhr, () => { if (!settled) { settled = true; cleanup(); resolve(); } });
			try { xhr.send(); } catch { failed(); }
		});
	};
```

## reader provider releases owned page blob URLs on exit

Source: `src/lib/components/reader/reader-provider.svelte`; original SHA-256 `c12a1d141b258f7461a150511387488193b55f6825f9f4c3cd86fbc962feb3fb`; anchor count 1.

Exact before:
```ts
	import { onMount, tick, type Snippet } from "svelte";
```

Exact after:
```ts
	import { onDestroy, onMount, tick, type Snippet } from "svelte";
```

## quality changes release prior blobs and cancel obsolete queue

Source: `src/lib/components/reader/reader-provider.svelte`; original SHA-256 `c12a1d141b258f7461a150511387488193b55f6825f9f4c3cd86fbc962feb3fb`; anchor count 1.

Exact before:
```ts
		if (isPreviousDataSaverChanged) {
			reader.pages =
```

Exact after:
```ts
		if (isPreviousDataSaverChanged) {
			reader.pages.forEach(page => { if (page.blob) URL.revokeObjectURL(page.blob); });
			reader.key++;
			reader.pages =
```

## reader exit releases downloaded object URLs

Source: `src/lib/components/reader/reader-provider.svelte`; original SHA-256 `c12a1d141b258f7461a150511387488193b55f6825f9f4c3cd86fbc962feb3fb`; anchor count 1.

Exact before:
```ts
	const readerPrefs = $derived(reader.prefs);
```

Exact after:
```ts
	const readerPrefs = $derived(reader.prefs);
	onDestroy(() => reader.pages.forEach(page => { if (page.blob) URL.revokeObjectURL(page.blob); }));
```

## Acceptance checks deferred to final testing

- A rejected WeebDex or MyAnimeList batch displays an error, never the successful-import message; earlier committed batches are described honestly.
- Invalid/corrupt/timed-out image decode shows an error and frees the upload mutex; a valid follow-up image can be added.
- Invalid archives surface an error; directory drops collect later readEntries batches.
- Page replacement updates the current chapter and version; a subsequent metadata save uses that version.
- All four content preferences unchecked yields an empty public discovery feed; default three enabled excludes pornography.
- Denied clipboard writes show an error rather than copied-success.
- Formatting and embedding an initially empty biography do not prepend the word undefined.

## Signed upload receipt availability

src/lib/components/upload/UploadChapter.svelte original SHA-256 91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3, one exact script anchor. A real accepted_pending_projection receipt now says the original protocol accepted the chapter and catalogue processing remains pending; it does not navigate Close to an unavailable reader. Explicit instance-local publication says where the chapter is actually available. Original markup and CSS remain byte-identical.

Before:
```ts
					if (await isPending()) {
```
After:
```ts
					const publication = res.body as Chapter & { publication_scope?: string; publication_status?: string };
					if (publication.publication_status === "accepted_pending_projection") {
						chapter = undefined;
						message = {
							title: "Signed chapter accepted",
							description: "The original protocol accepted this chapter. It is waiting for local catalogue processing before it can be read."
						};
					} else if (publication.publication_scope === "instance-local") {
						chapter = res.body;
						message = {
							title: "Chapter uploaded to this instance",
							description: "The chapter is available on this instance. Original-protocol publication is not configured here."
						};
					} else if (await isPending()) {
```

Acceptance: commit with pending source projection, receive success containing canonical event identity, show pending processing, Close returns to title; repeated identical commit returns durable pending receipt; once projection becomes visible the same retry returns the real public chapter. Disabled publication runtime displays instance-local availability without implying relay publication.

## Narrow approved event-handler JavaScript exception

src/routes/(app)/group/[id]/GroupClaimDialog.svelte original SHA-256 46723e0f934c3070727a9e1398918b977abab55ea8209648c5c279d7c1192f30, one exact onclick JavaScript anchor at original lines178–183. Parent approval permits this single script-bearing event-handler transformation outside the script tag. It changes only clipboard await/error behavior; original attachment bytes, DOM elements, labels, layout, classes, styling and all other markup remain unchanged. No copied values are inspected or logged. Manifest field approved_event_handler_exception identifies exactly this one permitted normalization in the retained-markup comparison.

Before:
```svelte
onclick={() => {
										navigator.clipboard.writeText(`${$Session!.identity.id}+${group.id}`);
										toast.success("Verification code has been successfully copied to the clipboard", {
											closeButton: false
										});
									}}
```
After:
```svelte
onclick={async () => {
										try {
											await navigator.clipboard.writeText(`${$Session!.identity.id}+${group.id}`);
											toast.success("Verification code has been successfully copied to the clipboard", { closeButton: false });
										} catch {
											toast.error("Could not copy verification code", { description: "Allow clipboard access or copy the displayed code manually." });
										}
									}}
```

Acceptance: allow clipboard, copy the displayed group verification code and receive success only after write; deny clipboard and receive useful error while the displayed code remains available for manual copy. Submission still creates only a claim review and never grants ownership automatically.

## Native chapter DELETE feedback integration

Three additional exact-SHA/anchor JavaScript repairs expose only the agreed
`x-weebdex-instance-removal`, `x-weebdex-source-deletion-status`, and optional
`x-weebdex-source-deletion-request` response headers through the retained XHR
helper, then change the chapter-edit delete toast. A successful local hide says
that only this instance's catalogue entry was hidden. A signed source request
says deletion is awaiting moderator review and explicitly avoids promising peer
copy removal. Status204 and original navigation remain unchanged; backend failure
statuses still show failures. No response bodies, cookies or arbitrary headers
are exposed. No supplied source file or markup/style is written.

Acceptance: exercise actual instance-local deletion and assert the local-hide
message; exercise native signed deletion and assert a real request receipt plus
pending-moderator message. Missing/mismatched authority must remain a failure.

Exact additional transformations:

### HTTP response exposes only native deletion feedback headers

Original SHA-256: `765a7ee6804ec81472c945e83a24b0b4343c0d58e8dbb2a2ab2421b681ba2fd1`; path: `src/lib/HTTP.ts`; occurrence count:1.

```javascript
	body?: T;
}
```

```javascript
	body?: T;
	headers?: Record<string, string | null>;
}
```

### successful XHR carries narrowly allowlisted deletion receipt headers

Original SHA-256: `765a7ee6804ec81472c945e83a24b0b4343c0d58e8dbb2a2ab2421b681ba2fd1`; path: `src/lib/HTTP.ts`; occurrence count:1.

```javascript
resolve({ status, body: response });
```

```javascript
resolve({ status, body: response, headers: {
						"x-weebdex-instance-removal": this.xhr.getResponseHeader("x-weebdex-instance-removal"),
						"x-weebdex-source-deletion-status": this.xhr.getResponseHeader("x-weebdex-source-deletion-status"),
						"x-weebdex-source-deletion-request": this.xhr.getResponseHeader("x-weebdex-source-deletion-request")
					} });
```

### chapter deletion feedback distinguishes local hide and pending signed review

Original SHA-256: `4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c`; path: `src/lib/components/upload/UploadChapterEdit.svelte`; occurrence count:1.

```javascript
toast.success("Chapter deleted successfully", {
						description: "It may take a few minutes for the change to be reflected."
					});
```

```javascript
toast.success("Chapter hidden on this instance", {
						description: res.headers?.["x-weebdex-source-deletion-status"] === "moderator-review-requested"
							? "Source deletion is awaiting moderator review. Other peer copies are not guaranteed to be removed."
							: "Only this instance’s catalogue entry was hidden. Other peer copies may remain."
					});
```

## UI17 partial cover and draft recovery

Current contracts and exact source evidence: `docs/round17-frontend-source-repairs.md`. All original676 files remain byte-identical. The acknowledged-draft error screen has one explicitly approved recovery-controls anchor; no unrelated DOM/classes/styles changed.

## UI18 relation control eligibility

A single original-SHA/exact-count script-only repair of `src/lib/permission/manga.ts` replaces `canEditRelations`. The original negated `deleted_at` denied every live ordinary title. The replacement follows `require_manga_edit`: a normal account is required; deleted, unavailable and immutable-pending DTOs are denied; staff/moderator with the exact relation permission can edit live locked titles; otherwise an unlocked published title requires `manga.relation.edit`, and an unlocked actual draft submitter requires `manga.create`. The existing unsaved create-form invocation uses `manga.create` for prospective owner selection. This helper only makes retained controls reachable; the backend still enforces current visibility, identity, permissions, lock and immutable publication fences. No original source, markup, classes or styles were changed. Exact before/after and original SHA are in `test-evidence/frontend/round18-relation-permission/exact-repair.json`. Source compilation and browser acceptance are distinct; Build19/source534 completed successfully; fresh independent source review and real browser acceptance remain unrun.
