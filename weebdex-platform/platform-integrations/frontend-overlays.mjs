// Reviewed script-only integration repairs; immutable attachment files are never written.
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
const patches = [
  {
    "path": "src/routes/(app)/settings/library/+page.svelte",
    "label": "weebdex import acknowledges persisted result",
    "sha256": "7ca6181b34ecb147d3d079beed6f309575b459c04b988f46d3bea3a63dc3ed88",
    "count": 1,
    "before": "await API.MangaFollow.Import({ weebdex: chunk });",
    "after": "const result = await API.MangaFollow.Import({ weebdex: chunk });\r\n\t\t\tif (result?.status !== 204) {\r\n\t\t\t\tthrow new Constants.Error.Error(\"Library import failed\", `The server rejected an import batch (${result?.status ?? \"network error\"}). Earlier successful batches remain imported.`);\r\n\t\t\t}"
  },
  {
    "path": "src/routes/(app)/settings/library/+page.svelte",
    "label": "myanimelist import acknowledges persisted result",
    "sha256": "7ca6181b34ecb147d3d079beed6f309575b459c04b988f46d3bea3a63dc3ed88",
    "count": 1,
    "before": "await API.MangaFollow.Import({ myanimelist: chunk });",
    "after": "const result = await API.MangaFollow.Import({ myanimelist: chunk });\r\n\t\t\tif (result?.status !== 204) {\r\n\t\t\t\tthrow new Constants.Error.Error(\"Library import failed\", `The server rejected an import batch (${result?.status ?? \"network error\"}). Earlier successful batches remain imported.`);\r\n\t\t\t}"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "page replacement refreshes chapter version",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "\t\t\t\t\tuploadSession = undefined;\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN)",
    "after": "\t\t\t\t\tchapter = res.body;\r\n\t\t\t\t\tchapter.relationships.groups ??= [];\r\n\t\t\t\t\tuploadSession = undefined;\r\n\t\t\t\t\tonReset();\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN)"
  },
  {
    "path": "src/lib/stores/prefs.ts",
    "label": "all unchecked content preferences encode empty discovery",
    "sha256": "cd441d7ca0bddd4ac0e2c8d60b40f073143b0c3b47084455dc2bb2c80acb64b6",
    "count": 1,
    "before": "if (prefs.show_hentai) searchParams.append(\"contentRating\", Constants.Manga.ContentRating.Pornographic);\r\n\t\t\t\t}",
    "after": "if (prefs.show_hentai) searchParams.append(\"contentRating\", Constants.Manga.ContentRating.Pornographic);\r\n\t\t\t\t\tif (!(prefs.show_safe || prefs.show_suggestive || prefs.show_erotic || prefs.show_hentai)) searchParams.append(\"contentRating\", \"__none__\");\r\n\t\t\t\t}"
  },
  {
    "path": "src/routes/(app)/settings/client/ClientEntry.svelte",
    "label": "clipboard success follows actual copy",
    "sha256": "cc119d9bd6ce9ee00809e83b5d9292d53704953695459b82e08bc931647415da",
    "count": 1,
    "before": "const copy = () => {\r\n\t\tif (client.key) {\r\n\t\t\tnavigator.clipboard.writeText(client.key);\r\n\t\t\ttoast.success(\"Client secret has been successfully copied to the clipboard\");\r\n\t\t}\r\n\t};",
    "after": "const copy = async () => {\r\n\t\tif (!client.key) return;\r\n\t\ttry {\r\n\t\t\tawait navigator.clipboard.writeText(client.key);\r\n\t\t\ttoast.success(\"Client secret has been successfully copied to the clipboard\");\r\n\t\t} catch {\r\n\t\t\ttoast.error(\"Client secret could not be copied\", { description: \"Allow clipboard access and try again.\" });\r\n\t\t}\r\n\t};"
  },
  {
    "path": "src/lib/components/ui/RichTextarea.svelte",
    "label": "formatting an empty biography has no undefined prefix",
    "sha256": "ed7a0573680a285cc64a8a7ee90ccbda7358fe1c734eb5487d327e6acb099fd6",
    "count": 1,
    "before": "value = value?.slice(0, start) + replacement + value?.slice(end);",
    "after": "value = (value ?? \"\").slice(0, start) + replacement + (value ?? \"\").slice(end);"
  },
  {
    "path": "src/lib/components/ui/RichTextarea.svelte",
    "label": "embedding into empty biography starts with an image",
    "sha256": "ed7a0573680a285cc64a8a7ee90ccbda7358fe1c734eb5487d327e6acb099fd6",
    "count": 1,
    "before": "value += `![](${url})`;",
    "after": "value = (value ?? \"\") + `![](${url})`;"
  },
  {
    "path": "src/lib/components/MangaSearchFilter.svelte",
    "label": "public search reset preserves configured ratings",
    "sha256": "f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7",
    "count": 1,
    "before": "\t\tcontentRating: [],",
    "after": "\t\tcontentRating: isLibrary ? [] : [\r\n\t\t\t\t...($prefs.show_safe ? [Constants.Manga.ContentRating.Safe] : []),\r\n\t\t\t\t...($prefs.show_suggestive ? [Constants.Manga.ContentRating.Suggestive] : []),\r\n\t\t\t\t...($prefs.show_erotic ? [Constants.Manga.ContentRating.Erotica] : []),\r\n\t\t\t\t...($prefs.show_hentai ? [Constants.Manga.ContentRating.Pornographic] : [])\r\n\t\t\t],"
  },
  {
    "path": "src/lib/components/MangaSearchFilter.svelte",
    "label": "initial rating selection is not duplicated",
    "sha256": "f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7",
    "count": 1,
    "before": "\t\t\t\tif ($prefs.show_safe) init.contentRating!.push(Constants.Manga.ContentRating.Safe);\r\n\t\t\t\tif ($prefs.show_suggestive) init.contentRating!.push(Constants.Manga.ContentRating.Suggestive);\r\n\t\t\t\tif ($prefs.show_erotic) init.contentRating!.push(Constants.Manga.ContentRating.Erotica);\r\n\t\t\t\tif ($prefs.show_hentai) init.contentRating!.push(Constants.Manga.ContentRating.Pornographic);\r\n",
    "after": ""
  },
  {
    "path": "src/lib/components/MangaSearchFilter.svelte",
    "label": "empty rating sentinel renders as no selected options",
    "sha256": "f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7",
    "count": 1,
    "before": "init[key as keyof SearchMangaRequest] = value;",
    "after": "init[key as keyof SearchMangaRequest] = key === \"contentRating\" && Array.isArray(value) ? value.filter(v => v !== \"__none__\") : value;"
  },
  {
    "path": "src/lib/components/MangaSearchFilter.svelte",
    "label": "explicitly clearing public ratings yields empty results",
    "sha256": "f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7",
    "count": 1,
    "before": "if (!search.hasChapters) q.delete(\"hasChapters\");",
    "after": "if (!isLibrary && !q.has(\"contentRating\")) q.set(\"contentRating\", \"__none__\");\r\n\t\t\tif (!search.hasChapters) q.delete(\"hasChapters\");"
  },
  {
    "path": "src/lib/stores/global.ts",
    "label": "tag selectors load the complete paginated catalog",
    "sha256": "cb8a1eda7fae685c3fd62164080e3852a074406e98d1477acd494f76f0483671",
    "count": 1,
    "before": "export const populateTagList = async () => {\r\n\tif (get(tagList)?.length) {\r\n\t\treturn;\r\n\t}\r\n\r\n\tconst res = await API.Tag.Search(new URLSearchParams());\r\n\tif (res?.status === StatusCodes.OK && res.body?.data) {\r\n\t\ttagList.set(Utils.groupTags(res.body.data));\r\n\t}\r\n};",
    "after": "export const populateTagList = async () => {\r\n\tif (Object.keys(get(tagList) ?? {}).length) return;\r\n\tconst tags: Tag[] = [];\r\n\tfor (let page = 1; page <= 100; page++) {\r\n\t\tconst query = new URLSearchParams();\r\n\t\tquery.set(\"limit\", \"100\");\r\n\t\tquery.set(\"page\", page.toString());\r\n\t\tconst res = await API.Tag.Search(query);\r\n\t\tif (res?.status !== StatusCodes.OK || !res.body?.data) throw Constants.Error.Unknown;\r\n\t\tif (res.body.total > 10000) throw new Constants.Error.Error(\"Too many tags\", \"The tag catalog exceeds the selector budget\");\r\n\t\ttags.push(...res.body.data);\r\n\t\tif (tags.length >= res.body.total) {\r\n\t\t\ttagList.set(Utils.groupTags(tags));\r\n\t\t\treturn;\r\n\t\t}\r\n\t\tif (!res.body.data.length) throw Constants.Error.Unknown;\r\n\t}\r\n\tthrow Constants.Error.Unknown;\r\n};"
  },
  {
    "path": "src/lib/API/MangaFollow.ts",
    "label": "LatestFeed honors followed-tag preference exemption",
    "sha256": "f97aa38ea7465c95a9e0667b41c3cc85df7e5ba3483c121c2a99e2f971df357b",
    "count": 1,
    "before": "export const LatestFeed = (searchParams: URLSearchParams) => {",
    "after": "export const LatestFeed = (searchParams: URLSearchParams) => {\r\n\tsearchParams = new URLSearchParams(searchParams);\r\n\tsearchParams.delete(\"tagx\");\r\n\tsearchParams.delete(\"txmod\");"
  },
  {
    "path": "src/lib/API/MangaFollow.ts",
    "label": "LatestUpdates honors followed-tag preference exemption",
    "sha256": "f97aa38ea7465c95a9e0667b41c3cc85df7e5ba3483c121c2a99e2f971df357b",
    "count": 1,
    "before": "export const LatestUpdates = (searchParams: URLSearchParams) => {",
    "after": "export const LatestUpdates = (searchParams: URLSearchParams) => {\r\n\tsearchParams = new URLSearchParams(searchParams);\r\n\tsearchParams.delete(\"tagx\");\r\n\tsearchParams.delete(\"txmod\");"
  },
  {
    "path": "src/lib/Mutex.ts",
    "label": "queued mutex wait preserves ownership and always releases",
    "sha256": "e2b96e03790cba7460ed48e023264e75f3924250b097799837bb57ff8df9f3c3",
    "count": 1,
    "before": "\twait = async <T>(callback: () => Promise<T> | T, onBusy?: () => void) => {\r\n\t\tawait this.tryLock(onBusy);\r\n\t\tconst r = await callback();\r\n\t\tthis.unlock();\r\n\t\treturn r;\r\n\t};\r\n",
    "after": "\twait = async <T>(callback: () => Promise<T> | T, onBusy?: () => void) => {\r\n\t\tconst previous = this.pending;\r\n\t\tlet release!: () => void;\r\n\t\tthis.pending = new Promise<void>(resolve => { release = resolve; });\r\n\t\tawait previous;\r\n\t\tawait this.tryLock(onBusy);\r\n\t\ttry {\r\n\t\t\treturn await callback();\r\n\t\t} finally {\r\n\t\t\tthis.unlock();\r\n\t\t\trelease();\r\n\t\t}\r\n\t};\r\n"
  },
  {
    "path": "src/lib/Mutex.ts",
    "label": "mutex queue is independent of observable locked state",
    "sha256": "e2b96e03790cba7460ed48e023264e75f3924250b097799837bb57ff8df9f3c3",
    "count": 1,
    "before": "readonly subscribe = this.state.subscribe;",
    "after": "readonly subscribe = this.state.subscribe;\r\n\tprivate pending: Promise<void> = Promise.resolve();"
  },
  {
    "path": "src/lib/Mutex.ts",
    "label": "mutex exec releases on callback rejection",
    "sha256": "e2b96e03790cba7460ed48e023264e75f3924250b097799837bb57ff8df9f3c3",
    "count": 1,
    "before": "\t\tthis.lock();\r\n\t\tconst r = await callback();\r\n\t\tthis.unlock();\r\n\t\treturn r;",
    "after": "\t\tthis.lock();\r\n\t\ttry {\r\n\t\t\treturn await callback();\r\n\t\t} finally {\r\n\t\t\tthis.unlock();\r\n\t\t}"
  },
  {
    "path": "src/lib/stores/Session.ts",
    "label": "account preference hydration reads scoped store directly",
    "sha256": "8baf2edac726343a1f1445c53afa7f9a5338c66fad41ffbbe6ac7285d5a25498",
    "count": 1,
    "before": "import Mutex from \"$lib/Mutex\";",
    "after": "import Mutex from \"$lib/Mutex\";\r\nimport { prefs } from \"./prefs\";"
  },
  {
    "path": "src/lib/stores/Session.ts",
    "label": "session initialization supports explicit post-auth refresh",
    "sha256": "8baf2edac726343a1f1445c53afa7f9a5338c66fad41ffbbe6ac7285d5a25498",
    "count": 1,
    "before": "const initialize = () =>\r\n\tmutex.exec(async () => {\r\n\t\tif (get(store)) return;",
    "after": "const initialize = (force = false) =>\r\n\tmutex.wait(async () => {\r\n\t\tif (get(store) && !force) return;"
  },
  {
    "path": "src/lib/stores/Session.ts",
    "label": "hydrate only authoritative account preferences without changing browser library",
    "sha256": "8baf2edac726343a1f1445c53afa7f9a5338c66fad41ffbbe6ac7285d5a25498",
    "count": 1,
    "before": "\t\t\tstore.set(res.body);\r\n\t\t}",
    "after": "\t\t\tstore.set(res.body);\r\n\t\t\tconst account = res.body.preferences;\r\n\t\t\tif (account) {\r\n\t\t\t\tconst allowed = [\"items_per_page\", \"languages\", \"show_safe\", \"show_suggestive\", \"show_erotic\", \"show_hentai\", \"tags\", \"translated_languages\", \"uploaders\", \"groups\", \"data_saver\", \"version\"];\r\n\t\t\t\tconst patch = Object.fromEntries(Object.entries(account).filter(([key]) => allowed.includes(key)));\r\n\t\t\t\tprefs.update(current => ({ ...current, ...patch }));\r\n\t\t\t}\r\n\t\t} else if (res?.status === StatusCodes.UNAUTHORIZED) {\r\n\t\t\tstore.set(undefined);\r\n\t\t}"
  },
  {
    "path": "src/routes/(app)/auth/login/+page.svelte",
    "label": "login accesses session refresh seam",
    "sha256": "0f352782af49530504142a2e6dc79e621f9433a53e581710917b386bd279c8c5",
    "count": 1,
    "before": "import { ErrorSessionAlreadyAvailable, type UINode } from \"$lib/Kratos\";",
    "after": "import { ErrorSessionAlreadyAvailable, type UINode } from \"$lib/Kratos\";\r\n\timport { Session } from \"$lib/stores\";"
  },
  {
    "path": "src/routes/(app)/auth/login/+page.svelte",
    "label": "successful authentication refreshes account controls before SPA navigation",
    "sha256": "0f352782af49530504142a2e6dc79e621f9433a53e581710917b386bd279c8c5",
    "count": 1,
    "before": "if (data?.session?.active) {",
    "after": "if (data?.session?.active) {\r\n\t\t\t\tawait Session.initialize(true);"
  },
  {
    "path": "src/routes/(app)/auth/register/+page.svelte",
    "label": "successful authentication refreshes account controls before SPA navigation",
    "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
    "count": 1,
    "before": "if (data?.session?.active) {",
    "after": "if (data?.session?.active) {\r\n\t\t\t\tawait Session.initialize(true);"
  },
  {
    "path": "src/routes/(app)/settings/account/+page.svelte",
    "label": "account settings refreshes authoritative identity after updates",
    "sha256": "ff92851ff500757d698401a06c0f213092bbb853219bd7992546cae427d3646c",
    "count": 1,
    "before": "import { InfoIcon, Link, Unlink } from \"@lucide/svelte\";",
    "after": "import { InfoIcon, Link, Unlink } from \"@lucide/svelte\";\r\n\timport { Session } from \"$lib/stores\";"
  },
  {
    "path": "src/routes/(app)/settings/account/+page.svelte",
    "label": "settings response rehydrates current session before flow rendering",
    "sha256": "ff92851ff500757d698401a06c0f213092bbb853219bd7992546cae427d3646c",
    "count": 1,
    "before": "} else if (data?.ui) {\r\n\t\t\t\tflow = data;",
    "after": "} else if (data?.ui) {\r\n\t\t\t\tawait Session.initialize(true);\r\n\t\t\t\tflow = data;"
  },
  {
    "path": "src/lib/components/reader/reader-queue.svelte",
    "label": "reader queue cancels requests when closed or retried",
    "sha256": "a40d89e88715e94cf4270dd7cfa9f4ef43ea1da33711cf12fda3ed06876dce03",
    "count": 1,
    "before": "import { tick } from \"svelte\";",
    "after": "import { onDestroy, tick } from \"svelte\";"
  },
  {
    "path": "src/lib/components/reader/reader-queue.svelte",
    "label": "reader HTTP failures expose real retry and bounded requests",
    "sha256": "a40d89e88715e94cf4270dd7cfa9f4ef43ea1da33711cf12fda3ed06876dce03",
    "count": 1,
    "before": "\tconst requests: XMLHttpRequest[] = [];\r\n\tconst maxAttempts = 3;\r\n\r\n\tconst load = async (page: ReaderPage, attempts = 0) => {\r\n\t\treturn new Promise<void>(resolve => {\r\n\t\t\tconst xhr = new XMLHttpRequest();\r\n\t\t\trequests.push(xhr);\r\n\r\n\t\t\txhr.responseType = \"blob\";\r\n\t\t\txhr.open(\"GET\", `${reader.chapter.node}/data/${reader.chapter.id}/${page.data.name}`);\r\n\r\n\t\t\txhr.addEventListener(\"load\", () => {\r\n\t\t\t\tconst idx = requests.indexOf(xhr);\r\n\t\t\t\tif (idx >= 0) requests.splice(idx, 1);\r\n\r\n\t\t\t\tif (page.blob) URL.revokeObjectURL(page.blob);\r\n\t\t\t\tpage.blob = URL.createObjectURL(xhr.response);\r\n\t\t\t\tpage.isLoaded = true;\r\n\t\t\t\tresolve();\r\n\t\t\t});\r\n\r\n\t\t\txhr.addEventListener(\"error\", () => {\r\n\t\t\t\tconst idx = requests.indexOf(xhr);\r\n\t\t\t\tif (idx >= 0) requests.splice(idx, 1);\r\n\r\n\t\t\t\tif (attempts < maxAttempts) {\r\n\t\t\t\t\twindow.setTimeout(() => load(page, attempts + 1).then(resolve), 1000);\r\n\t\t\t\t} else {\r\n\t\t\t\t\tpage.isFailed = true;\r\n\t\t\t\t\tresolve();\r\n\t\t\t\t}\r\n\t\t\t});\r\n\t\t\txhr.send();\r\n\t\t});\r\n\t};",
    "after": "\tconst requests: XMLHttpRequest[] = [];\r\n\tconst maxAttempts = 3;\r\n\tconst cancellations = new Map<XMLHttpRequest, () => void>();\r\n\tconst temporaryUrls = new Set<string>();\r\n\tlet disposed = false;\r\n\tonDestroy(() => {\r\n\t\tdisposed = true;\r\n\t\tfor (const request of [...requests]) { request.abort(); cancellations.get(request)?.(); }\r\n\t\tfor (const url of temporaryUrls) URL.revokeObjectURL(url);\r\n\t\ttemporaryUrls.clear();\r\n\t});\r\n\r\n\tconst load = async (page: ReaderPage, attempts = 0): Promise<void> => {\r\n\t\tif (disposed) return;\r\n\t\treturn new Promise<void>(resolve => {\r\n\t\t\tconst xhr = new XMLHttpRequest();\r\n\t\t\trequests.push(xhr);\r\n\t\t\txhr.responseType = \"blob\";\r\n\t\t\txhr.timeout = 60000;\r\n\t\t\txhr.open(\"GET\", `${reader.chapter.node}/data/${reader.chapter.id}/${page.data.name}`);\r\n\t\t\tlet settled = false;\r\n\t\t\tconst cleanup = () => {\r\n\t\t\t\tconst idx = requests.indexOf(xhr);\r\n\t\t\t\tif (idx >= 0) requests.splice(idx, 1);\r\n\t\t\t\txhr.onload = xhr.onerror = xhr.ontimeout = xhr.onabort = null;\r\n\t\t\t\tcancellations.delete(xhr);\r\n\t\t\t};\r\n\t\t\tconst failed = () => {\r\n\t\t\t\tif (settled) return;\r\n\t\t\t\tsettled = true;\r\n\t\t\t\tcleanup();\r\n\t\t\t\tif (disposed) return resolve();\r\n\t\t\t\tif (attempts < maxAttempts) {\r\n\t\t\t\t\twindow.setTimeout(() => {\r\n\t\t\t\t\t\tif (disposed) resolve();\r\n\t\t\t\t\t\telse load(page, attempts + 1).then(resolve);\r\n\t\t\t\t\t}, 1000);\r\n\t\t\t\t} else {\r\n\t\t\t\t\tpage.isLoaded = false;\r\n\t\t\t\t\tpage.isFailed = true;\r\n\t\t\t\t\tresolve();\r\n\t\t\t\t}\r\n\t\t\t};\r\n\t\t\txhr.onload = async () => {\r\n\t\t\t\tconst contentType = xhr.getResponseHeader(\"Content-Type\")?.split(\";\")[0]?.trim();\r\n\t\t\t\tif (![200, 206].includes(xhr.status) || ![\"image/jpeg\", \"image/png\", \"image/webp\", \"image/gif\"].includes(contentType ?? \"\") || !(xhr.response instanceof Blob) || !xhr.response.size) return failed();\r\n\t\t\t\tconst objectUrl = URL.createObjectURL(xhr.response);\r\n\t\t\t\ttemporaryUrls.add(objectUrl);\r\n\t\t\t\tlet decodeTimer: number | undefined;\r\n\t\t\t\ttry {\r\n\t\t\t\t\tconst image = new Image();\r\n\t\t\t\t\timage.src = objectUrl;\r\n\t\t\t\t\tawait Promise.race([image.decode(), new Promise<void>((_, reject) => { decodeTimer = window.setTimeout(() => reject(new Error(\"Image decode timed out\")), 30000); })]);\r\n\t\t\t\t\twindow.clearTimeout(decodeTimer);\r\n\t\t\t\t\ttemporaryUrls.delete(objectUrl);\r\n\t\t\t\t\tif (disposed || settled) { URL.revokeObjectURL(objectUrl); return resolve(); }\r\n\t\t\t\t\tsettled = true;\r\n\t\t\t\t\tcleanup();\r\n\t\t\t\t\tif (page.blob) URL.revokeObjectURL(page.blob);\r\n\t\t\t\t\tpage.blob = objectUrl;\r\n\t\t\t\t\tpage.isFailed = false;\r\n\t\t\t\t\tpage.isLoaded = true;\r\n\t\t\t\t\tresolve();\r\n\t\t\t\t} catch {\r\n\t\t\t\t\twindow.clearTimeout(decodeTimer);\r\n\t\t\t\t\ttemporaryUrls.delete(objectUrl);\r\n\t\t\t\t\tURL.revokeObjectURL(objectUrl);\r\n\t\t\t\t\tfailed();\r\n\t\t\t\t}\r\n\t\t\t};\r\n\t\t\txhr.onerror = xhr.ontimeout = failed;\r\n\t\t\txhr.onabort = () => {\r\n\t\t\t\tif (settled) return;\r\n\t\t\t\tsettled = true;\r\n\t\t\t\tcleanup();\r\n\t\t\t\tresolve();\r\n\t\t\t};\r\n\t\t\tcancellations.set(xhr, () => { if (!settled) { settled = true; cleanup(); resolve(); } });\r\n\t\t\ttry { xhr.send(); } catch { failed(); }\r\n\t\t});\r\n\t};"
  },
  {
    "path": "src/lib/components/reader/reader-provider.svelte",
    "label": "reader provider releases owned page blob URLs on exit",
    "sha256": "c12a1d141b258f7461a150511387488193b55f6825f9f4c3cd86fbc962feb3fb",
    "count": 1,
    "before": "\timport { onMount, tick, type Snippet } from \"svelte\";",
    "after": "\timport { onDestroy, onMount, tick, type Snippet } from \"svelte\";"
  },
  {
    "path": "src/lib/components/reader/reader-provider.svelte",
    "label": "quality changes release prior blobs and cancel obsolete queue",
    "sha256": "c12a1d141b258f7461a150511387488193b55f6825f9f4c3cd86fbc962feb3fb",
    "count": 1,
    "before": "\t\tif (isPreviousDataSaverChanged) {\r\n\t\t\treader.pages =",
    "after": "\t\tif (isPreviousDataSaverChanged) {\r\n\t\t\treader.pages.forEach(page => { if (page.blob) URL.revokeObjectURL(page.blob); });\r\n\t\t\treader.key++;\r\n\t\t\treader.pages ="
  },
  {
    "path": "src/lib/components/reader/reader-provider.svelte",
    "label": "reader exit releases downloaded object URLs",
    "sha256": "c12a1d141b258f7461a150511387488193b55f6825f9f4c3cd86fbc962feb3fb",
    "count": 1,
    "before": "\tconst readerPrefs = $derived(reader.prefs);",
    "after": "\tconst readerPrefs = $derived(reader.prefs);\r\n\tonDestroy(() => reader.pages.forEach(page => { if (page.blob) URL.revokeObjectURL(page.blob); }));"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "signed upload receipt exposes pending or instance-local availability truthfully",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "\t\t\t\t\tif (await isPending()) {",
    "after": "\t\t\t\t\tconst publication = res.body as Chapter & { publication_scope?: string; publication_status?: string };\r\n\t\t\t\t\tif (publication.publication_status === \"accepted_pending_projection\") {\r\n\t\t\t\t\t\tchapter = undefined;\r\n\t\t\t\t\t\tmessage = {\r\n\t\t\t\t\t\t\ttitle: \"Signed chapter accepted\",\r\n\t\t\t\t\t\t\tdescription: \"The original protocol accepted this chapter. It is waiting for local catalogue processing before it can be read.\"\r\n\t\t\t\t\t\t};\r\n\t\t\t\t\t} else if (publication.publication_scope === \"instance-local\") {\r\n\t\t\t\t\t\tchapter = res.body;\r\n\t\t\t\t\t\tmessage = {\r\n\t\t\t\t\t\t\ttitle: \"Chapter uploaded to this instance\",\r\n\t\t\t\t\t\t\tdescription: \"The chapter is available on this instance. Original-protocol publication is not configured here.\"\r\n\t\t\t\t\t\t};\r\n\t\t\t\t\t} else if (await isPending()) {"
  },
  {
    "path": "src/routes/(app)/group/[id]/GroupClaimDialog.svelte",
    "label": "group claim verification copy acknowledges actual clipboard result",
    "sha256": "46723e0f934c3070727a9e1398918b977abab55ea8209648c5c279d7c1192f30",
    "count": 1,
    "before": "onclick={() => {\r\n\t\t\t\t\t\t\t\t\t\tnavigator.clipboard.writeText(`${$Session!.identity.id}+${group.id}`);\r\n\t\t\t\t\t\t\t\t\t\ttoast.success(\"Verification code has been successfully copied to the clipboard\", {\r\n\t\t\t\t\t\t\t\t\t\t\tcloseButton: false\r\n\t\t\t\t\t\t\t\t\t\t});\r\n\t\t\t\t\t\t\t\t\t}}",
    "after": "onclick={async () => {\r\n\t\t\t\t\t\t\t\t\t\ttry {\r\n\t\t\t\t\t\t\t\t\t\t\tawait navigator.clipboard.writeText(`${$Session!.identity.id}+${group.id}`);\r\n\t\t\t\t\t\t\t\t\t\t\ttoast.success(\"Verification code has been successfully copied to the clipboard\", { closeButton: false });\r\n\t\t\t\t\t\t\t\t\t\t} catch {\r\n\t\t\t\t\t\t\t\t\t\t\ttoast.error(\"Could not copy verification code\", { description: \"Allow clipboard access or copy the displayed code manually.\" });\r\n\t\t\t\t\t\t\t\t\t\t}\r\n\t\t\t\t\t\t\t\t\t}}",
    "approved_event_handler_exception": true
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "chapter deletion feedback distinguishes local hide and pending signed review",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "toast.success(\"Chapter deleted successfully\", {\r\n\t\t\t\t\t\tdescription: \"It may take a few minutes for the change to be reflected.\"\r\n\t\t\t\t\t});",
    "after": "toast.success(\"Chapter hidden on this instance\", {\r\n\t\t\t\t\t\tdescription: res.headers?.[\"x-weebdex-source-deletion-status\"] === \"moderator-review-requested\"\r\n\t\t\t\t\t\t\t? \"Source deletion is awaiting moderator review. Other peer copies are not guaranteed to be removed.\"\r\n\t\t\t\t\t\t\t: \"Only this instance\u2019s catalogue entry was hidden. Other peer copies may remain.\"\r\n\t\t\t\t\t});"
  },
  {
    "path": "src/lib/HTTP.ts",
    "label": "HTTP retries only GET and settles cancellation or uncertain mutation exactly once",
    "sha256": "765a7ee6804ec81472c945e83a24b0b4343c0d58e8dbb2a2ab2421b681ba2fd1",
    "count": 1,
    "before": "import { StatusCodes } from \"http-status-codes\";\r\nimport { RateLimiter } from \"limiter\";\r\n\r\nexport class HTTPError {\r\n\tconstructor(\r\n\t\tpublic code: number,\r\n\t\tpublic error: string\r\n\t) {}\r\n}\r\n\r\nexport interface HTTPResponse<T = void> {\r\n\tstatus: number;\r\n\tbody?: T;\r\n}\r\n\r\nconst limiter = new RateLimiter({ tokensPerInterval: 5, interval: \"second\" });\r\n\r\nclass HTTP<T, U = unknown> {\r\n\tprivate xhr: XMLHttpRequest;\r\n\r\n\tprivate timeout = 0;\r\n\tprivate maxAttempts = 3;\r\n\tprivate isAlreadyOpened = false;\r\n\r\n\tprivate method: \"GET\" | \"POST\" | \"PATCH\" | \"PUT\" | \"DELETE\" = \"GET\";\r\n\tprivate headers: Record<string, string> = {};\r\n\r\n\tconstructor(\r\n\t\tprivate readonly url: string,\r\n\t\tprivate readonly body?: U\r\n\t) {\r\n\t\tthis.xhr = new XMLHttpRequest();\r\n\t}\r\n\r\n\tprivate open() {\r\n\t\tthis.xhr.open(this.method, this.url);\r\n\t\tfor (const [k, value] of Object.entries(this.headers)) {\r\n\t\t\tthis.xhr.setRequestHeader(k, value);\r\n\t\t}\r\n\t}\r\n\r\n\tprivate send(): Promise<HTTPResponse<T>> {\r\n\t\tlet attempts = 0;\r\n\t\treturn new Promise((resolve, reject) => {\r\n\t\t\tconst tryAttempt = () =>\r\n\t\t\t\tlimiter.removeTokens(1).then(() => {\r\n\t\t\t\t\tthis.open();\r\n\t\t\t\t\tattempts++;\r\n\r\n\t\t\t\t\tif (this.body) {\r\n\t\t\t\t\t\tif (this.body instanceof FormData) {\r\n\t\t\t\t\t\t\treturn this.xhr.send(this.body);\r\n\t\t\t\t\t\t}\r\n\r\n\t\t\t\t\t\tthis.xhr.setRequestHeader(\"Content-Type\", \"application/json\");\r\n\t\t\t\t\t\treturn this.xhr.send(JSON.stringify(this.body));\r\n\t\t\t\t\t}\r\n\t\t\t\t\tthis.xhr.send();\r\n\t\t\t\t});\r\n\r\n\t\t\tthis.xhr.addEventListener(\"load\", () => {\r\n\t\t\t\twindow.clearTimeout(this.timeout);\r\n\t\t\t\tif (this.xhr.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tif (attempts > this.maxAttempts) {\r\n\t\t\t\t\t\tresolve({ status: this.xhr.status });\r\n\t\t\t\t\t} else {\r\n\t\t\t\t\t\twindow.setTimeout(() => tryAttempt(), 1000);\r\n\t\t\t\t\t}\r\n\t\t\t\t} else {\r\n\t\t\t\t\tconst status = this.xhr.status;\r\n\t\t\t\t\tlet response: T | undefined;\r\n\r\n\t\t\t\t\tif (this.xhr.response) {\r\n\t\t\t\t\t\tconst ct = this.xhr.getResponseHeader(\"Content-Type\");\r\n\t\t\t\t\t\tif (ct?.includes(\"text/plain\")) {\r\n\t\t\t\t\t\t\tresponse = this.xhr.responseText as T;\r\n\t\t\t\t\t\t} else if (ct?.includes(\"application/json\")) {\r\n\t\t\t\t\t\t\ttry {\r\n\t\t\t\t\t\t\t\tresponse = JSON.parse(this.xhr.response);\r\n\t\t\t\t\t\t\t} catch {\r\n\t\t\t\t\t\t\t\tresponse = this.xhr.response;\r\n\t\t\t\t\t\t\t}\r\n\t\t\t\t\t\t} else {\r\n\t\t\t\t\t\t\tconsole.warn(\"[HTTP] unhandled response content-type\", ct);\r\n\t\t\t\t\t\t}\r\n\t\t\t\t\t}\r\n\t\t\t\t\tresolve({ status, body: response });\r\n\t\t\t\t}\r\n\t\t\t});\r\n\r\n\t\t\tthis.xhr.addEventListener(\"abort\", () => {\r\n\t\t\t\twindow.clearTimeout(this.timeout);\r\n\t\t\t\treject(new HTTPError(this.xhr.status, \"request has been aborted\"));\r\n\t\t\t});\r\n\r\n\t\t\tthis.xhr.addEventListener(\"error\", () => {\r\n\t\t\t\twindow.clearTimeout(this.timeout);\r\n\t\t\t\tif (attempts > this.maxAttempts) {\r\n\t\t\t\t\tresolve({ status: this.xhr.status });\r\n\t\t\t\t} else {\r\n\t\t\t\t\twindow.setTimeout(() => tryAttempt(), 1000);\r\n\t\t\t\t}\r\n\t\t\t});\r\n\t\t\ttryAttempt();\r\n\t\t});\r\n\t}\r\n\r\n\tGET() {\r\n\t\tif (this.isAlreadyOpened) {\r\n\t\t\treturn;\r\n\t\t}\r\n\t\tthis.method = \"GET\";\r\n\t\treturn this.send();\r\n\t}\r\n\r\n\tPOST() {\r\n\t\tif (this.isAlreadyOpened) {\r\n\t\t\treturn;\r\n\t\t}\r\n\t\tthis.method = \"POST\";\r\n\t\treturn this.send();\r\n\t}\r\n\r\n\tPATCH() {\r\n\t\tif (this.isAlreadyOpened) {\r\n\t\t\treturn;\r\n\t\t}\r\n\t\tthis.method = \"PATCH\";\r\n\t\treturn this.send();\r\n\t}\r\n\r\n\tPUT() {\r\n\t\tif (this.isAlreadyOpened) {\r\n\t\t\treturn;\r\n\t\t}\r\n\t\tthis.method = \"PUT\";\r\n\t\treturn this.send();\r\n\t}\r\n\r\n\tDELETE() {\r\n\t\tif (this.isAlreadyOpened) {\r\n\t\t\treturn;\r\n\t\t}\r\n\t\tthis.method = \"DELETE\";\r\n\t\treturn this.send();\r\n\t}\r\n\r\n\tsetWithCredentials(value: boolean) {\r\n\t\tthis.xhr.withCredentials = value;\r\n\t}\r\n\r\n\tsetRequestHeader(name: string, value: string): void {\r\n\t\tthis.headers[name] = value;\r\n\t}\r\n\r\n\tsetMaxAttempts(attempts: number) {\r\n\t\tthis.maxAttempts = attempts;\r\n\t}\r\n\r\n\taddEventListener<K extends keyof XMLHttpRequestEventTargetEventMap>(\r\n\t\tk: K,\r\n\t\tlistener: (ev: XMLHttpRequestEventTargetEventMap[K]) => void\r\n\t) {\r\n\t\tthis.xhr.addEventListener(k, listener);\r\n\t}\r\n\r\n\taddUploadListener<K extends keyof XMLHttpRequestEventTargetEventMap>(\r\n\t\tk: K,\r\n\t\tlistener: (ev: XMLHttpRequestEventTargetEventMap[K]) => void\r\n\t) {\r\n\t\tthis.xhr.upload.addEventListener(k, listener);\r\n\t}\r\n\r\n\tcancel() {\r\n\t\tthis.xhr.abort();\r\n\t}\r\n\r\n\ttick(timeout = 1000 * 60) {\r\n\t\twindow.clearTimeout(this.timeout);\r\n\t\tthis.timeout = window.setTimeout(() => this.xhr.abort(), timeout);\r\n\t}\r\n}\r\n\r\nexport default HTTP;\r\n",
    "after": "// Source-backed HTTP replacement: never replay an ambiguous mutation.\r\nimport { StatusCodes } from \"http-status-codes\";\r\nimport { RateLimiter } from \"limiter\";\r\nimport { Error as UserVisibleError } from \"$lib/constants/Error\";\r\n\r\nexport class HTTPError extends UserVisibleError {\r\n\tconstructor(public code: number, public error: string, title = \"Request failed\") {\r\n\t\tsuper(title, error);\r\n\t}\r\n}\r\nexport interface HTTPResponse<T = void> {\r\n\tstatus: number;\r\n\tbody?: T;\r\n\theaders?: Record<string, string | null>;\r\n}\r\n// Tokens are a refilling request budget, not permits held by an active request.\r\nconst limiter = new RateLimiter({ tokensPerInterval: 5, interval: \"second\" });\r\nclass HTTP<T, U = unknown> {\r\n\tprivate xhr: XMLHttpRequest;\r\n\tprivate timeout = 0;\r\n\tprivate timeoutMs = 60000;\r\n\tprivate maxAttempts = 3;\r\n\tprivate isAlreadyOpened = false;\r\n\tprivate cancelled = false;\r\n\tprivate finished = false;\r\n\tprivate cancelPending?: () => void;\r\n\tprivate deadlinePending?: () => void;\r\n\tprivate method: \"GET\" | \"POST\" | \"PATCH\" | \"PUT\" | \"DELETE\" = \"GET\";\r\n\tprivate headers: Record<string, string> = {};\r\n\tconstructor(private readonly url: string, private readonly body?: U) {\r\n\t\tthis.xhr = new XMLHttpRequest();\r\n\t}\r\n\tprivate open() {\r\n\t\tthis.xhr.open(this.method, this.url);\r\n\t\tfor (const [key, value] of Object.entries(this.headers)) this.xhr.setRequestHeader(key, value);\r\n\t}\r\n\tprivate send(): Promise<HTTPResponse<T>> {\r\n\t\tthis.isAlreadyOpened = true;\r\n\t\treturn new Promise((resolve, reject) => {\r\n\t\t\tlet attempts = 0;\r\n\t\t\tlet terminal = false;\r\n\t\t\tlet timer = 0;\r\n\t\t\tlet sentMutation = false;\r\n\t\t\tconst cleanup = () => {\r\n\t\t\t\twindow.clearTimeout(timer);\r\n\t\t\t\twindow.clearTimeout(this.timeout);\r\n\t\t\t\tthis.xhr.removeEventListener(\"load\", onLoad);\r\n\t\t\t\tthis.xhr.removeEventListener(\"error\", onTransportFailure);\r\n\t\t\t\tthis.xhr.removeEventListener(\"timeout\", onTransportFailure);\r\n\t\t\t\tthis.xhr.removeEventListener(\"abort\", onAbort);\r\n\t\t\t\tthis.cancelPending = this.deadlinePending = undefined;\r\n\t\t\t\tthis.finished = true;\r\n\t\t\t};\r\n\t\t\tconst fail = (reason: string) => {\r\n\t\t\t\tif (terminal) return;\r\n\t\t\t\tterminal = true;\r\n\t\t\t\tcleanup();\r\n\t\t\t\ttry { this.xhr.abort(); } catch { /* No pending network operation. */ }\r\n\t\t\t\treject(new HTTPError(0, sentMutation\r\n\t\t\t\t\t? `${reason} The operation may have succeeded. Reload and check the current state before submitting it again.`\r\n\t\t\t\t\t: reason, sentMutation ? \"Request outcome unknown\" : \"Request failed\"));\r\n\t\t\t};\r\n\t\t\tconst succeed = (response: HTTPResponse<T>) => {\r\n\t\t\t\tif (terminal) return;\r\n\t\t\t\tterminal = true; cleanup(); resolve(response);\r\n\t\t\t};\r\n\t\t\tconst schedule = () => {\r\n\t\t\t\tif (terminal || this.cancelled) return;\r\n\t\t\t\twindow.clearTimeout(timer);\r\n\t\t\t\ttimer = window.setTimeout(attempt, 1000);\r\n\t\t\t};\r\n\t\t\tconst onTransportFailure = () => {\r\n\t\t\t\tif (terminal) return;\r\n\t\t\t\tif (this.method === \"GET\" && !this.cancelled && attempts < this.maxAttempts) schedule();\r\n\t\t\t\telse fail(\"The connection ended before a reliable response was received.\");\r\n\t\t\t};\r\n\t\t\tconst onAbort = () => fail(\"The request was cancelled.\");\r\n\t\t\tconst onLoad = () => {\r\n\t\t\t\tif (terminal || this.cancelled) return;\r\n\t\t\t\tif (!this.xhr.status) return onTransportFailure();\r\n\t\t\t\tif (this.method === \"GET\" && this.xhr.status === StatusCodes.TOO_MANY_REQUESTS && attempts < this.maxAttempts) return schedule();\r\n\t\t\t\tlet body: T | undefined;\r\n\t\t\t\tconst contentType = this.xhr.getResponseHeader(\"Content-Type\") ?? \"\";\r\n\t\t\t\tif (this.xhr.response) {\r\n\t\t\t\t\tif (contentType.includes(\"text/plain\")) body = this.xhr.responseText as T;\r\n\t\t\t\t\telse if (contentType.includes(\"application/json\")) {\r\n\t\t\t\t\t\ttry { body = JSON.parse(this.xhr.response); } catch { body = this.xhr.response as T; }\r\n\t\t\t\t\t}\r\n\t\t\t\t}\r\n\t\t\t\tsucceed({ status: this.xhr.status, body, headers: {\r\n\t\t\t\t\t\"x-weebdex-instance-removal\": this.xhr.getResponseHeader(\"x-weebdex-instance-removal\"),\r\n\t\t\t\t\t\"x-weebdex-source-deletion-status\": this.xhr.getResponseHeader(\"x-weebdex-source-deletion-status\"),\r\n\t\t\t\t\t\"x-weebdex-source-deletion-request\": this.xhr.getResponseHeader(\"x-weebdex-source-deletion-request\"),\r\n\t\t\t\t\t\"x-weebdex-manga-version\": this.xhr.getResponseHeader(\"x-weebdex-manga-version\")\r\n\t\t\t\t} });\r\n\t\t\t};\r\n\t\t\tconst attempt = () => {\r\n\t\t\t\tif (terminal || this.cancelled) return;\r\n\t\t\t\t// Poll only our cancellable timer. Never queue an uncancellable limiter promise.\r\n\t\t\t\tif (!limiter.tryRemoveTokens(1)) {\r\n\t\t\t\t\ttimer = window.setTimeout(attempt, 250);\r\n\t\t\t\t\treturn;\r\n\t\t\t\t}\r\n\t\t\t\ttry {\r\n\t\t\t\t\tconst payload = this.body === undefined ? undefined : this.body instanceof FormData ? this.body : JSON.stringify(this.body);\r\n\t\t\t\t\tthis.open();\r\n\t\t\t\t\tif (terminal || this.cancelled) return;\r\n\t\t\t\t\tattempts++;\r\n\t\t\t\t\tif (payload !== undefined && !(payload instanceof FormData)) this.xhr.setRequestHeader(\"Content-Type\", \"application/json\");\r\n\t\t\t\t\tsentMutation ||= this.method !== \"GET\";\r\n\t\t\t\t\tthis.xhr.send(payload);\r\n\t\t\t\t} catch {\r\n\t\t\t\t\tfail(\"The request could not be completed.\");\r\n\t\t\t\t}\r\n\t\t\t};\r\n\t\t\tthis.cancelPending = () => fail(\"The request was cancelled.\");\r\n\t\t\tthis.deadlinePending = () => fail(\"The request timed out.\");\r\n\t\t\tthis.xhr.addEventListener(\"load\", onLoad);\r\n\t\t\tthis.xhr.addEventListener(\"error\", onTransportFailure);\r\n\t\t\tthis.xhr.addEventListener(\"timeout\", onTransportFailure);\r\n\t\t\tthis.xhr.addEventListener(\"abort\", onAbort);\r\n\t\t\tif (this.cancelled) return fail(\"The request was cancelled before it was sent.\");\r\n\t\t\tthis.tick(this.timeoutMs);\r\n\t\t\tattempt();\r\n\t\t});\r\n\t}\r\n\tGET() { if (this.isAlreadyOpened) return; this.method = \"GET\"; return this.send(); }\r\n\tPOST() { if (this.isAlreadyOpened) return; this.method = \"POST\"; return this.send(); }\r\n\tPATCH() { if (this.isAlreadyOpened) return; this.method = \"PATCH\"; return this.send(); }\r\n\tPUT() { if (this.isAlreadyOpened) return; this.method = \"PUT\"; return this.send(); }\r\n\tDELETE() { if (this.isAlreadyOpened) return; this.method = \"DELETE\"; return this.send(); }\r\n\tsetWithCredentials(value: boolean) { this.xhr.withCredentials = value; }\r\n\tsetRequestHeader(name: string, value: string) { this.headers[name] = value; }\r\n\tsetMaxAttempts(attempts: number) { this.maxAttempts = Number.isFinite(attempts) ? Math.max(1, Math.min(10, Math.floor(attempts))) : 3; }\r\n\taddEventListener<K extends keyof XMLHttpRequestEventTargetEventMap>(key: K, listener: (event: XMLHttpRequestEventTargetEventMap[K]) => void) { this.xhr.addEventListener(key, listener); }\r\n\taddUploadListener<K extends keyof XMLHttpRequestEventTargetEventMap>(key: K, listener: (event: XMLHttpRequestEventTargetEventMap[K]) => void) { this.xhr.upload.addEventListener(key, listener); }\r\n\tcancel() { this.cancelled = true; this.cancelPending?.(); }\r\n\ttick(timeout = 60000) {\r\n\t\tif (this.finished || this.cancelled) return;\r\n\t\tthis.timeoutMs = Number.isFinite(timeout) ? Math.max(1, Math.min(300000, timeout)) : 60000;\r\n\t\twindow.clearTimeout(this.timeout);\r\n\t\tif (this.deadlinePending) this.timeout = window.setTimeout(() => this.deadlinePending?.(), this.timeoutMs);\r\n\t}\r\n}\r\nexport default HTTP;\r\n"
  },
  {
    "path": "src/service-worker.ts",
    "label": "service worker imports network-only authority checks",
    "sha256": "9176298fc05fb1d60c9281ea67f0927abe41ee95c9eed7558346614819ab7cbc",
    "count": 1,
    "before": "import { CacheFirst, StaleWhileRevalidate } from \"workbox-strategies\";",
    "after": "import { CacheFirst, NetworkOnly, StaleWhileRevalidate } from \"workbox-strategies\";"
  },
  {
    "path": "src/service-worker.ts",
    "label": "authority-sensitive media is network-only and activation purges exact legacy covers cache",
    "sha256": "9176298fc05fb1d60c9281ea67f0927abe41ee95c9eed7558346614819ab7cbc",
    "count": 1,
    "before": "registerRoute(\r\n\t({ request }) => request.url.includes(\"/covers/\") && request.destination === \"image\",\r\n\tnew CacheFirst({\r\n\t\tcacheName: \"covers\",\r\n\t\tplugins: [\r\n\t\t\tnew CacheableResponsePlugin({ statuses: [200] }),\r\n\t\t\tnew ExpirationPlugin({\r\n\t\t\t\tmaxEntries: 1000,\r\n\t\t\t\tmaxAgeSeconds: 3600 * 24 * 7\r\n\t\t\t}),\r\n\t\t\t{\r\n\t\t\t\tcacheWillUpdate: async ({ response }) => {\r\n\t\t\t\t\ttry {\r\n\t\t\t\t\t\tif (response?.status !== 200) {\r\n\t\t\t\t\t\t\treturn null;\r\n\t\t\t\t\t\t}\r\n\r\n\t\t\t\t\t\tconst blob = await response.clone().blob();\r\n\t\t\t\t\t\tif (blob.size < 1024) {\r\n\t\t\t\t\t\t\treturn null;\r\n\t\t\t\t\t\t}\r\n\t\t\t\t\t} catch {\r\n\t\t\t\t\t\treturn null;\r\n\t\t\t\t\t}\r\n\t\t\t\t\treturn response;\r\n\t\t\t\t}\r\n\t\t\t}\r\n\t\t]\r\n\t}),\r\n\t\"GET\"\r\n);\r\n",
    "after": ""
  },
  {
    "path": "src/@types/Upload.d.ts",
    "label": "resumable upload fields are explicit owner-only DTO extensions",
    "sha256": "a5579edd66f0af5efe63f226e00912fbbd996fbae52b1ea30a09a70b9358db83",
    "count": 1,
    "before": "\t\tis_committed: bool;",
    "after": "\t\tstate?: \"open\" | \"committed\" | \"deleted\";\r\n\t\texpires_at?: string;\r\n\t\tis_expired?: boolean;\r\n\t\tcan_mutate?: boolean;\r\n\t\tcan_cancel?: boolean;\r\n\t\tcan_retry_commit?: boolean;\r\n\t\tpublication_configuration_available?: boolean;\r\n\t\tpublication_status?: string;\r\n\t\tpublication_event_id?: string;\r\n\t\ttarget_manga?: { id: string; title: string } | null;\r\n\t\ttarget_status?: string;\r\n\t\tmanga_id?: string;\r\n\t\tchapter_id?: string;\r\n\t\tdraft?: ChapterDraft | null;\r\n\t\tpage_order?: string[];\r\n\t\tcommitted_result?: Chapter & { publication_status?: string };\r\n\t\treceipt?: Chapter & { publication_status?: string };\r\n\t\tis_committed: bool;"
  },
  {
    "path": "src/lib/API/UploadSession.ts",
    "label": "GET upload returns complete authorized resumable DTO",
    "sha256": "cba4ff41e2098df2e3668c9d726387c9fba774e71e2ecdd656c50e364dcd734e",
    "count": 1,
    "before": "new HTTP<{ id?: string }>",
    "after": "new HTTP<UploadSession>"
  },
  {
    "path": "src/routes/(app)/title/[id]/upload/+page.svelte",
    "label": "upload page never deletes staged work on navigation",
    "sha256": "0625ce874f8b332aec8e742919634676cf58e2b0a8dd78225dcc928425f607d0",
    "count": 1,
    "before": "await Promise.all([getUser(), deleteExistingUploadSession()]);",
    "after": "await getUser();"
  },
  {
    "path": "src/routes/(app)/title/[id]/upload/+page.svelte",
    "label": "remove obsolete automatic upload deletion path",
    "sha256": "0625ce874f8b332aec8e742919634676cf58e2b0a8dd78225dcc928425f607d0",
    "count": 1,
    "before": "\tconst deleteExistingUploadSession = async () => {\r\n\t\tconst res = await API.UploadSession.Get();\r\n\t\tif (res?.status === StatusCodes.OK && res.body?.id) {\r\n\t\t\tawait API.UploadSession.Delete(res.body.id);\r\n\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t} else if (res?.status !== StatusCodes.NO_CONTENT) {\r\n\t\t\tthrow Constants.Error.Unknown;\r\n\t\t}\r\n\t};",
    "after": ""
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "upload restoration imports private versioned adapter",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "\timport API from \"$lib/API\";",
    "after": "\timport API from \"$lib/API\";\r\n\timport { currentUpload, prepareResume, snapshot, signature, saveResume, acknowledge, immutableCommit, resumeNotice, cancelResume, resumeError } from \"$platform/upload-resume\";"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "upload unsaved navigation guard uses existing app lifecycle",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "import { goto } from \"$app/navigation\";",
    "after": "import { beforeNavigate, goto } from \"$app/navigation\";"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "upload restoration imports private versioned adapter",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "\timport API from \"$lib/API\";",
    "after": "\timport API from \"$lib/API\";\r\n\timport { currentUpload, prepareResume, snapshot, signature, saveResume, acknowledge, immutableCommit, resumeNotice, cancelResume, resumeError } from \"$platform/upload-resume\";"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "upload unsaved navigation guard uses existing app lifecycle",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "import { goto } from \"$app/navigation\";",
    "after": "import { beforeNavigate, goto } from \"$app/navigation\";"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "new chapter restoration lifecycle import",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "\timport { toast } from \"svelte-sonner\";",
    "after": "\timport { toast } from \"svelte-sonner\";\r\n\timport { onMount } from \"svelte\";"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "new chapter initial GET must resolve before staging starts",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "\tlet draft = $state<ChapterDraft>({});",
    "after": "\tlet draft = $state<ChapterDraft>({});\r\n\tlet resumeReady = $state(false);"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "immutable or unavailable publication cannot be edited/recommitted",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "\t\tisTemsAccepted &&",
    "after": "\t\tresumeReady && uploadSession?.can_retry_commit !== false && !uploadSession?.is_committed && isTemsAccepted &&"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "begin waits for restoration rather than race-delete staged work",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "\t\t\tif (uploadSession) {",
    "after": "\t\t\tif (!resumeReady || uploadSession) {"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "restore autosave and durable receipt reconciliation without markup changes",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "\tconst isPending = async () => {",
    "after": "\tlet lastSavedUpload = $state(\"\");\r\n\tlet resumeSaveTimer = 0;\r\n\tlet resumeDisposed = false;\r\n\tconst showResumeError = (error: unknown) => {\r\n\t\tif (!owner.alive()) return;\r\n\t\tif (error instanceof Constants.Error.Error) toast.error(error.title, { description: error.body });\r\n\t\telse toast.error(\"Upload could not be restored\", { description: \"Reload before creating another upload. Existing staging was kept.\" });\r\n\t};\r\n\tconst restoreUpload = (saved: UploadSession) => {\r\n\t\towner.assert();\r\n\t\tuploadSession = prepareResume(saved);\r\n\t\tdraft = structuredClone(saved.draft ?? {});\r\n\t\tgroups = saved.relationships.groups ?? [];\r\n\t\tisOneshot = !draft.volume && !draft.chapter;\r\n\t\tlastSavedUpload = signature(snapshot(draft, saved.relationships.files ?? []));\r\n\t\tif (saved.is_committed) {\r\n\t\t\tconst result = saved.committed_result ?? saved.receipt;\r\n\t\t\tchapter = result?.relationships?.manga && ![\"accepted_pending_projection\", \"pending_projection\"].includes(result.publication_status ?? \"\") ? result : undefined;\r\n\t\t\tmessage = { title: chapter ? \"Saved chapter restored\" : \"Signed chapter accepted\", description: chapter ? \"This chapter has been materialized on this instance.\" : \"The saved publication is waiting for local catalogue processing before it can be read.\" };\r\n\t\t\tisBeginAnotherUploadSession = false;\r\n\t\t\tisShowMessage = true;\r\n\t\t} else toast.info(\"Upload session restored\", { description: resumeNotice(saved) });\r\n\t};\r\n\tconst persistCurrentUpload = async () => {\r\n\t\tif (resumeDisposed || !uploadSession || uploadSession.can_mutate === false) return;\r\n\t\tconst files = uploadSession.relationships.files ?? [];\r\n\t\tif (files.some(file => file.file || file.error || file.id.length !== 10)) return;\r\n\t\tconst value = snapshot($state.snapshot(draft), files);\r\n\t\tif (signature(value) === lastSavedUpload) return;\r\n\t\tconst saved = await saveResume(uploadSession, value, owner);\r\n\t\tif (resumeDisposed || uploadSession?.id !== saved.id) return;\r\n\t\tacknowledge(uploadSession, saved);\r\n\t\tlastSavedUpload = signature(value);\r\n\t};\r\n\tconst reconcileUploadFailure = async () => {\r\n\t\ttry {\r\n\t\t\tconst saved = await currentUpload(owner);\r\n\t\t\tif (saved && (saved.manga_id ?? saved.relationships.manga?.id) === manga.id && (!saved.chapter_id || saved.is_committed) && (saved.can_mutate === false || !uploadSession)) restoreUpload(saved);\r\n\t\t} catch { /* Preserve original uncertainty; never replay a mutation. */ }\r\n\t};\r\n\t$effect(() => {\r\n\t\tif (!resumeReady || !uploadSession || uploadSession.can_mutate === false) return;\r\n\t\tconst value = snapshot(draft, uploadSession.relationships.files ?? []);\r\n\t\tif (signature(value) === lastSavedUpload) return;\r\n\t\twindow.clearTimeout(resumeSaveTimer);\r\n\t\tresumeSaveTimer = window.setTimeout(() => { void mutex.wait(persistCurrentUpload).catch(showResumeError); }, 500);\r\n\t\treturn () => window.clearTimeout(resumeSaveTimer);\r\n\t});\r\n\tconst hasUnsavedUpload = () => !!uploadSession && uploadSession.can_mutate !== false && signature(snapshot(draft, uploadSession.relationships.files ?? [])) !== lastSavedUpload;\r\n\tbeforeNavigate(navigation => {\r\n\t\tif (!navigation.willUnload && hasUnsavedUpload()) {\r\n\t\t\tnavigation.cancel();\r\n\t\t\ttoast.info(\"Upload changes are still saving\", { description: \"Wait for the private staging save before leaving. No session was discarded.\" });\r\n\t\t}\r\n\t});\r\n\tonMount(() => {\r\n\t\tconst warnUnsaved = (event: BeforeUnloadEvent) => { if (hasUnsavedUpload()) { event.preventDefault(); event.returnValue = \"\"; } };\r\n\t\twindow.addEventListener(\"beforeunload\", warnUnsaved);\r\n\t\tvoid mutex.wait(async () => {\r\n\t\t\tconst saved = await currentUpload(owner);\r\n\t\t\tif (resumeDisposed) return;\r\n\t\t\tif (saved) {\r\n\t\t\t\tif ((saved.manga_id ?? saved.relationships.manga?.id) !== manga.id || (saved.chapter_id && !saved.is_committed)) {\r\n\t\t\t\t\ttoast.info(\"Existing upload kept\", { description: \"Opening its original editor instead of deleting staged work.\" });\r\n\t\t\t\t\tawait goto(saved.chapter_id ? `/chapter/${saved.chapter_id}/edit` : `/title/${saved.manga_id ?? saved.relationships.manga!.id}/upload`);\r\n\t\t\t\t\treturn;\r\n\t\t\t\t}\r\n\t\t\t\trestoreUpload(saved);\r\n\t\t\t}\r\n\t\t\tresumeReady = true;\r\n\t\t}).catch(showResumeError);\r\n\t\treturn () => { resumeDisposed = true; window.clearTimeout(resumeSaveTimer); window.removeEventListener(\"beforeunload\", warnUnsaved); };\r\n\t});\r\n\r\n\tconst isPending = async () => {"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "commit flushes private autosave or exact immutable durable payload",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "\t\t\t\tconst res = await API.UploadSession.Commit(uploadSession.id, {\r\n\t\t\t\t\tdraft,\r\n\t\t\t\t\tpage_order: uploadSession.relationships.files.map(e => e.id)\r\n\t\t\t\t});",
    "after": "\t\t\t\tawait persistCurrentUpload();\r\n\t\t\t\tconst payload = immutableCommit(uploadSession, snapshot($state.snapshot(draft), uploadSession.relationships.files));\r\n\t\t\t\tconst res = await API.UploadSession.Commit(uploadSession.id, payload);"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "committed uploads stop private autosave and duplicate submission",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "\t\t\t\t\tuploadSession.is_committed = true;",
    "after": "\t\t\t\t\tuploadSession.is_committed = true;\r\n\t\t\t\t\tuploadSession.can_mutate = false;\r\n\t\t\t\t\tuploadSession.can_retry_commit = false;"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "ambiguous begin or commit reconciles real session without replay",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 2,
    "before": "\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error)",
    "after": "\t\t\t} catch (e) {\r\n\t\t\t\tawait reconcileUploadFailure();\r\n\t\t\t\tif (e instanceof Constants.Error.Error)"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "chapter page edit restores existing staging on mount",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "\tonMount(() => onReset());",
    "after": "\tlet resumeReady = $state(false);\r\n\tlet lastSavedUpload = $state(\"\");\r\n\tlet resumeSaveTimer = 0;\r\n\tlet resumeDisposed = false;\r\n\tconst showResumeError = (error: unknown) => {\r\n\t\tif (!owner.alive()) return;\r\n\t\tif (error instanceof Constants.Error.Error) toast.error(error.title, { description: error.body });\r\n\t\telse toast.error(\"Upload could not be restored\", { description: \"Existing staging was kept. Reload its current state before retrying.\" });\r\n\t};\r\n\tconst restoreUpload = (saved: UploadSession) => {\r\n\t\towner.assert();\r\n\t\tuploadSession = prepareResume(saved);\r\n\t\tlastSavedUpload = signature(snapshot(saved.draft ?? chapter, saved.relationships.files ?? []));\r\n\t\ttoast.info(\"Upload session restored\", { description: resumeNotice(saved) });\r\n\t};\r\n\tconst persistCurrentUpload = async () => {\r\n\t\tif (resumeDisposed || !uploadSession || uploadSession.can_mutate === false) return;\r\n\t\tconst files = uploadSession.relationships.files ?? [];\r\n\t\tif (files.some(file => file.file || file.error || file.id.length !== 10)) return;\r\n\t\tconst value = snapshot(chapter, files);\r\n\t\tif (signature(value) === lastSavedUpload) return;\r\n\t\tconst saved = await saveResume(uploadSession, value, owner);\r\n\t\tif (resumeDisposed || uploadSession?.id !== saved.id) return;\r\n\t\tacknowledge(uploadSession, saved); lastSavedUpload = signature(value);\r\n\t};\r\n\t$effect(() => {\r\n\t\tif (!resumeReady || !uploadSession || uploadSession.can_mutate === false) return;\r\n\t\tif (signature(snapshot(chapter, uploadSession.relationships.files ?? [])) === lastSavedUpload) return;\r\n\t\twindow.clearTimeout(resumeSaveTimer);\r\n\t\tresumeSaveTimer = window.setTimeout(() => { void mutex.wait(persistCurrentUpload).catch(showResumeError); }, 500);\r\n\t\treturn () => window.clearTimeout(resumeSaveTimer);\r\n\t});\r\n\tconst hasUnsavedUpload = () => !!uploadSession && uploadSession.can_mutate !== false && signature(snapshot(chapter, uploadSession.relationships.files ?? [])) !== lastSavedUpload;\r\n\tbeforeNavigate(navigation => {\r\n\t\tif (!navigation.willUnload && hasUnsavedUpload()) { navigation.cancel(); toast.info(\"Upload changes are still saving\", { description: \"Wait for the private staging save before leaving.\" }); }\r\n\t});\r\n\tonMount(() => {\r\n\t\tconst warnUnsaved = (event: BeforeUnloadEvent) => { if (hasUnsavedUpload()) { event.preventDefault(); event.returnValue = \"\"; } };\r\n\t\twindow.addEventListener(\"beforeunload\", warnUnsaved);\r\n\t\tvoid mutex.wait(async () => {\r\n\t\t\tawait onReset();\r\n\t\t\tconst saved = await currentUpload(owner);\r\n\t\t\tif (resumeDisposed) return;\r\n\t\t\tif (saved?.chapter_id === chapter.id) restoreUpload(saved);\r\n\t\t\tresumeReady = true;\r\n\t\t}).catch(showResumeError);\r\n\t\treturn () => { resumeDisposed = true; window.clearTimeout(resumeSaveTimer); window.removeEventListener(\"beforeunload\", warnUnsaved); };\r\n\t});\r\n"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "restored page edit respects retry eligibility",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "\t\tif (uploadSession) {\r\n\t\t\treturn (",
    "after": "\t\tif (uploadSession) {\r\n\t\t\tif (!resumeReady || uploadSession.can_retry_commit === false || uploadSession.is_committed) return false;\r\n\t\t\treturn ("
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "page edit begins only after authoritative restore",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "\t\t\tif (uploadSession) {\r\n\t\t\t\treturn;",
    "after": "\t\t\tif (!resumeReady || uploadSession) {\r\n\t\t\t\treturn;"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "page edit never destroys another resumable session",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "\t\t\t\tawait deleteExistingUploadSession();",
    "after": "\t\t\t\tawait deleteExistingUploadSession();\r\n\t\t\t\tif (uploadSession) return;"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "cancellation requires server204 and never discards409 state",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "\tconst deleteExistingUploadSession = async () => {\r\n\t\tconst res = await API.UploadSession.Get();\r\n\t\tif (res?.status === StatusCodes.OK && res.body?.id) {\r\n\t\t\tawait API.UploadSession.Delete(res.body.id);\r\n\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t} else if (res?.status !== StatusCodes.NO_CONTENT) {\r\n\t\t\tthrow Constants.Error.Unknown;\r\n\t\t}\r\n\t};\r\n\r\n\tconst cancelUploadSession = () =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tif (!uploadSession) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tawait API.UploadSession.Delete(uploadSession.id);\r\n\t\t\tuploadSession = undefined;\r\n\t\t});",
    "after": "\tconst deleteExistingUploadSession = async () => {\r\n\t\tconst saved = await currentUpload(owner);\r\n\t\tif (!saved) return;\r\n\t\tif (saved.chapter_id === chapter.id) { restoreUpload(saved); return; }\r\n\t\tthrow new Constants.Error.Error(\"Existing upload kept\", \"Resume or explicitly cancel the staged upload from its original title/editor before starting another. No files were deleted.\");\r\n\t};\r\n\tconst cancelUploadSession = () => mutex.exec(async () => {\r\n\t\tif (!uploadSession) return;\r\n\t\ttry { await cancelResume(uploadSession, owner); uploadSession = undefined; }\r\n\t\tcatch (error) { showResumeError(error); }\r\n\t});"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "page commit flushes saved order or exact durable immutable payload",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "\t\t\t\tconst res = await API.UploadSession.Commit(uploadSession.id, {\r\n\t\t\t\t\tdraft: {\r\n\t\t\t\t\t\tvolume: chapter.volume,\r\n\t\t\t\t\t\tchapter: chapter.chapter,\r\n\t\t\t\t\t\ttitle: chapter.title,\r\n\t\t\t\t\t\tlanguage: chapter.language\r\n\t\t\t\t\t},\r\n\t\t\t\t\tpage_order: uploadSession.relationships.files.map(e => e.id)\r\n\t\t\t\t});",
    "after": "\t\t\t\tawait persistCurrentUpload();\r\n\t\t\t\tconst payload = immutableCommit(uploadSession, snapshot(chapter, uploadSession.relationships.files));\r\n\t\t\t\tconst res = await API.UploadSession.Commit(uploadSession.id, payload);"
  },
  {
    "path": "src/lib/components/upload/UploadChapterMetadata.svelte",
    "label": "metadata derives authority lock through existing disabled prop",
    "sha256": "c94de7465c23c046f6277dae37266b845b62ba8c5097980a3629014f6f3d7ff9",
    "count": 1,
    "before": "\timport { Locale } from \"$lib\";",
    "after": "\timport { Locale } from \"$lib\";\r\n\timport { getUploadSession } from \"$lib/Context\";"
  },
  {
    "path": "src/lib/components/upload/UploadChapterMetadata.svelte",
    "label": "prepared publication locks existing editing controls",
    "sha256": "c94de7465c23c046f6277dae37266b845b62ba8c5097980a3629014f6f3d7ff9",
    "count": 1,
    "before": "\t\tdisabled\r\n\t}: {",
    "after": "\t\tdisabled: propDisabled\r\n\t}: {"
  },
  {
    "path": "src/lib/components/upload/UploadChapterMetadata.svelte",
    "label": "metadata uses context mutation authority without DOM change",
    "sha256": "c94de7465c23c046f6277dae37266b845b62ba8c5097980a3629014f6f3d7ff9",
    "count": 1,
    "before": "\t} = $props();",
    "after": "\t} = $props();\r\n\tconst getSession = getUploadSession();\r\n\tconst disabled = $derived(propDisabled || getSession()?.can_mutate === false);"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "prepared publication locks existing editing controls",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\t\tdisabled\r\n\t}: {",
    "after": "\t\tdisabled: propDisabled\r\n\t}: {"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "page controls use context mutation authority without DOM change",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\tconst uploadSession = $derived(getUploadSession()());",
    "after": "\tconst uploadSession = $derived(getUploadSession()());\r\n\tconst disabled = $derived(propDisabled || uploadSession?.can_mutate === false);"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPagesEntry.svelte",
    "label": "prepared publication locks existing editing controls",
    "sha256": "87bce9e9394fd6301e81821ea4aa78e661b14aae6f5616db249a938f11879fad",
    "count": 1,
    "before": "\t\tdisabled\r\n\t}: {",
    "after": "\t\tdisabled: propDisabled\r\n\t}: {"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPagesEntry.svelte",
    "label": "page controls use context mutation authority without DOM change",
    "sha256": "87bce9e9394fd6301e81821ea4aa78e661b14aae6f5616db249a938f11879fad",
    "count": 1,
    "before": "\tconst uploadSession = $derived(getUploadSession()());",
    "after": "\tconst uploadSession = $derived(getUploadSession()());\r\n\tconst disabled = $derived(propDisabled || uploadSession?.can_mutate === false);"
  },
  {
    "path": "src/service-worker.ts",
    "label": "network-only authority route precedes all broad legacy cache predicates",
    "sha256": "9176298fc05fb1d60c9281ea67f0927abe41ee95c9eed7558346614819ab7cbc",
    "count": 1,
    "before": "clientsClaim();",
    "after": "clientsClaim();\r\n\r\n// Authority-dependent bytes must be rechecked against the server on every read.\r\n// Purge exact legacy attachment caches that could contain authority-sensitive URLs.\r\n// Never delete IndexedDB, local library data, or immutable assets.\r\nself.addEventListener(\"activate\", event => {\r\n\tevent.waitUntil(Promise.all([caches.delete(\"covers\"), caches.delete(\"favicons\")]));\r\n});\r\nregisterRoute(\r\n\t({ url }) => url.origin === self.location.origin && (\r\n\t\t[\"/covers/\", \"/data/\", \"/uploads/\", \"/originals/\", \"/api/\", \"/auth/\"].some(prefix => url.pathname.startsWith(prefix)) ||\r\n\t\t[\"/proxy\", \"/og-image\"].some(prefix => url.pathname === prefix || url.pathname.startsWith(prefix + \"/\"))\r\n\t),\r\n\tnew NetworkOnly({ fetchOptions: { cache: \"no-store\" } }),\r\n\t\"GET\"\r\n);\r\n"
  },
  {
    "path": "src/lib/hooks/persistent-store.svelte.ts",
    "label": "device preferences tolerate corrupt storage and persist update callbacks",
    "sha256": "60a856fb50feba335c96517c5a8e419038b758ff12a15aeb73f0bc3bbde810fc",
    "count": 1,
    "before": "import { browser } from \"$app/environment\";\r\nimport { writable, type Subscriber, type Writable } from \"svelte/store\";\r\n\r\nexport class PersistentStore<T> {\r\n\tprivate readonly key: string;\r\n\tprivate readonly w: Writable<T>;\r\n\tprivate readonly origSet: (value: T) => void;\r\n\r\n\tconstructor(key: string, initialValue: T) {\r\n\t\tif (browser) {\r\n\t\t\tconst localRaw = localStorage.getItem(key);\r\n\t\t\tconst localState: T = localRaw ? JSON.parse(localRaw) : initialValue;\r\n\t\t\tthis.key = key;\r\n\t\t\tthis.w = writable(localState);\r\n\t\t\tthis.origSet = this.w.set;\r\n\r\n\t\t\tif (!localRaw) {\r\n\t\t\t\tthis.set(initialValue);\r\n\t\t\t}\r\n\t\t}\r\n\t}\r\n\r\n\tsubscribe(run: Subscriber<T>) {\r\n\t\treturn this.w.subscribe(run);\r\n\t}\r\n\r\n\tset(value: T) {\r\n\t\tlocalStorage.setItem(this.key, JSON.stringify(value));\r\n\t\tthis.origSet(value);\r\n\t}\r\n\r\n\tupdate(updater: (value: T) => T) {\r\n\t\tthis.w.update(updater);\r\n\t}\r\n}\r\n",
    "after": "// Corrupt/denied browser storage must not crash app initialization or lose updates.\r\nimport { browser } from \"$app/environment\";\r\nimport { get, writable, type Subscriber, type Writable } from \"svelte/store\";\r\nimport { toast } from \"svelte-sonner\";\r\nfunction normalizeStored(value: unknown, defaults: unknown, key: string, field = \"\"): unknown {\r\n\tif (Array.isArray(defaults)) {\r\n\t\tif (!Array.isArray(value) || value.length > 1000) throw Error(\"Invalid stored preference array\");\r\n\t\tif ([\"uploaders\", \"groups\"].includes(field)) {\r\n\t\t\tif (value.some(row => !row || typeof row !== \"object\" || Array.isArray(row) || typeof row.id !== \"string\" || !row.id.length || row.id.length > 128 || row.name !== undefined && typeof row.name !== \"string\")) throw Error(\"Invalid stored preference identity\");\r\n\t\t} else if (value.some(row => typeof row !== \"string\" || row.length > 128)) throw Error(\"Invalid stored preference array member\");\r\n\t\treturn value;\r\n\t}\r\n\tif (defaults !== null && typeof defaults === \"object\") {\r\n\t\tif (!value || typeof value !== \"object\" || Array.isArray(value)) throw Error(\"Invalid stored preference object\");\r\n\t\tconst source = value as Record<string, unknown>;\r\n\t\treturn Object.fromEntries(Object.entries(defaults).map(([name, fallback]) => [name, name in source ? normalizeStored(source[name], fallback, key, name) : fallback]));\r\n\t}\r\n\tif (typeof value !== typeof defaults || value === null) throw Error(\"Invalid stored preference type\");\r\n\tif (typeof value === \"number\") {\r\n\t\tif (!Number.isFinite(value)) throw Error(\"Invalid stored preference number\");\r\n\t\tconst ranges: Record<string, [number, number]> = { scale: [0, 6], layout: [0, 1], direction: [0, 1], offset: [0, 10], width: [10, 100], gap: [0, 100], scroll: [5, 100], preload: [1, 100], parallel: [1, 10], version: [0, 2147483647] };\r\n\t\tconst range = ranges[field]; if (range && (!Number.isInteger(value) || value < range[0] || value > range[1])) throw Error(\"Invalid stored preference range\");\r\n\t\tif (key === \"whyxk2oq\" && ![0, 1, 2].includes(value)) throw Error(\"Invalid comments visibility\");\r\n\t}\r\n\tif (typeof value === \"string\") {\r\n\t\tif (value.length > 128 || field === \"items_per_page\" && (!/^[0-9]{1,3}$/.test(value) || Number(value) < 1 || Number(value) > 100) || field === \"panel_position\" && ![\"left\", \"right\"].includes(value) || [\"nrs2qcav\", \"a0yb25h8\"].includes(key) && ![\"grid\", \"list\"].includes(value) || key === \"ho5r3tmc\" && ![\"settings\", \"chapters\", \"groups\", \"comments\"].includes(value)) throw Error(\"Invalid stored preference choice\");\r\n\t}\r\n\treturn value;\r\n}\r\nexport class PersistentStore<T> {\r\n\tprivate readonly key: string;\r\n\tprivate readonly w: Writable<T>;\r\n\tconstructor(key: string, initialValue: T) {\r\n\t\tthis.key = key;\r\n\t\tlet value = initialValue;\r\n\t\tlet absent = false;\r\n\t\tif (browser) {\r\n\t\t\ttry {\r\n\t\t\t\tconst raw = localStorage.getItem(key);\r\n\t\t\t\tabsent = raw === null;\r\n\t\t\t\tif (raw !== null) value = normalizeStored(JSON.parse(raw), initialValue, key) as T;\r\n\t\t\t} catch {\r\n\t\t\t\t// Keep malformed old bytes for recovery until an explicit subsequent edit.\r\n\t\t\t\tconsole.warn(\"Stored device preferences could not be read; defaults are active.\");\r\n\t\t\t}\r\n\t\t}\r\n\t\tthis.w = writable(value);\r\n\t\tif (browser && absent) this.set(initialValue);\r\n\t}\r\n\tsubscribe(run: Subscriber<T>) { return this.w.subscribe(run); }\r\n\tset(value: T) {\r\n\t\tif (browser) {\r\n\t\t\ttry { localStorage.setItem(this.key, JSON.stringify(value)); }\r\n\t\t\tcatch { toast.error(\"Device preferences could not be saved\", { description: \"The current value is active only in this page. Browser storage was denied or full.\" }); }\r\n\t\t}\r\n\t\tthis.w.set(value);\r\n\t}\r\n\tupdate(updater: (value: T) => T) { this.set(updater(get(this.w))); }\r\n}\r\n"
  },
  {
    "path": "src/lib/components/reader/reader.svelte",
    "label": "reader data JSON escapes script-breaking characters without changing template",
    "sha256": "0b5ea1b1d3cb2ffbf818117e8f72d46218cd60b9b72e1539b864fb082e7ccf1d",
    "count": 1,
    "before": "\tconst reader = useReader();",
    "after": "\t// Scope the existing template's JSON name; no raw '<' can terminate its data script.\r\n\tconst JSON = { stringify: (value: unknown) => globalThis.JSON.stringify(value).replace(/</g, \"\\\\u003c\").replace(/\\u2028/g, \"\\\\u2028\").replace(/\\u2029/g, \"\\\\u2029\") };\r\n\tconst reader = useReader();"
  },
  {
    "path": "src/routes/+layout.ts",
    "label": "all external Markdown images use bounded proxy and local media retains authority checks",
    "sha256": "8e56aba992c24f457df79e2345a62fc9a819d31976290ee8a5fb8fbf9ce14c65",
    "count": 1,
    "before": "\t\t\tif (isSafeUrl(src)) {\r\n\t\t\t\tif (src.includes(\"://i.imgur.com\")) {\r\n\t\t\t\t\tnode.setAttribute(\"src\", src);\r\n\t\t\t\t} else {\r\n\t\t\t\t\tnode.setAttribute(\"src\", `${PUBLIC_PROXY_URL}?image=${encodeURIComponent(src)}`);\r\n\t\t\t\t}\r\n\t\t\t}",
    "after": "\t\t\tif (src && isSafeUrl(src)) {\r\n\t\t\t\ttry {\r\n\t\t\t\t\tconst image = new URL(src, window.location.origin);\r\n\t\t\t\t\tif ([\"https:\", \"http:\"].includes(image.protocol) && !image.username && !image.password) {\r\n\t\t\t\t\t\tconst local = image.origin === window.location.origin && [\"/covers/\", \"/data/\", \"/uploads/\", \"/originals/\", \"/images/\", \"/assets/\"].some(prefix => image.pathname.startsWith(prefix));\r\n\t\t\t\t\t\tnode.setAttribute(\"src\", local ? image.href : `${PUBLIC_PROXY_URL}?image=${encodeURIComponent(image.href)}`);\r\n\t\t\t\t\t}\r\n\t\t\t\t} catch { /* Malformed images remain without a source. */ }\r\n\t\t\t}"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPagesEntry.svelte",
    "label": "page removal shares parent upload mutex with autosave and commit",
    "sha256": "87bce9e9394fd6301e81821ea4aa78e661b14aae6f5616db249a938f11879fad",
    "count": 1,
    "before": "import { getUploadSession } from \"$lib/Context\";",
    "after": "import { getMutex, getUploadSession } from \"$lib/Context\";"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPagesEntry.svelte",
    "label": "page removal cannot race parent commit",
    "sha256": "87bce9e9394fd6301e81821ea4aa78e661b14aae6f5616db249a938f11879fad",
    "count": 1,
    "before": "\tconst mutex = new Mutex();",
    "after": "\tconst mutex = getMutex();"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPagesEntry.svelte",
    "label": "page mutation rechecks prepared authority even for programmatic events",
    "sha256": "87bce9e9394fd6301e81821ea4aa78e661b14aae6f5616db249a938f11879fad",
    "count": 1,
    "before": "\t\t\tif (!uploadSession) {",
    "after": "\t\t\tif (!uploadSession || uploadSession.can_mutate === false) {"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "prepared uploads cannot reorder through retained drag handlers",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\t\tpages = e.detail.items;",
    "after": "\t\tif (disabled || mutex.isLocked() || uploadSession?.can_mutate === false) return;\r\n\t\tpages = e.detail.items;"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "drag enable obeys authority-disabled without markup changes",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "const createDragToggleHandler = (shouldEnable: boolean) => () => (isDragDisabled = !shouldEnable);",
    "after": "const createDragToggleHandler = (shouldEnable: boolean) => () => (isDragDisabled = !shouldEnable || !!disabled || uploadSession?.can_mutate === false);"
  },
  {
    "path": "src/routes/(app)/title/[id]/upload/+page.ts",
    "label": "private upload loader restores only current owner staging when public target unavailable",
    "sha256": "861dd8ef4f908b988334787a74652cda308697a94ef767821fe97bbdd801ef7f",
    "count": 1,
    "before": "import { API } from \"$lib\";\r\nimport { error } from \"@sveltejs/kit\";\r\nimport { StatusCodes } from \"http-status-codes\";\r\n\r\nexport const load = async ({ params }) => {\r\n\tconst res = await API.Manga.Get(params.id);\r\n\tif (res && res.status !== StatusCodes.OK) {\r\n\t\terror(res.status);\r\n\t}\r\n\treturn {\r\n\t\tid: params.id,\r\n\t\tmanga: res?.body\r\n\t};\r\n};\r\n",
    "after": "import { API } from \"$lib\";\r\nimport { error } from \"@sveltejs/kit\";\r\nimport { StatusCodes } from \"http-status-codes\";\r\nexport const load = async ({ params }) => {\r\n\tconst res = await API.Manga.Get(params.id);\r\n\tif (res?.status === StatusCodes.NOT_FOUND) {\r\n\t\tconst owner = await API.UploadSession.Get();\r\n\t\tif (owner?.status === StatusCodes.OK && owner.body?.manga_id === params.id) {\r\n\t\t\tconst target = owner.body.target_manga;\r\n\t\t\treturn { id: params.id, manga: (target ?? { id: params.id, title: \"Unavailable staged title\" }) as Manga };\r\n\t\t}\r\n\t}\r\n\tif (res && res.status !== StatusCodes.OK) error(res.status);\r\n\treturn { id: params.id, manga: res?.body };\r\n};\r\n"
  },
  {
    "path": "src/routes/(app)/title/[id]/upload/+layout.svelte",
    "label": "verified owner can view retained staging without acquiring new publishing permissions",
    "sha256": "a6833f58f3069475244bedbeb29125540baca071c3e20a979321bc32f67b659c",
    "count": 1,
    "before": "import { Permission } from \"$lib\";",
    "after": "import { API, Permission as RetainedPermission } from \"$lib\";\r\n\timport { page } from \"$app/state\";\r\n\timport { onMount } from \"svelte\";"
  },
  {
    "path": "src/routes/(app)/title/[id]/upload/+layout.svelte",
    "label": "private resume permission facade never replaces auth Session",
    "sha256": "a6833f58f3069475244bedbeb29125540baca071c3e20a979321bc32f67b659c",
    "count": 1,
    "before": "\tconst { children } = $props();",
    "after": "\tconst { children } = $props();\r\n\tlet hasOwnedStaging = $state(false);\r\n\tconst Permission = { Chapter: { canCreate: () => RetainedPermission.Chapter.canCreate() || hasOwnedStaging } };\r\n\tonMount(() => {\r\n\t\tlet disposed = false;\r\n\t\tvoid (async () => {\r\n\t\t\tif (!(await Session.isValid())) return;\r\n\t\t\tconst saved = await API.UploadSession.Get();\r\n\t\t\tif (!disposed) hasOwnedStaging = saved?.status === 200 && saved.body?.manga_id === page.params.id;\r\n\t\t})().catch(() => { /* Existing protected UI remains conservative. */ });\r\n\t\treturn () => { disposed = true; };\r\n\t});"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "archive admission bounds compressed bytes entries streamed output and remaining page budget",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\tconst extractArchive = async (file: File) => {\r\n\t\tconst files: File[] = [];\r\n\t\ttry {\r\n\t\t\tconst zip = await JSZip.loadAsync(file);\r\n\t\t\tfor (const e of Object.values(zip.files)\r\n\t\t\t\t.filter(e => !e.dir && /\\.(jpg|jpeg|png|gif)$/i.test(e.name))\r\n\t\t\t\t.sort((a, b) =>\r\n\t\t\t\t\ta.name.localeCompare(b.name, undefined, {\r\n\t\t\t\t\t\tnumeric: true,\r\n\t\t\t\t\t\tsensitivity: \"base\"\r\n\t\t\t\t\t})\r\n\t\t\t\t)) {\r\n\t\t\t\tconst blob = await e.async(\"blob\");\r\n\t\t\t\tif (blob.size > maxFileSize) {\r\n\t\t\t\t\ttoast.error(e.name, { description: \"File too large\" });\r\n\t\t\t\t\tcontinue;\r\n\t\t\t\t}\r\n\t\t\t\tfiles.push(new File([blob], e.name));\r\n\t\t\t}\r\n\t\t} catch (e) {\r\n\t\t\tif (e instanceof Constants.Error.Error) throw e;\r\n\t\t\telse new Constants.Error.Error(file.name, \"Failed to extract archive\");\r\n\t\t}\r\n\r\n\t\treturn files;\r\n\t};\r\n",
    "after": "\tconst maxSessionFiles = 500;\r\n\tconst maxArchiveCompressedSize = 100 * 1024 * 1024;\r\n\tconst inspectArchiveDirectory = async (file: File) => {\r\n\t\t// Admit standard single-disk ZIP centrally, before JSZip allocates entry objects.\r\n\t\tconst start = Math.max(0, file.size - 65557);\r\n\t\tconst tail = new Uint8Array(await file.slice(start).arrayBuffer());\r\n\t\tconst view = new DataView(tail.buffer);\r\n\t\tlet end = -1;\r\n\t\tfor (let offset = tail.length - 22; offset >= 0; offset--) {\r\n\t\t\tif (view.getUint32(offset, true) === 0x06054b50 && offset + 22 + view.getUint16(offset + 20, true) === tail.length) { end = offset; break; }\r\n\t\t}\r\n\t\tif (end < 0) throw new Constants.Error.Error(file.name, \"Failed to extract archive: no bounded standard ZIP directory.\");\r\n\t\t// JSZip chooses the final EOCD signature; reject any competing marker after ours.\r\n\t\tfor (let cursor = end + 4; cursor + 4 <= tail.length; cursor++) {\r\n\t\t\tif (view.getUint32(cursor, true) === 0x06054b50) throw new Constants.Error.Error(file.name, \"Ambiguous ZIP end-directory markers are not supported by the bounded importer.\");\r\n\t\t}\r\n\t\tconst count = view.getUint16(end + 10, true), size = view.getUint32(end + 12, true), offset = view.getUint32(end + 16, true);\r\n\t\tif (view.getUint16(end + 4, true) || view.getUint16(end + 6, true) || view.getUint16(end + 8, true) !== count || count === 65535 || size === 0xffffffff || offset === 0xffffffff)\r\n\t\t\tthrow new Constants.Error.Error(file.name, \"Split-disk and ZIP64 archives are not supported by the bounded browser importer. Use a standard single-disk CBZ.\");\r\n\t\tif (count > 2048 || size > 8 * 1024 * 1024 || offset + size !== start + end)\r\n\t\t\tthrow new Constants.Error.Error(file.name, \"Archive directory exceeds the bounded entry/metadata admission limit.\");\r\n\t\tconst directory = new Uint8Array(await file.slice(offset, offset + size).arrayBuffer()), central = new DataView(directory.buffer);\r\n\t\tlet cursor = 0;\r\n\t\tfor (let index = 0; index < count; index++) {\r\n\t\t\tif (cursor + 46 > directory.length || central.getUint32(cursor, true) !== 0x02014b50) throw new Constants.Error.Error(file.name, \"Failed to extract archive: invalid central directory.\");\r\n\t\t\tconst compressed = central.getUint32(cursor + 20, true), expanded = central.getUint32(cursor + 24, true), local = central.getUint32(cursor + 42, true);\r\n\t\t\tif (central.getUint16(cursor + 34, true) || [compressed, expanded, local].includes(0xffffffff)) throw new Constants.Error.Error(file.name, \"ZIP64 and split-disk entries are not supported.\");\r\n\t\t\tif (local >= offset || compressed > file.size || expanded > maxSessionSize) throw new Constants.Error.Error(file.name, \"Archive entry exceeds the bounded size admission limit.\");\r\n\t\t\tcursor += 46 + central.getUint16(cursor + 28, true) + central.getUint16(cursor + 30, true) + central.getUint16(cursor + 32, true);\r\n\t\t\tif (cursor > directory.length) throw new Constants.Error.Error(file.name, \"Failed to extract archive: truncated central directory.\");\r\n\t\t}\r\n\t\tif (cursor !== directory.length) throw new Constants.Error.Error(file.name, \"Unsupported trailing ZIP directory metadata.\");\r\n\t};\r\n\tconst extractArchive = async (file: File, byteBudget = maxSessionSize, countBudget = maxSessionFiles) => {\r\n\t\tif (file.size > maxArchiveCompressedSize) throw new Constants.Error.Error(file.name, \"Archive exceeds the 100 MiB compressed admission limit. Split it into smaller archives.\");\r\n\t\tawait inspectArchiveDirectory(file);\r\n\t\ttry {\r\n\t\t\tconst zip = await JSZip.loadAsync(file);\r\n\t\t\tconst all = Object.values(zip.files);\r\n\t\t\tif (all.length > 2048) throw new Constants.Error.Error(file.name, \"Archive has too many entries.\");\r\n\t\t\tconst entries = all.filter(entry => !entry.dir && /\\.(jpg|jpeg|png|gif)$/i.test(entry.name)).sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: \"base\" }));\r\n\t\t\tif (entries.length > countBudget) throw new Constants.Error.Error(file.name, \"Archive exceeds the remaining 500-page session limit.\");\r\n\t\t\tconst files: File[] = [];\r\n\t\t\tlet remaining = byteBudget;\r\n\t\t\tfor (const entry of entries) {\r\n\t\t\t\t// Stream decompressed output and stop before accumulating a ZIP bomb.\r\n\t\t\t\tconst blob = await new Promise<Blob>((resolve, reject) => {\r\n\t\t\t\t\tconst stream = (entry as any).internalStream(\"uint8array\");\r\n\t\t\t\t\tlet chunks: Uint8Array[] = [], size = 0, finished = false;\r\n\t\t\t\t\tconst fail = (error: unknown) => { if (finished) return; finished = true; stream.pause(); chunks = []; reject(error); };\r\n\t\t\t\t\tstream.on(\"data\", (chunk: Uint8Array) => {\r\n\t\t\t\t\t\tif (finished) return;\r\n\t\t\t\t\t\tif (size + chunk.byteLength > maxFileSize || chunk.byteLength > remaining) return fail(new Constants.Error.Error(entry.name, \"Decompressed image or aggregate archive output exceeds the upload limit.\"));\r\n\t\t\t\t\t\tsize += chunk.byteLength; remaining -= chunk.byteLength; chunks.push(new Uint8Array(chunk));\r\n\t\t\t\t\t}).on(\"error\", fail).on(\"end\", () => { if (!finished) { finished = true; resolve(new Blob(chunks as BlobPart[])); chunks = []; } }).resume();\r\n\t\t\t\t});\r\n\t\t\t\tconst ext = entry.name.toLowerCase().split(\".\").pop();\r\n\t\t\t\tconst type = ext === \"png\" ? \"image/png\" : ext === \"gif\" ? \"image/gif\" : \"image/jpeg\";\r\n\t\t\t\tfiles.push(new File([blob], entry.name, { type }));\r\n\t\t\t}\r\n\t\t\treturn files;\r\n\t\t} catch (error) {\r\n\t\t\tif (error instanceof Constants.Error.Error) throw error;\r\n\t\t\tthrow new Constants.Error.Error(file.name, \"Failed to extract archive\");\r\n\t\t}\r\n\t};\r\n"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "untrusted image processing is sequential dimensions-admitted and session-bounded",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\tconst processFiles = async (files: File[]) => {\r\n\t\tconst processedFiles = (\r\n\t\t\tawait Promise.all(\r\n\t\t\t\tfiles.map(async file => {\r\n\t\t\t\t\tif (imageMimeTypes.includes(file.type)) {\r\n\t\t\t\t\t\tif (file.size > maxFileSize) {\r\n\t\t\t\t\t\t\ttoast.error(file.name, { description: \"File too large\" });\r\n\t\t\t\t\t\t\treturn undefined;\r\n\t\t\t\t\t\t}\r\n\r\n\t\t\t\t\t\tconst image = await new Promise<HTMLImageElement>(resolve => {\r\n\t\t\t\t\t\t\tconst image = new Image();\r\n\t\t\t\t\t\t\timage.onload = () => resolve(image);\r\n\t\t\t\t\t\t\timage.src = URL.createObjectURL(file);\r\n\t\t\t\t\t\t});\r\n\t\t\t\t\t\treturn image.width > maxImageWidth || image.height > maxImageHeight ? splitImage(image, file.name) : file;\r\n\t\t\t\t\t} else if (archiveMimeTypes.includes(file.type) || file.name.endsWith(\".cbz\")) {\r\n\t\t\t\t\t\treturn extractArchive(file);\r\n\t\t\t\t\t}\r\n\t\t\t\t})\r\n\t\t\t)\r\n\t\t)\r\n\t\t\t.flat()\r\n\t\t\t.filter(e => e instanceof File);\r\n",
    "after": "\tconst inspectImageDimensions = async (file: File) => {\r\n\t\tconst bytes = new Uint8Array(await file.slice(0, 1024 * 1024).arrayBuffer());\r\n\t\tconst view = new DataView(bytes.buffer);\r\n\t\tlet width = 0, height = 0;\r\n\t\tif (bytes.length >= 24 && bytes[0] === 137 && bytes[1] === 80 && bytes[2] === 78 && bytes[3] === 71) { width = view.getUint32(16); height = view.getUint32(20); }\r\n\t\telse if (bytes.length >= 10 && String.fromCharCode(...bytes.subarray(0, 3)) === \"GIF\") { width = view.getUint16(6, true); height = view.getUint16(8, true); }\r\n\t\telse if (bytes.length >= 4 && bytes[0] === 255 && bytes[1] === 216) {\r\n\t\t\tlet offset = 2;\r\n\t\t\twhile (offset + 4 <= bytes.length) {\r\n\t\t\t\tif (bytes[offset++] !== 255) break;\r\n\t\t\t\twhile (bytes[offset] === 255) offset++;\r\n\t\t\t\tconst marker = bytes[offset++];\r\n\t\t\t\tif (marker === 217 || marker === 218) break;\r\n\t\t\t\tif (marker === 1 || marker >= 208 && marker <= 215) continue;\r\n\t\t\t\tif (offset + 2 > bytes.length) break;\r\n\t\t\t\tconst length = view.getUint16(offset);\r\n\t\t\t\tif (length < 2 || offset + length > bytes.length) break;\r\n\t\t\t\tif (marker >= 192 && marker <= 207 && ![196, 200, 204].includes(marker) && length >= 7) { height = view.getUint16(offset + 3); width = view.getUint16(offset + 5); break; }\r\n\t\t\t\toffset += length;\r\n\t\t\t}\r\n\t\t}\r\n\t\tif (!width || !height || width * height > 50_000_000) throw new Constants.Error.Error(file.name, \"Could not decode image, or its decoded dimensions exceed the 50-million-pixel admission limit.\");\r\n\t};\r\n\tconst decodeUploadImage = async (file: File) => {\r\n\t\tawait inspectImageDimensions(file);\r\n\t\treturn new Promise<HTMLImageElement>((resolve, reject) => {\r\n\t\t\tconst image = new Image(), url = URL.createObjectURL(file);\r\n\t\t\tconst cleanup = () => { image.onload = image.onerror = null; window.clearTimeout(timer); URL.revokeObjectURL(url); };\r\n\t\t\tconst timer = window.setTimeout(() => { cleanup(); image.src = \"\"; reject(new Constants.Error.Error(file.name, \"Could not decode image before the 30-second deadline.\")); }, 30000);\r\n\t\t\timage.onload = () => { cleanup(); resolve(image); };\r\n\t\t\timage.onerror = () => { cleanup(); reject(new Constants.Error.Error(file.name, \"Could not decode image.\")); };\r\n\t\t\timage.src = url;\r\n\t\t});\r\n\t};\r\n\tconst processFiles = async (files: File[]) => {\r\n\t\tif (!uploadSession || uploadSession.can_mutate === false || disabled) return;\r\n\t\tif (files.length > maxSessionFiles || pages.length + files.length > maxSessionFiles) throw new Constants.Error.Error(\"Too many upload files\", \"A session admits at most 500 pages. Split large folders into smaller selections.\");\r\n\t\tconst admittedSessionSize = pages.reduce((total, page) => total + Number(page.file_size ?? page.file?.size ?? 0), 0);\r\n\t\tlet remaining = maxSessionSize - admittedSessionSize;\r\n\t\tconst processedFiles: File[] = [];\r\n\t\t// One decoder at a time bounds memory; no Promise.all over untrusted files.\r\n\t\tconst admit = async (file: File) => {\r\n\t\t\tif (file.size > maxFileSize) throw new Constants.Error.Error(file.name, \"File too large\");\r\n\t\t\tconst decoded = await decodeUploadImage(file);\r\n\t\t\tconst pieces = decoded.width > maxImageWidth || decoded.height > maxImageHeight ? await splitImage(decoded, file.name) : [file];\r\n\t\t\tfor (const piece of pieces) {\r\n\t\t\t\tif (pages.length + processedFiles.length >= maxSessionFiles || piece.size > maxFileSize || piece.size > remaining) throw new Constants.Error.Error(\"Upload limit exceeded\", \"The decoded selection exceeds 500 pages or the 500 MiB session budget.\");\r\n\t\t\t\tremaining -= piece.size; processedFiles.push(piece);\r\n\t\t\t}\r\n\t\t};\r\n\t\tfor (const file of files) {\r\n\t\t\tif (imageMimeTypes.includes(file.type)) await admit(file);\r\n\t\t\telse if (archiveMimeTypes.includes(file.type) || /\\.(cbz|zip)$/i.test(file.name)) {\r\n\t\t\t\tconst extracted = await extractArchive(file, remaining, maxSessionFiles - pages.length - processedFiles.length);\r\n\t\t\t\tfor (const image of extracted) await admit(image);\r\n\t\t\t}\r\n\t\t}\r\n"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "folder scans share one bounded admission budget across every browser batch",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\tconst scanFiles = async (entry?: FileSystemEntry | null): Promise<File[]> => {\r\n\t\tif (entry?.isFile) {\r\n\t\t\treturn new Promise<File[]>(resolve => (<FileSystemFileEntry>entry).file(file => resolve([file])));\r\n\t\t} else if (entry?.isDirectory) {\r\n\t\t\tconst reader = (entry as FileSystemDirectoryEntry).createReader();\r\n\t\t\tconst entries = await new Promise<FileSystemEntry[]>((resolve, reject) => reader.readEntries(resolve, reject));\r\n\t\t\treturn (await Promise.all(entries.map(scanFiles))).flat();\r\n\t\t}\r\n\t\treturn [];\r\n\t};\r\n",
    "after": "\tconst scanFiles = async (entry?: FileSystemEntry | null, budget = { files: 0, entries: 0, bytes: 0 }, depth = 0): Promise<File[]> => {\r\n\t\tif (!entry) return [];\r\n\t\tif (++budget.entries > 2048 || depth > 16) throw new Constants.Error.Error(\"Folder exceeds upload limits\", \"Limit the selection to 500 files, 2048 entries and 16 nested directories.\");\r\n\t\tif (entry.isFile) {\r\n\t\t\tif (++budget.files > maxSessionFiles) throw new Constants.Error.Error(\"Too many upload files\", \"A selection admits at most 500 files.\");\r\n\t\t\tconst file = await new Promise<File>((resolve, reject) => (<FileSystemFileEntry>entry).file(resolve, reject));\r\n\t\t\tbudget.bytes += file.size;\r\n\t\t\tif (budget.bytes > maxSessionSize) throw new Constants.Error.Error(\"Folder exceeds upload limits\", \"Selected input exceeds 500 MiB.\");\r\n\t\t\treturn [file];\r\n\t\t}\r\n\t\tif (!entry.isDirectory) return [];\r\n\t\tconst reader = (entry as FileSystemDirectoryEntry).createReader(), files: File[] = [];\r\n\t\twhile (true) {\r\n\t\t\tconst entries = await new Promise<FileSystemEntry[]>((resolve, reject) => reader.readEntries(resolve, reject));\r\n\t\t\tif (!entries.length) break;\r\n\t\t\tfor (const child of entries) files.push(...await scanFiles(child, budget, depth + 1));\r\n\t\t}\r\n\t\treturn files;\r\n\t};\r\n"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "directory drop roots are scanned sequentially with shared budget",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "const files = await Promise.all(Array.from(ev.dataTransfer.items).map(e => scanFiles(e.webkitGetAsEntry())));",
    "after": "const budget = { files: 0, entries: 0, bytes: 0 };\r\n\t\t\t\t\tconst files: File[][] = [];\r\n\t\t\t\t\tif (ev.dataTransfer.items.length > maxSessionFiles) throw new Constants.Error.Error(\"Too many upload files\", \"A selection admits at most 500 roots.\");\r\n\t\t\t\t\tfor (const item of Array.from(ev.dataTransfer.items)) files.push(await scanFiles(item.webkitGetAsEntry(), budget));"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "split PNG filenames retain actual extension for backend MIME validation",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 2,
    "before": "new File([blob], name + \"_\" + i, { type: \"image/png\" })",
    "after": "new File([blob], name.replace(/\\.[^.]+$/, \"\") + \"_\" + i + \".png\", { type: \"image/png\" })"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "untrusted aggregate bytes use finite staged file size fallback",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "Number(file.file_size)",
    "after": "Number(file.file_size ?? file.file?.size ?? 0)"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "upload batch transport failures remain visible and are never treated as successful files",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\t\t\tpages = pages;\r\n\t\t}\r\n\t};",
    "after": "\t\t\tpages = pages;\r\n\t\t} else throw new Constants.Error.Error(\"Upload was not confirmed\", \"The server did not accept this batch. Staged files were retained; reload its current receipt before retrying.\");\r\n\t};"
  },
  {
    "path": "src/lib/components/draft/manga/MangaEdit.svelte",
    "label": "draft publish consumes actual200 receipt and truthful pending or local scope",
    "sha256": "e1f8b3a97bc414eab974258ee1e529fa0a7df5b0841573f778868770e44b3782",
    "count": 1,
    "before": "\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\ttoast.success(\"Draft published successfully\");\r\n\t\t\t\t\tmanga.state = Constants.State.Published;",
    "after": "\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\tconst receipt = res.body as Manga & { publication_scope?: string; publication_status?: string; protocol_event_id?: string; is_unavailable?: boolean };\r\n\t\t\t\t\tif (receipt.relationships) manga = receipt;\r\n\t\t\t\t\telse Object.assign(manga, { state: receipt.state, version: receipt.version, protocol_event_id: receipt.protocol_event_id, is_unavailable: receipt.is_unavailable });\r\n\t\t\t\t\tif (receipt.publication_status === \"accepted_pending_projection\") {\r\n\t\t\t\t\t\ttoast.info(\"Signed title accepted\", { description: \"The original protocol accepted this title. It is waiting for local catalogue processing; public detail is not available yet.\", duration: 15000 });\r\n\t\t\t\t\t} else if (receipt.publication_scope === \"instance-local\") {\r\n\t\t\t\t\t\ttoast.success(\"Title published on this instance\", { description: \"Original-protocol publication is not configured here. Other peers were not notified.\" });\r\n\t\t\t\t\t} else toast.success(\"Signed title published\", { description: \"The actual original-protocol receipt was accepted and locally hosted.\" });"
  },
  {
    "path": "src/lib/components/draft/manga/MangaEdit.svelte",
    "label": "draft rejection consumes authoritative200 body and updated version",
    "sha256": "e1f8b3a97bc414eab974258ee1e529fa0a7df5b0841573f778868770e44b3782",
    "count": 1,
    "before": "\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\ttoast.success(\"Draft rejected successfully\");\r\n\t\t\t\t\tmanga.state = Constants.State.Rejected;",
    "after": "\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\tmanga = res.body;\r\n\t\t\t\t\ttoast.success(\"Draft rejected successfully\");"
  },
  {
    "path": "src/lib/components/draft/manga/MangaEdit.svelte",
    "label": "lock mutation consumes authoritative body rather than stale optimistic flag",
    "sha256": "e1f8b3a97bc414eab974258ee1e529fa0a7df5b0841573f778868770e44b3782",
    "count": 1,
    "before": "\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\ttoast.success(\"Lock state updated successfully\");\r\n\t\t\t\t\tmanga.locked = !manga.locked;",
    "after": "\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\tmanga = res.body;\r\n\t\t\t\t\ttoast.success(\"Lock state updated successfully\");"
  },
  {
    "path": "src/routes/(app)/title/[id]/edit/+page.svelte",
    "label": "pending title receipt uses scoped external editor boundary",
    "sha256": "c49a2631b678999fd4354cb8e17f5e3843b33d57532d4dba141401bd4aae5c78",
    "count": 1,
    "before": "import MangaEdit from \"$lib/components/draft/manga/MangaEdit.svelte\";",
    "after": "import MangaEdit from \"$platform/MangaEditBoundary.svelte\";"
  },
  {
    "path": "src/routes/(app)/title/[id]/edit/+page.svelte",
    "label": "minimal pending title receipt does not fetch public covers",
    "sha256": "c49a2631b678999fd4354cb8e17f5e3843b33d57532d4dba141401bd4aae5c78",
    "count": 1,
    "before": "if (!data.manga?.id) return;",
    "after": "if (!data.manga?.id || !data.manga.relationships) return;"
  },
  {
    "path": "src/routes/(app)/title/[id]/edit/+page.ts",
    "label": "owner-authorized minimal pending receipt remains read-only instead of full editor permission claim",
    "sha256": "b7afa5cd1115e3e43279c98da877ee6c7ae3a70b1019a6b7d2e0595cacd0ff0e",
    "count": 1,
    "before": "if (!Permission.Manga.canUpdate(res.body)) {",
    "after": "if (!((res.body as Manga & { publication_status?: string }).publication_status === \"accepted_pending_projection\" && !res.body.relationships) && !Permission.Manga.canUpdate(res.body)) {"
  },
  {
    "path": "src/lib/components/draft/manga/MangaEdit.svelte",
    "label": "publication outcome triggers authoritative parent boundary reconciliation",
    "sha256": "e1f8b3a97bc414eab974258ee1e529fa0a7df5b0841573f778868770e44b3782",
    "count": 1,
    "before": "\t\t\t\tisPublishing = false;",
    "after": "\t\t\t\tisPublishing = false;\r\n\t\t\t\twindow.dispatchEvent(new CustomEvent(\"weebdex-title-publication-changed\", { detail: { id: manga.id } }));"
  },
  {
    "path": "src/lib/components/reader/reader-provider.svelte",
    "label": "aggregate is component-owned rather than stale manga-only shared state",
    "sha256": "c12a1d141b258f7461a150511387488193b55f6825f9f4c3cd86fbc962feb3fb",
    "count": 1,
    "before": "\tlet lastMangaId = $state<string>();\r\n\tlet aggregated = $state<ChapterAggregateResponse>();\r\n",
    "after": ""
  },
  {
    "path": "src/lib/components/reader/reader-provider.svelte",
    "label": "aggregate request context and lifecycle are explicit per reader",
    "sha256": "c12a1d141b258f7461a150511387488193b55f6825f9f4c3cd86fbc962feb3fb",
    "count": 1,
    "before": "\tconst { chapter, children }: { chapter: Chapter; children: Snippet } = $props();",
    "after": "\tconst { chapter, children }: { chapter: Chapter; children: Snippet } = $props();\r\n\tlet aggregated = $state<ChapterAggregateResponse>();\r\n\tlet aggregateGeneration = 0, aggregateDisposed = false;\r\n\tonDestroy(() => { aggregateDisposed = true; aggregateGeneration++; aggregated = undefined; });"
  },
  {
    "path": "src/lib/components/reader/reader-provider.svelte",
    "label": "aggregate exact language context rejects late results and last page uses one-based state",
    "sha256": "c12a1d141b258f7461a150511387488193b55f6825f9f4c3cd86fbc962feb3fb",
    "count": 1,
    "before": "\tonMount(async () => {\r\n\t\tif (reader.isViewingLastPage) {\r\n\t\t\tawait tick().then(async () => {\r\n\t\t\t\tconst lastPage = Math.max(reader.pages.length - 1, 0);\r\n\t\t\t\treader.currentPage = reader.pages[lastPage];\r\n\r\n\t\t\t\treplaceState(`/chapter/${chapter.id}/${lastPage}`, { ...page.state, page: lastPage });\r\n\t\t\t});\r\n\t\t} else if (!page.params.page) {\r\n\t\t\tawait tick().then(() => replaceState(`/chapter/${chapter.id}/1`, { ...page.state, page: 1 }));\r\n\t\t}\r\n\r\n\t\tif (lastMangaId !== chapter.relationships.manga.id || !aggregated) {\r\n\t\t\ttry {\r\n\t\t\t\tconst q = new URLSearchParams();\r\n\t\t\t\tq.set(\"tlang\", chapter.language);\r\n\r\n\t\t\t\tconst res = await API.Chapter.Aggregate(chapter.relationships.manga.id, q);\r\n\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\tlastMangaId = chapter.relationships.manga.id;\r\n\t\t\t\t\taggregated = res.body;\r\n\t\t\t\t}\r\n\t\t\t} catch (e) {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t}\r\n\t\t}\r\n\t});",
    "after": "\tonMount(() => {\r\n\t\tconst generation = ++aggregateGeneration;\r\n\t\tconst alive = () => !aggregateDisposed && generation === aggregateGeneration;\r\n\t\tvoid (async () => {\r\n\t\t\tawait tick(); if (!alive()) return;\r\n\t\t\tif (reader.isViewingLastPage) {\r\n\t\t\t\tconst lastIndex = Math.max(reader.pages.length - 1, 0);\r\n\t\t\t\treader.currentPage = reader.pages[lastIndex];\r\n\t\t\t\treplaceState(`/chapter/${chapter.id}/${lastIndex + 1}`, { ...page.state, page: lastIndex + 1 });\r\n\t\t\t} else if (!page.params.page) replaceState(`/chapter/${chapter.id}/1`, { ...page.state, page: 1 });\r\n\t\t\tconst mangaId = chapter.relationships.manga.id, chapterId = chapter.id;\r\n\t\t\tconst q = new URLSearchParams(); q.set(\"tlang\", chapter.language);\r\n\t\t\tconst contextKey = JSON.stringify([mangaId, [...q.entries()]]);\r\n\t\t\taggregated = undefined;\r\n\t\t\tconst res = await API.Chapter.Aggregate(mangaId, q);\r\n\t\t\tif (!alive() || chapter.id !== chapterId || JSON.stringify([chapter.relationships.manga.id, [[\"tlang\", chapter.language]]]) !== contextKey) return;\r\n\t\t\t// Every mounted reader rechecks current authority; failures/hidden results are never cached.\r\n\t\t\taggregated = res?.status === StatusCodes.OK && res.body ? res.body : undefined;\r\n\t\t})().catch(error => { if (alive()) { aggregated = undefined; console.error(error); } });\r\n\t});"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "replacement validates all image budgets and swaps durable pages atomically without prior delete",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\tconst replaceFile = async (file: File) => {\r\n\t\tif (!targetFile || !uploadSession) return;\r\n\r\n\t\tconst idx = pages.indexOf(targetFile);\r\n\t\tif (idx < 0) return;\r\n\r\n\t\tconst newFile = createPage(file);\r\n\t\tif (!targetFile.file && !targetFile.error) {\r\n\t\t\tconst res = await API.UploadSession.DeleteFile(uploadSession.id, targetFile.id);\r\n\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t}\r\n\r\n\t\trevokeThumbnail(targetFile);\r\n\t\tpages[idx] = newFile;\r\n\t\tprogress.total += newFile.file!.size;\r\n\r\n\t\tawait uploadChunk([newFile], delta => {\r\n\t\t\tprogress.loaded += delta;\r\n\t\t});\r\n\r\n\t\tif (progress.loaded >= progress.total) {\r\n\t\t\tsetTimeout(() => {\r\n\t\t\t\tprogress = { total: 0, loaded: 0 };\r\n\t\t\t}, 1000);\r\n\t\t}\r\n\t};",
    "after": "\tconst replaceFile = async (file: File) => {\r\n\t\tif (!targetFile || !uploadSession || uploadSession.can_mutate === false || disabled) return;\r\n\t\tconst target = targetFile, idx = pages.indexOf(target); if (idx < 0) return;\r\n\t\tif (!imageMimeTypes.includes(file.type) || !file.size || file.size > maxFileSize) throw new Constants.Error.Error(file.name, \"Replacement must be a valid image within the 25 MiB page limit. Existing page is retained.\");\r\n\t\tconst header = new Uint8Array(await file.slice(0, 12).arrayBuffer()), extension = file.name.toLowerCase().match(/\\.(png|gif|jpe?g)$/)?.[1];\r\n\t\tconst actual = header.length >= 8 && [137,80,78,71,13,10,26,10].every((byte,index) => header[index] === byte) ? \"png\" : header.length >= 6 && [\"GIF87a\",\"GIF89a\"].includes(String.fromCharCode(...header.subarray(0,6))) ? \"gif\" : header.length >= 3 && header[0] === 255 && header[1] === 216 && header[2] === 255 ? \"jpeg\" : undefined;\r\n\t\tif (!actual || actual !== (extension === \"jpg\" ? \"jpeg\" : extension) || (file.type.replace(\"image/jpg\", \"image/jpeg\") !== `image/${actual}`)) throw new Constants.Error.Error(file.name, \"Image filename, MIME and encoded format must agree. Existing page is retained.\");\r\n\t\tconst decoded = await decodeUploadImage(file);\r\n\t\tif (decoded.width > maxImageWidth || decoded.height > maxImageHeight) throw new Constants.Error.Error(file.name, \"A replacement must fit one bounded page. Add this image normally to split it; existing page is retained.\");\r\n\t\tconst total = pages.reduce((sum, page) => sum + Number(page.file_size ?? page.file?.size ?? 0), 0), previousSize = Number(target.file_size ?? target.file?.size ?? 0);\r\n\t\tif (!Number.isFinite(total) || !Number.isFinite(previousSize) || pages.length > maxSessionFiles || total - previousSize + file.size > maxSessionSize) throw new Constants.Error.Error(file.name, \"Replacement exceeds the 500-page/500 MiB session budget. Existing page is retained.\");\r\n\t\t// A local rejected entry has no durable old file to replace.\r\n\t\tif (target.file || target.error) {\r\n\t\t\tconst candidate = createPage(file); pages[idx] = candidate;\r\n\t\t\ttry { await uploadChunk([candidate]); revokeThumbnail(target); }\r\n\t\t\tcatch (error) { pages[idx] = target; revokeThumbnail(candidate); throw error; }\r\n\t\t\treturn;\r\n\t\t}\r\n\t\tconst current = await API.UploadSession.Get();\r\n\t\tif (current?.status !== StatusCodes.OK || current.body?.id !== uploadSession.id || current.body.can_mutate === false) throw new Constants.Error.Error(\"Replacement unavailable\", \"Current owner staging authority was not confirmed. Existing page is retained.\");\r\n\t\tconst old = current.body.relationships.files?.find(row => row.id === target.id);\r\n\t\tif (!old || target.file_hash && old.file_hash !== target.file_hash) throw new Constants.Error.Error(\"Page changed\", \"This staged page changed in another tab. Reload before replacing it.\");\r\n\t\tconst sha = [...new Uint8Array(await crypto.subtle.digest(\"SHA-256\", await file.arrayBuffer()))].map(byte => byte.toString(16).padStart(2,\"0\")).join(\"\");\r\n\t\tconst form = new FormData(); form.append(\"file\", file);\r\n\t\tconst http = new HTTP<UploadSessionFile & { version: number }>(`${PUBLIC_API_URL}/upload/${uploadSession.id}/files/${target.id}/replace`, form);\r\n\t\thttp.setWithCredentials(true); http.setRequestHeader(\"X-WeebDex-Upload-Version\", String(current.body.version));\r\n\t\tlet replacement: UploadSessionFile | undefined, version = current.body.version;\r\n\t\ttry {\r\n\t\t\tconst result = await http.POST();\r\n\t\t\tif (result?.status !== StatusCodes.OK || result.body?.id !== target.id || result.body.error) throw new Constants.Error.Error(\"Page not replaced\", `The server rejected the atomic replacement (${result?.status ?? \"unknown\"}). Existing page is retained; refresh staging before trying again.`);\r\n\t\t\treplacement = result.body; version = result.body.version;\r\n\t\t} catch (error) {\r\n\t\t\t// Read-only reconciliation, never a replay of the ambiguous mutation.\r\n\t\t\tconst latest = await API.UploadSession.Get().catch(() => undefined), confirmed = latest?.body?.relationships.files?.find(row => row.id === target.id);\r\n\t\t\tif (latest?.status === StatusCodes.OK && latest.body?.id === uploadSession.id && confirmed?.file_hash === sha && confirmed.original_file_name === file.name) { replacement = confirmed; version = latest.body.version; toast.success(\"Replacement confirmed from current staging\"); }\r\n\t\t\telse throw error;\r\n\t\t}\r\n\t\tif (!replacement || !Number.isInteger(version) || version < 1) throw new Constants.Error.Error(\"Replacement not confirmed\", \"Refresh owner staging to reconcile the current page. No request was replayed.\");\r\n\t\treplacement.thumbnail = URL.createObjectURL(file); revokeThumbnail(target); pages[idx] = replacement; pages = pages; uploadSession.version = version;\r\n\t};"
  },
  {
    "path": "src/lib/components/ReportSearch.svelte",
    "label": "private feed ignores all historical DTOs and reloads on actual account authority",
    "sha256": "f94201eb8724cd15ee05401376549896f4698a5853206faf6c375b6177f25edd",
    "count": 1,
    "before": "\tonMount(async () => {\r\n\t\tconst state = page.state as {\r\n\t\t\t_$ReportSearch_feed: typeof feed;\r\n\t\t};\r\n\t\tif (state._$ReportSearch_feed) {\r\n\t\t\tfeed = state._$ReportSearch_feed;\r\n\t\t} else {\r\n\t\t\ttry {\r\n\t\t\t\tawait onSearch();\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) error = e;\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\terror = Constants.Error.Unknown;\r\n\t\t\t\t}\r\n\t\t\t}\r\n\t\t}\r\n\t\tisMounted = true;\r\n\t});",
    "after": "\tlet privateMounted = $state(false), privateDisposed = false, privateGeneration = 0;\r\n\tonMount(() => { privateMounted = true; return () => { privateDisposed = true; privateGeneration++; feed = undefined; }; });\r\n\t$effect(() => {\r\n\t\tconst identity = $privateFeedIdentity;\r\n\t\tif (!privateMounted || privateDisposed) return;\r\n\t\tconst generation = ++privateGeneration;\r\n\t\tfeed = undefined; error = undefined; isMounted = false;\r\n\t\tvoid onSearch().then(() => { if (!privateDisposed && generation === privateGeneration && identity === $privateFeedIdentity) isMounted = true; }).catch(cause => {\r\n\t\t\tif (privateDisposed || generation !== privateGeneration || identity !== $privateFeedIdentity) return;\r\n\t\t\tfeed = undefined; error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown;\r\n\t\t});\r\n\t});"
  },
  {
    "path": "src/lib/components/ReportSearch.svelte",
    "label": "private feed authority subscription",
    "sha256": "f94201eb8724cd15ee05401376549896f4698a5853206faf6c375b6177f25edd",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport { privateFeedIdentity } from \"$platform/private-feed\";"
  },
  {
    "path": "src/routes/(app)/my/reports/+page.svelte",
    "label": "private feed read seam uses fresh matched browser authority",
    "sha256": "8a5ddb5387ecf78a376d2cffe6d5b0214ea8359895e7c438730156b811363f5a",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport { readPrivateFeed, privateFeedIdentity } from \"$platform/private-feed\";\r\n\tlet privateSearchGeneration = 0;\r\n\t$effect(() => { $privateFeedIdentity; privateSearchGeneration++; feed = undefined; });"
  },
  {
    "path": "src/routes/(app)/my/reports/+page.svelte",
    "label": "capture private request generation before any await",
    "sha256": "8a5ddb5387ecf78a376d2cffe6d5b0214ea8359895e7c438730156b811363f5a",
    "count": 1,
    "before": "\tconst onSearch = async () => {",
    "after": "\tconst onSearch = async () => {\r\n\t\tconst generation = ++privateSearchGeneration, identity = $privateFeedIdentity;\r\n\t\tfeed = undefined;"
  },
  {
    "path": "src/routes/(app)/my/reports/+page.svelte",
    "label": "private feed request is account pinned",
    "sha256": "8a5ddb5387ecf78a376d2cffe6d5b0214ea8359895e7c438730156b811363f5a",
    "count": 1,
    "before": "const res = await API.Report.Search(q);",
    "after": "const res = await readPrivateFeed<ReportFeedResponse>(\"/report\", q);"
  },
  {
    "path": "src/routes/(app)/my/reports/+page.svelte",
    "label": "late old private feed cannot attach to current page",
    "sha256": "8a5ddb5387ecf78a376d2cffe6d5b0214ea8359895e7c438730156b811363f5a",
    "count": 1,
    "before": "if (window.location.href !== href) {",
    "after": "if (generation !== privateSearchGeneration || identity !== $privateFeedIdentity || window.location.href !== href) {"
  },
  {
    "path": "src/routes/(app)/my/reports/+page.svelte",
    "label": "navigation history retains no private feed DTO",
    "sha256": "8a5ddb5387ecf78a376d2cffe6d5b0214ea8359895e7c438730156b811363f5a",
    "count": 1,
    "before": "\t\t\treplaceState(window.location.href, {\r\n\t\t\t\t...page.state,\r\n\t\t\t\t_$ReportSearch_feed: $state.snapshot(feed)\r\n\t\t\t});",
    "after": "\t\t\tconst { _$ReportSearch_feed: discardedPrivateCache, ...safeState } = page.state as Record<string, unknown>;\r\n\t\t\treplaceState(window.location.href, safeState);"
  },
  {
    "path": "src/lib/components/draft/manga/DraftSearch.svelte",
    "label": "private feed ignores all historical DTOs and reloads on actual account authority",
    "sha256": "8fb57e167d33566febc60403cc5ad7667257c395c2cead532e74530333ee99bb",
    "count": 1,
    "before": "\tonMount(async () => {\r\n\t\tconst state = page.state as {\r\n\t\t\t_$DraftSearch_feed: typeof feed;\r\n\t\t};\r\n\t\tif (state._$DraftSearch_feed) {\r\n\t\t\tfeed = state._$DraftSearch_feed;\r\n\t\t} else {\r\n\t\t\ttry {\r\n\t\t\t\tawait onSearch();\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) error = e;\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\terror = Constants.Error.Unknown;\r\n\t\t\t\t}\r\n\t\t\t}\r\n\t\t}\r\n\t\tisMounted = true;\r\n\t});",
    "after": "\tlet privateMounted = $state(false), privateDisposed = false, privateGeneration = 0;\r\n\tonMount(() => { privateMounted = true; return () => { privateDisposed = true; privateGeneration++; feed = undefined; }; });\r\n\t$effect(() => {\r\n\t\tconst identity = $privateFeedIdentity;\r\n\t\tif (!privateMounted || privateDisposed) return;\r\n\t\tconst generation = ++privateGeneration;\r\n\t\tfeed = undefined; error = undefined; isMounted = false;\r\n\t\tvoid onSearch().then(() => { if (!privateDisposed && generation === privateGeneration && identity === $privateFeedIdentity) isMounted = true; }).catch(cause => {\r\n\t\t\tif (privateDisposed || generation !== privateGeneration || identity !== $privateFeedIdentity) return;\r\n\t\t\tfeed = undefined; error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown;\r\n\t\t});\r\n\t});"
  },
  {
    "path": "src/lib/components/draft/manga/DraftSearch.svelte",
    "label": "private feed authority subscription",
    "sha256": "8fb57e167d33566febc60403cc5ad7667257c395c2cead532e74530333ee99bb",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport { privateFeedIdentity } from \"$platform/private-feed\";"
  },
  {
    "path": "src/routes/(app)/drafts/title/+page.svelte",
    "label": "private feed read seam uses fresh matched browser authority",
    "sha256": "495a4e72a3c9098d6b67f9dbb0cdbb01bf3692bb2849430f2547cad02fda22fd",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport { readPrivateFeed, privateFeedIdentity } from \"$platform/private-feed\";\r\n\tlet privateSearchGeneration = 0;\r\n\t$effect(() => { $privateFeedIdentity; privateSearchGeneration++; feed = undefined; });"
  },
  {
    "path": "src/routes/(app)/drafts/title/+page.svelte",
    "label": "capture private request generation before any await",
    "sha256": "495a4e72a3c9098d6b67f9dbb0cdbb01bf3692bb2849430f2547cad02fda22fd",
    "count": 1,
    "before": "\tconst onSearch = async () => {",
    "after": "\tconst onSearch = async () => {\r\n\t\tconst generation = ++privateSearchGeneration, identity = $privateFeedIdentity;\r\n\t\tfeed = undefined;"
  },
  {
    "path": "src/routes/(app)/drafts/title/+page.svelte",
    "label": "private feed request is account pinned",
    "sha256": "495a4e72a3c9098d6b67f9dbb0cdbb01bf3692bb2849430f2547cad02fda22fd",
    "count": 1,
    "before": "const res = await API.Manga.SearchDrafts(q);",
    "after": "const res = await readPrivateFeed<MangaFeedResponse>(\"/manga/draft\", q);"
  },
  {
    "path": "src/routes/(app)/drafts/title/+page.svelte",
    "label": "late old private feed cannot attach to current page",
    "sha256": "495a4e72a3c9098d6b67f9dbb0cdbb01bf3692bb2849430f2547cad02fda22fd",
    "count": 1,
    "before": "if (window.location.href !== href) {",
    "after": "if (generation !== privateSearchGeneration || identity !== $privateFeedIdentity || window.location.href !== href) {"
  },
  {
    "path": "src/routes/(app)/drafts/title/+page.svelte",
    "label": "navigation history retains no private feed DTO",
    "sha256": "495a4e72a3c9098d6b67f9dbb0cdbb01bf3692bb2849430f2547cad02fda22fd",
    "count": 1,
    "before": "\t\t\treplaceState(window.location.href, {\r\n\t\t\t\t...page.state,\r\n\t\t\t\t_$DraftSearch_feed: $state.snapshot(feed)\r\n\t\t\t});",
    "after": "\t\t\tconst { _$DraftSearch_feed: discardedPrivateCache, ...safeState } = page.state as Record<string, unknown>;\r\n\t\t\treplaceState(window.location.href, safeState);"
  },
  {
    "path": "src/lib/components/ReportDialog.svelte",
    "label": "report close timer and reason reads belong to exact open subject generation",
    "sha256": "12fc7e4392f038dc0cabd22a192208eae8c6e78db2adb06c5c1b730e8c2beca7",
    "count": 1,
    "before": "\t$effect(() => {\r\n\t\tif (!isOpen) {\r\n\t\t\twindow.setTimeout(() => {\r\n\t\t\t\treasons = undefined;\r\n\t\t\t\tselectedReasonId = undefined;\r\n\t\t\t\tadditionalExplanation = undefined;\r\n\t\t\t\tentity = undefined;\r\n\t\t\t\tentityType = undefined;\r\n\t\t\t}, 200);\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tif (!reasons?.length && entityType) {\r\n\t\t\t(async () => {\r\n\t\t\t\tconst { href } = window.location;\r\n\r\n\t\t\t\ttry {\r\n\t\t\t\t\tconst res = await API.Report.Reasons(entityType);\r\n\t\t\t\t\tif (window.location.href !== href) {\r\n\t\t\t\t\t\treturn;\r\n\t\t\t\t\t}\r\n\r\n\t\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\t\treasons = res.body;\r\n\t\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t\t} catch (e) {\r\n\t\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\t\telse {\r\n\t\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t\t}\r\n\t\t\t\t}\r\n\t\t\t})();\r\n\t\t}\r\n\t});",
    "after": "\tlet reportGeneration = 0, reportDisposed = false, closeTimer = 0, reportContext = \"\";\r\n\tonDestroy(() => { reportDisposed = true; reportGeneration++; window.clearTimeout(closeTimer); });\r\n\t$effect(() => {\r\n\t\tconst type = entityType, subject = entity?.id, open = isOpen, href = page.url.href;\r\n\t\tconst context = JSON.stringify([open, type, subject, href]);\r\n\t\tif (context === reportContext) return;\r\n\t\treportContext = context; const generation = ++reportGeneration;\r\n\t\twindow.clearTimeout(closeTimer); closeTimer = 0;\r\n\t\treasons = undefined; selectedReasonId = undefined; additionalExplanation = undefined;\r\n\t\tif (!open) {\r\n\t\t\tcloseTimer = window.setTimeout(() => { if (!reportDisposed && !isOpen && generation === reportGeneration) { entity = undefined; entityType = undefined; } closeTimer = 0; }, 200);\r\n\t\t\treturn;\r\n\t\t}\r\n\t\tif (!type || subject === undefined) return;\r\n\t\tconst alive = () => !reportDisposed && isOpen && generation === reportGeneration && entityType === type && entity?.id === subject && window.location.href === href;\r\n\t\tvoid API.Report.Reasons(type).then(res => {\r\n\t\t\tif (!alive()) return;\r\n\t\t\tif (res?.status === StatusCodes.OK && res.body) reasons = res.body;\r\n\t\t\telse throw res?.status === StatusCodes.FORBIDDEN ? Constants.Error.Forbidden : res?.status === StatusCodes.TOO_MANY_REQUESTS ? Constants.Error.TooManyRequests : Constants.Error.Unknown;\r\n\t\t}).catch(cause => { if (!alive()) return; const error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; toast.error(error.title, { description: error.body }); });\r\n\t});"
  },
  {
    "path": "src/lib/components/ReportDialog.svelte",
    "label": "report dialog lifecycle import",
    "sha256": "12fc7e4392f038dc0cabd22a192208eae8c6e78db2adb06c5c1b730e8c2beca7",
    "count": 1,
    "before": "\timport { toast } from \"svelte-sonner\";",
    "after": "\timport { toast } from \"svelte-sonner\";\r\n\timport { onDestroy } from \"svelte\";"
  },
  {
    "path": "src/lib/components/ReportDialog.svelte",
    "label": "report submission captures original subject generation",
    "sha256": "12fc7e4392f038dc0cabd22a192208eae8c6e78db2adb06c5c1b730e8c2beca7",
    "count": 1,
    "before": "\t\t\ttry {\r\n\t\t\t\tconst res = await API.Report.Create({",
    "after": "\t\t\tconst submittedGeneration = reportGeneration;\r\n\t\t\ttry {\r\n\t\t\t\tconst res = await API.Report.Create({"
  },
  {
    "path": "src/lib/components/ReportDialog.svelte",
    "label": "accepted old report cannot close newly opened subject",
    "sha256": "12fc7e4392f038dc0cabd22a192208eae8c6e78db2adb06c5c1b730e8c2beca7",
    "count": 1,
    "before": "\t\t\t\t\tisOpen = false;\r\n\t\t\t\t\ttoast.success(\"Report submitted successfully\");",
    "after": "\t\t\t\t\tif (submittedGeneration === reportGeneration) isOpen = false;\r\n\t\t\t\t\ttoast.success(\"Report submitted successfully\");"
  },
  {
    "path": "src/routes/(app)/title/[id]/edit/+page.svelte",
    "label": "title edit covers load only for current target generation",
    "sha256": "c49a2631b678999fd4354cb8e17f5e3843b33d57532d4dba141401bd4aae5c78",
    "count": 1,
    "before": "\tonMount(async () => {\r\n\t\tif (data.manga) {\r\n\t\t\ttry {\r\n\t\t\t\tawait Promise.all([getCovers()]);\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) error = e;\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\terror = Constants.Error.Unknown;\r\n\t\t\t\t}\r\n\t\t\t}\r\n\t\t}\r\n\t\tisMounted = true;\r\n\t});",
    "after": "\tconst editCoverReader = createEditorCoverReader(() => loadedData.manga?.id ?? \"\");\r\n\tlet coverGeneration = 0, editDisposed = false;\r\n\tonDestroy(() => { editDisposed = true; coverGeneration++; });\r\n\t$effect.pre(() => {\r\n\t\tconst selectedAuthority = $privateFeedIdentity;\r\n\t\tconst mangaId = loadedData.manga?.id, generation = ++coverGeneration;\r\n\t\tresolvedEditorManga = undefined; resolvedEditorAuthority = undefined;\r\n\t\tcovers = []; error = undefined; isMounted = false;\r\n\t\tif (!mangaId) { isMounted = true; return; }\r\n\t\tlet ticket: ReturnType<typeof editCoverReader.capture>;\r\n\t\ttry { ticket = editCoverReader.capture(); }\r\n\t\tcatch (cause) { error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; isMounted = true; return; }\r\n\t\tconst current = () => !editDisposed && generation === coverGeneration && loadedData.manga?.id === mangaId && selectedAuthority === $privateFeedIdentity;\r\n\t\tvoid (async () => {\r\n\t\t\ttry {\r\n\t\t\t\tconst manga = await editCoverReader.currentTitle(mangaId, ticket); if (!current()) return;\r\n\t\t\t\tconst full = !!manga.relationships && typeof manga.language === \"string\";\r\n\t\t\t\tif (full && !Permission.Manga.canUpdate(manga)) throw Constants.Error.Forbidden;\r\n\t\t\t\tconst images = full ? await editCoverReader.covers(manga, ticket) : []; if (!current()) return;\r\n\t\t\t\tticket.assert(); covers = images; resolvedEditorManga = manga; resolvedEditorAuthority = selectedAuthority; isMounted = true;\r\n\t\t\t} catch (cause) {\r\n\t\t\t\tif (!current()) return;\r\n\t\t\t\terror = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; isMounted = true;\r\n\t\t\t} finally { ticket.release(); }\r\n\t\t})();\r\n\t});"
  },
  {
    "path": "src/routes/(app)/title/[id]/edit/+page.svelte",
    "label": "title edit cover lifecycle import",
    "sha256": "c49a2631b678999fd4354cb8e17f5e3843b33d57532d4dba141401bd4aae5c78",
    "count": 1,
    "before": "\timport { onMount } from \"svelte\";",
    "after": "\timport { onDestroy, onMount } from \"svelte\";"
  },
  {
    "path": "src/routes/(app)/title/[id]/edit/+page.svelte",
    "label": "remove unused unfenced edit cover read",
    "sha256": "c49a2631b678999fd4354cb8e17f5e3843b33d57532d4dba141401bd4aae5c78",
    "count": 1,
    "before": "\tconst getCovers = async () => {\r\n\t\tif (!data.manga?.id || !data.manga.relationships) return;\r\n\r\n\t\tconst res = await API.Cover.List(data.manga.id);\r\n\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\tcovers = res.body;\r\n\t\t}\r\n\t};",
    "after": ""
  },
  {
    "path": "src/routes/(app)/title/[id]/edit/+page.svelte",
    "label": "retained editor is keyed by actual title target",
    "sha256": "c49a2631b678999fd4354cb8e17f5e3843b33d57532d4dba141401bd4aae5c78",
    "count": 1,
    "before": "\t\t\t<MangaEdit data={{ manga: data.manga, covers }} />",
    "after": "\t\t\t{#key data.manga.id}\r\n\t\t\t\t<MangaEdit data={{ manga: data.manga, covers }} />\r\n\t\t\t{/key}",
    "approved_title_context_exception": "title_target_key"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "start read query is scoped to full current preferences URL and library",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "before": "\tlet aggregated = $state<ChapterAggregateResponse>();",
    "after": "\tlet aggregated = $state<ChapterAggregateResponse>();\r\n\tlet readGeneration = 0, readDisposed = false;\r\n\tconst readContext = () => JSON.stringify([manga.id, page.url.search, $prefs, $libraryIdentity.mode, $libraryIdentity.accountId, $localLibraryState.scopeStatus]);\r\n\t$effect(() => { readContext(); readGeneration++; aggregated = undefined; targetAggregatedChapter = undefined; isChapterSelectionDialogOpen = false; });\r\n\tonDestroy(() => { readDisposed = true; readGeneration++; });"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "start read component disposal import",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "before": "\timport { onMount } from \"svelte\";",
    "after": "\timport { onDestroy, onMount } from \"svelte\";"
  },
  {
    "path": "src/routes/(app)/title/MangaActions.svelte",
    "label": "start read fetches current authority and rejects stale query completions",
    "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
    "count": 1,
    "before": "\t\t\t\tif (!aggregated) {\r\n\t\t\t\t\tconst { pathname } = window.location;\r\n\t\t\t\t\tconst q = new URLSearchParams();\r\n\t\t\t\t\tprefs.applyChapterPreferences(q);\r\n\r\n\t\t\t\t\tif (page.url.searchParams.has(\"tlang\")) {\r\n\t\t\t\t\t\tq.delete(\"tlang\");\r\n\t\t\t\t\t\tpage.url.searchParams.getAll(\"tlang\").forEach(lang => q.append(\"tlang\", lang));\r\n\t\t\t\t\t}\r\n\t\t\t\t\tif (page.url.searchParams.has(\"group\")) {\r\n\t\t\t\t\t\tq.delete(\"group\");\r\n\t\t\t\t\t\tpage.url.searchParams.getAll(\"group\").forEach(group => q.append(\"group\", group));\r\n\t\t\t\t\t}\r\n\r\n\t\t\t\t\tconst res = await API.Chapter.Aggregate(manga.id, q);\r\n\t\t\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\t\t\treturn;\r\n\t\t\t\t\t}\r\n\r\n\t\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\t\taggregated = res.body;\r\n\t\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t\t}",
    "after": "\t\t\t\t{\r\n\t\t\t\t\tconst generation = ++readGeneration, context = readContext();\r\n\t\t\t\t\taggregated = undefined; targetAggregatedChapter = undefined; isChapterSelectionDialogOpen = false;\r\n\t\t\t\t\tconst { pathname } = window.location;\r\n\t\t\t\t\tconst q = new URLSearchParams();\r\n\t\t\t\t\tprefs.applyChapterPreferences(q);\r\n\r\n\t\t\t\t\tif (page.url.searchParams.has(\"tlang\")) {\r\n\t\t\t\t\t\tq.delete(\"tlang\");\r\n\t\t\t\t\t\tpage.url.searchParams.getAll(\"tlang\").forEach(lang => q.append(\"tlang\", lang));\r\n\t\t\t\t\t}\r\n\t\t\t\t\tif (page.url.searchParams.has(\"group\")) {\r\n\t\t\t\t\t\tq.delete(\"group\");\r\n\t\t\t\t\t\tpage.url.searchParams.getAll(\"group\").forEach(group => q.append(\"group\", group));\r\n\t\t\t\t\t}\r\n\r\n\t\t\t\t\tconst res = await API.Chapter.Aggregate(manga.id, q);\r\n\t\t\t\t\tif (readDisposed || generation !== readGeneration || context !== readContext() || window.location.pathname !== pathname) {\r\n\t\t\t\t\t\treturn;\r\n\t\t\t\t\t}\r\n\r\n\t\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\t\taggregated = res.body;\r\n\t\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t\t}"
  },
  {
    "path": "src/routes/(app)/chapter/[id]/edit/+page.svelte",
    "label": "key upload/editor by immutable actual target",
    "sha256": "05d8e7dfb6e1e3d145f4a1b9f52d173423feeb11b41733d3a5e9595a25137012",
    "count": 1,
    "before": "\t<UploadChapterEdit bind:chapter manga={chapter.relationships.manga} {user} />",
    "after": "\t{#key data.chapter.id}\r\n\t\t<UploadChapterEdit bind:chapter manga={chapter.relationships.manga} {user} />\r\n\t{/key}",
    "approved_title_context_exception": "chapter_edit_target_key"
  },
  {
    "path": "src/routes/(app)/title/[id]/upload/+page.svelte",
    "label": "key upload/editor by immutable actual target",
    "sha256": "0625ce874f8b332aec8e742919634676cf58e2b0a8dd78225dcc928425f607d0",
    "count": 1,
    "before": "\t<UploadChapter manga={data.manga} {user} />",
    "after": "\t{#key data.manga.id}\r\n\t\t<UploadChapter manga={data.manga} {user} />\r\n\t{/key}",
    "approved_title_context_exception": "upload_target_key"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "upload keyed lifetime uses one immutable target browser owner",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport { targetOwner } from \"$platform/target-owner\";"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "upload immutable target and actor constructor",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "\tconst mutex = new Mutex();",
    "after": "\tconst mutex = new Mutex();\r\n\tconst owner = targetOwner(() => manga.id, user.id);"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "upload begin captures immutable target and browser actor",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "\t\t\t\tconst res = await API.UploadSession.Begin(\r\n\t\t\t\t\tmanga.id,\r\n\t\t\t\t\tgroups.map(e => e.id)\r\n\t\t\t\t);",
    "after": "\t\t\t\tconst res = await owner.request<UploadSession>(\"/upload/begin\", \"POST\", { manga: owner.id, groups: groups.map(e => e.id) });"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "approval read stays in live editor owner",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "const res = await API.UploadSession.CheckApprovalRequired(draft.language);",
    "after": "const res = await owner.request(\"/upload/check-approval-required\", \"POST\", { language: draft.language });"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "chapter commit captures session id and actor without mutation replay",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "const res = await API.UploadSession.Commit(uploadSession.id, payload);",
    "after": "const sessionId = uploadSession.id;\r\r\n\t\t\t\tconst res = await owner.request<Chapter>(`/upload/${sessionId}/commit`, \"POST\", payload);"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "upload keyed lifetime uses one immutable target browser owner",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport { targetOwner } from \"$platform/target-owner\";"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "upload immutable target and actor constructor",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "\tconst mutex = new Mutex();",
    "after": "\tconst mutex = new Mutex();\r\n\tconst owner = targetOwner(() => chapter.id, user.id);"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "metadata save stays bound to old chapter and browser actor",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "const res = await API.Chapter.Update(chapter.id, payload);",
    "after": "const res = await owner.request<Chapter>(`/chapter/${owner.id}`, \"PUT\", payload);"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "chapter claim stays bound to old chapter and browser actor",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "const res = await API.Chapter.Claim(chapter.id);",
    "after": "const res = await owner.request<Chapter>(`/chapter/${owner.id}/claim`, \"POST\");"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "chapter deletion stays bound to old chapter and browser actor",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "const res = await API.Chapter.Delete(chapter.id);",
    "after": "const res = await owner.request(`/chapter/${owner.id}`, \"DELETE\");"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "page edit begin captures immutable chapter and saved version",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "const res = await API.UploadSession.BeginEdit(chapter.id, {\r\n\t\t\t\t\treplace: submitter === \"replace\",\r\n\t\t\t\t\tversion: chapter.version\r\n\t\t\t\t});",
    "after": "const version = chapter.version;\r\n\t\t\t\tconst res = await owner.request<UploadSession>(`/upload/begin/${owner.id}`, \"POST\", { replace: submitter === \"replace\", version });"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "chapter commit captures session id and actor without mutation replay",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "const res = await API.UploadSession.Commit(uploadSession.id, payload);",
    "after": "const sessionId = uploadSession.id;\r\r\n\t\t\t\tconst res = await owner.request<Chapter>(`/upload/${sessionId}/commit`, \"POST\", payload);"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "metadata dirty state and active operations cannot silently switch chapter",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "\tlet isSubmitting = $state(false);",
    "after": "\tlet isSubmitting = $state(false);\r\n\tbeforeNavigate(navigation => { if (!navigation.willUnload && (isChanged || isSubmitting)) { navigation.cancel(); toast.info(\"Chapter changes are not saved\", { description: \"Save or reset the current metadata before leaving. No other chapter was changed.\" }); } });"
  },
  {
    "path": "src/routes/(app)/chapter/[id]/edit/+page.svelte",
    "label": "editor actor initialization belongs only to current target and account",
    "sha256": "05d8e7dfb6e1e3d145f4a1b9f52d173423feeb11b41733d3a5e9595a25137012",
    "count": 1,
    "before": "\tonMount(async () => {\r\n\t\ttry {\r\n\t\t\tawait Promise.all([getUser()]);\r\n\t\t} catch (e) {\r\n\t\t\tif (e instanceof Constants.Error.Error) error = e;\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\terror = Constants.Error.Unknown;\r\n\t\t\t}\r\n\t\t}\r\n\t\tisMounted = true;\r\n\t});",
    "after": "\tlet lookupGeneration = 0, lookupDisposed = false;\r\n\tonDestroy(() => { lookupDisposed = true; lookupGeneration++; user = undefined; });\r\n\t$effect(() => {\r\n\t\tconst target = data.chapter?.id, actor = $Session?.identity.id, generation = ++lookupGeneration;\r\n\t\tuser = undefined; error = undefined; isMounted = false;\r\n\t\tif (!target || !actor) return;\r\n\t\tvoid API.User.Get(actor).then(res => {\r\n\t\t\tif (lookupDisposed || generation !== lookupGeneration || actor !== $Session?.identity.id || target !== data.chapter?.id) return;\r\n\t\t\tif (res?.status === StatusCodes.OK && res.body?.id === actor) { user = res.body; isMounted = true; }\r\n\t\t\telse throw res?.status === StatusCodes.TOO_MANY_REQUESTS ? Constants.Error.TooManyRequests : Constants.Error.Unknown;\r\n\t\t}).catch(cause => { if (!lookupDisposed && generation === lookupGeneration) error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; });\r\n\t});"
  },
  {
    "path": "src/routes/(app)/chapter/[id]/edit/+page.svelte",
    "label": "editor parent async lookup lifecycle import",
    "sha256": "05d8e7dfb6e1e3d145f4a1b9f52d173423feeb11b41733d3a5e9595a25137012",
    "count": 1,
    "before": "\timport { onMount } from \"svelte\";",
    "after": "\timport { onDestroy, onMount } from \"svelte\";"
  },
  {
    "path": "src/routes/(app)/chapter/[id]/edit/+page.svelte",
    "label": "remove unused unfenced parent actor lookup",
    "sha256": "05d8e7dfb6e1e3d145f4a1b9f52d173423feeb11b41733d3a5e9595a25137012",
    "count": 1,
    "before": "\tconst getUser = async () => {\r\n\t\tif (!$Session) return;\r\n\r\n\t\tconst res = await API.User.Get($Session.identity.id);\r\n\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\tuser = res.body;\r\n\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t} else throw Constants.Error.Unknown;\r\n\t};",
    "after": ""
  },
  {
    "path": "src/routes/(app)/title/[id]/upload/+page.svelte",
    "label": "editor actor initialization belongs only to current target and account",
    "sha256": "0625ce874f8b332aec8e742919634676cf58e2b0a8dd78225dcc928425f607d0",
    "count": 1,
    "before": "\tonMount(async () => {\r\n\t\ttry {\r\n\t\t\tawait getUser();\r\n\t\t} catch (e) {\r\n\t\t\tif (e instanceof Constants.Error.Error) error = e;\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\terror = Constants.Error.Unknown;\r\n\t\t\t}\r\n\t\t}\r\n\t\tisMounted = true;\r\n\t});",
    "after": "\tlet lookupGeneration = 0, lookupDisposed = false;\r\n\tonDestroy(() => { lookupDisposed = true; lookupGeneration++; user = undefined; });\r\n\t$effect(() => {\r\n\t\tconst target = data.manga?.id, actor = $Session?.identity.id, generation = ++lookupGeneration;\r\n\t\tuser = undefined; error = undefined; isMounted = false;\r\n\t\tif (!target || !actor) return;\r\n\t\tvoid API.User.Get(actor).then(res => {\r\n\t\t\tif (lookupDisposed || generation !== lookupGeneration || actor !== $Session?.identity.id || target !== data.manga?.id) return;\r\n\t\t\tif (res?.status === StatusCodes.OK && res.body?.id === actor) { user = res.body; isMounted = true; }\r\n\t\t\telse throw res?.status === StatusCodes.TOO_MANY_REQUESTS ? Constants.Error.TooManyRequests : Constants.Error.Unknown;\r\n\t\t}).catch(cause => { if (!lookupDisposed && generation === lookupGeneration) error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; });\r\n\t});"
  },
  {
    "path": "src/routes/(app)/title/[id]/upload/+page.svelte",
    "label": "editor parent async lookup lifecycle import",
    "sha256": "0625ce874f8b332aec8e742919634676cf58e2b0a8dd78225dcc928425f607d0",
    "count": 1,
    "before": "\timport { onMount } from \"svelte\";",
    "after": "\timport { onDestroy, onMount } from \"svelte\";"
  },
  {
    "path": "src/routes/(app)/title/[id]/upload/+page.svelte",
    "label": "remove unused unfenced parent actor lookup",
    "sha256": "0625ce874f8b332aec8e742919634676cf58e2b0a8dd78225dcc928425f607d0",
    "count": 1,
    "before": "\tconst getUser = async () => {\r\n\t\tif (!$Session) return;\r\n\r\n\t\tconst res = await API.User.Get($Session.identity.id);\r\n\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\tuser = res.body;\r\n\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t} else throw Constants.Error.Unknown;\r\n\t};",
    "after": ""
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "parent supplies immutable upload actor context",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "\timport { targetOwner } from \"$platform/target-owner\";",
    "after": "\timport { targetOwner } from \"$platform/target-owner\";\r\n\timport { setUploadOwner } from \"$platform/upload-batch\";"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "upload actor context is set before child staging controls",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "\tconst owner = targetOwner(() => manga.id, user.id);",
    "after": "\tconst owner = targetOwner(() => manga.id, user.id);\r\n\tsetUploadOwner(owner);"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "parent supplies immutable upload actor context",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "\timport { targetOwner } from \"$platform/target-owner\";",
    "after": "\timport { targetOwner } from \"$platform/target-owner\";\r\n\timport { setUploadOwner } from \"$platform/upload-batch\";"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "upload actor context is set before child staging controls",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "\tconst owner = targetOwner(() => chapter.id, user.id);",
    "after": "\tconst owner = targetOwner(() => chapter.id, user.id);\r\n\tsetUploadOwner(owner);"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "upload recovery shares owned actor and terminal batch state",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\timport { PUBLIC_API_URL } from \"$env/static/public\";",
    "after": "\timport { PUBLIC_API_URL } from \"$env/static/public\";\r\n\timport { getUploadOwner, runUploadBatch, resolveUploadPage, markUnsentUploadPages } from \"$platform/upload-batch\";"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "private page uploads capture parent actor lifetime",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\tconst mutex = getMutex();",
    "after": "\tconst mutex = getMutex();\r\n\tconst owner = getUploadOwner();"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "batch failure resolves exact receipts or leaves visible nonreplayable errors",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\tconst uploadChunk = async (chunk: UploadSessionFile[], onProgress?: (delta: number) => void) => {\r\n\t\tif (!uploadSession) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tconst form = new FormData();\r\n\t\tchunk.forEach(e => form.append(\"file\", e.file!));\r\n\r\n\t\tconst http = new HTTP<UploadSessionFile[]>(`${PUBLIC_API_URL}/upload/${uploadSession.id}`, form);\r\n\t\thttp.setWithCredentials(true);\r\n\r\n\t\tif (onProgress) {\r\n\t\t\tlet lastLoaded = 0;\r\n\t\t\thttp.addUploadListener(\"progress\", e => {\r\n\t\t\t\tif (e.lengthComputable) {\r\n\t\t\t\t\tconst delta = e.loaded - lastLoaded;\r\n\t\t\t\t\tlastLoaded = e.loaded;\r\n\t\t\t\t\tonProgress(delta);\r\n\t\t\t\t}\r\n\t\t\t});\r\n\t\t}\r\n\r\n\t\tconst res = await http.POST();\r\n\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\tres.body.forEach((remote, i) => {\r\n\t\t\t\tconst idx = pages.findIndex(e => e.id === chunk[i].id);\r\n\t\t\t\tif (idx >= 0) {\r\n\t\t\t\t\tconst local = pages[idx];\r\n\t\t\t\t\tremote.id = remote.id ?? local.id;\r\n\t\t\t\t\tremote.file = remote.error ? local.file : undefined;\r\n\t\t\t\t\tremote.thumbnail = local.thumbnail;\r\n\t\t\t\t\tpages[idx] = remote;\r\n\t\t\t\t}\r\n\t\t\t});\r\n\t\t\tpages = pages;\r\n\t\t} else throw new Constants.Error.Error(\"Upload was not confirmed\", \"The server did not accept this batch. Staged files were retained; reload its current receipt before retrying.\");\r\n\t};\r\n",
    "after": "\tconst uploadChunk = async (chunk: UploadSessionFile[], onProgress?: (delta: number) => void) => {\r\n\t\tif (!uploadSession) return;\r\n\t\tconst session = uploadSession;\r\n\t\ttry {\r\n\t\t\tconst result = await runUploadBatch(session, chunk, owner, onProgress); owner.assert();\r\n\t\t\tif (uploadSession?.id !== session.id) throw new Constants.Error.Error(\"Upload changed\", \"Old batch results were discarded. No mutation was replayed.\");\r\n\t\t\tresult.forEach((remote, index) => { const position = pages.findIndex(page => page.id === chunk[index].id); if (position >= 0) pages[position] = remote; });\r\n\t\t\tpages = pages;\r\n\t\t} catch (error) { if (owner.alive()) pages = pages; throw error; }\r\n\t};\r\n"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "later unattempted chunks become removable local errors instead of spinners",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\t\t\tawait uploadChunk(chunk, delta => {\r\n\t\t\t\tprogress.loaded += delta;\r\n\t\t\t});",
    "after": "\t\t\ttry { await uploadChunk(chunk, delta => { progress.loaded += delta; }); }\r\n\t\t\tcatch (error) { if (owner.alive()) { markUnsentUploadPages(newFiles, uploadSession.id); pages = pages; progress = { total: 0, loaded: 0 }; } throw error; }"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "unknown local replacements must reconcile before any new POST",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\t\tconst target = targetFile, idx = pages.indexOf(target); if (idx < 0) return;",
    "after": "\t\tlet target = targetFile; const idx = pages.indexOf(target); if (idx < 0) return;\r\n\t\towner.assert();\r\n\t\tif (target.upload_recovery?.state === \"unknown\") { target = await resolveUploadPage(uploadSession, target, owner); owner.assert(); pages[idx] = target; }"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "replacement uncertainty retains actual new candidate receipt context",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\t\t\tcatch (error) { pages[idx] = target; revokeThumbnail(candidate); throw error; }",
    "after": "\t\t\tcatch (error) { if (owner.alive()) { if (candidate.upload_recovery?.state !== \"unknown\") { pages[idx] = target; revokeThumbnail(candidate); } else revokeThumbnail(target); } throw error; }"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "bulk removal reconciles ambiguous batch receipts before clearing pages",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\t\t\t\tconst ids = pages.filter(e => !e.file && !e.error).map(e => e.id);",
    "after": "\t\t\t\tconst resolved: UploadSessionFile[] = [];\r\n\t\t\t\tfor (const page of pages) resolved.push(await resolveUploadPage(uploadSession, page, owner));\r\n\t\t\t\towner.assert();\r\n\t\t\t\tconst ids = [...new Set(resolved.filter(e => !e.file && !e.error).map(e => e.id))];"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "bulk removal uses captured browser authority",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "const res = await API.UploadSession.DeleteFiles(uploadSession.id, ids);",
    "after": "const res = await owner.request(`/upload/${uploadSession.id}/batch`, \"DELETE\", ids);"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPagesEntry.svelte",
    "label": "per-item recovery shares upload actor",
    "sha256": "87bce9e9394fd6301e81821ea4aa78e661b14aae6f5616db249a938f11879fad",
    "count": 1,
    "before": "\timport { Constants, Mutex } from \"$lib\";",
    "after": "\timport { Constants, Mutex } from \"$lib\";\r\n\timport { getUploadOwner, resolveUploadPage } from \"$platform/upload-batch\";"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPagesEntry.svelte",
    "label": "per-item recovery uses parent actor lifetime",
    "sha256": "87bce9e9394fd6301e81821ea4aa78e661b14aae6f5616db249a938f11879fad",
    "count": 1,
    "before": "\tconst mutex = getMutex();",
    "after": "\tconst mutex = getMutex();\r\n\tconst owner = getUploadOwner();"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPagesEntry.svelte",
    "label": "unknown per-item deletion resolves actual durable file without replay",
    "sha256": "87bce9e9394fd6301e81821ea4aa78e661b14aae6f5616db249a938f11879fad",
    "count": 1,
    "before": "\t\t\t\tif (!page.file) {\r\n\t\t\t\t\tconst res = await API.UploadSession.DeleteFile(uploadSession.id, page.id);",
    "after": "\t\t\t\tconst resolved = await resolveUploadPage(uploadSession, page, owner); owner.assert();\r\n\t\t\t\tif (!resolved.file) {\r\n\t\t\t\t\tconst res = await owner.request(`/upload/${uploadSession.id}/${resolved.id}`, \"DELETE\");"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPagesEntry.svelte",
    "label": "local page discard asserts current owner before changing bound staging",
    "sha256": "87bce9e9394fd6301e81821ea4aa78e661b14aae6f5616db249a938f11879fad",
    "count": 1,
    "before": "\t\t\t\tconst idx = pages.indexOf(page);",
    "after": "\t\t\t\towner.assert();\r\n\t\t\t\tconst idx = pages.indexOf(page);"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "cancel staged upload reuses supplied confirmation dialog",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "\timport Guidelines from \"$lib/components/Guidelines.svelte\";",
    "after": "\timport Guidelines from \"$lib/components/Guidelines.svelte\";\r\n\timport ConfirmDialog from \"$lib/components/ConfirmDialog.svelte\";"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "explicit cancel captures session actor and only clears after204",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "\tconst isPending = async () => {",
    "after": "\tconst cancelCurrentUpload = () => mutex.wait(async () => {\r\n\t\tif (!uploadSession || uploadSession.can_cancel !== true || uploadSession.is_committed) return;\r\n\t\tconst staged = uploadSession; owner.assert();\r\n\t\ttry {\r\n\t\t\tconst current = await currentUpload(owner); owner.assert();\r\n\t\t\tif (!current || current.id !== staged.id || current.can_cancel !== true) throw resumeError(409);\r\n\t\t\tawait cancelResume(current, owner); owner.assert();\r\n\t\t\tif (uploadSession?.id !== staged.id) throw resumeError(409);\r\n\t\t\tfor (const file of staged.relationships.files ?? []) if (file.thumbnail?.startsWith(\"blob:\")) URL.revokeObjectURL(file.thumbnail);\r\n\t\t\twindow.clearTimeout(resumeSaveTimer); uploadSession = undefined; lastSavedUpload = \"\";\r\n\t\t\tdraft = { language: draft.language }; isOneshot = false; isTemsAccepted = false;\r\n\t\t\ttoast.success(\"Staged upload cancelled\", { description: \"The server confirmed cancellation. No chapter publication was cancelled or peer copy removed.\" });\r\n\t\t} catch (error) { showResumeError(error); }\r\n\t});\r\n\r\n\tconst isPending = async () => {"
  },
  {
    "path": "src/lib/components/upload/UploadChapter.svelte",
    "label": "parent-approved explicit mutable staging cancel control",
    "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
    "count": 1,
    "before": "\t\t\t{#if uploadSession}\r\n\t\t\t\t<Button class=\"w-50 border-none\"",
    "after": "\t\t\t{#if uploadSession}\r\n\t\t\t\t{#if uploadSession.can_cancel === true && !uploadSession.is_committed}\r\n\t\t\t\t\t<ConfirmDialog title=\"Cancel staged upload?\" description=\"Discard this private staging session and its files? Prepared publications cannot be cancelled here.\" confirmText=\"Cancel staged upload\" onConfirm={dialogMutex => dialogMutex.exec(cancelCurrentUpload)} disabled={$mutex}>\r\n\t\t\t\t\t\t{#snippet child({ props })}<Button class=\"border-none\" variant=\"outline\" type=\"button\" {...props}>Cancel staged upload</Button>{/snippet}\r\n\t\t\t\t\t</ConfirmDialog>\r\n\t\t\t\t{/if}\r\n\t\t\t\t<Button class=\"w-50 border-none\"",
    "approved_upload_recovery_exception": "cancel_staged_upload_control"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "parent-approved Revert remains available for invalid dirty local metadata",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "\t\t\t\t\t\t\ttype=\"reset\"\r\n\t\t\t\t\t\t\tdisabled={$mutex || !isValid || !isChanged}",
    "after": "\t\t\t\t\t\t\ttype=\"reset\"\r\n\t\t\t\t\t\t\tdisabled={$mutex || !isChanged || !!uploadSession}",
    "approved_upload_recovery_exception": "invalid_metadata_reset_guard"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "existing replacement reads current expected actor staging",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\t\tconst current = await API.UploadSession.Get();",
    "after": "\t\tconst current = await owner.request<UploadSession>(\"/upload\", \"GET\");"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "atomic replacement request remains bound to current actor",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\t\thttp.setWithCredentials(true); http.setRequestHeader(\"X-WeebDex-Upload-Version\", String(current.body.version));",
    "after": "\t\thttp.setWithCredentials(true); http.setRequestHeader(\"X-WeebDex-Upload-Version\", String(current.body.version)); http.setRequestHeader(\"X-WeebDex-Expected-Account\", owner.actor); owner.assert();"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "replacement result cannot attach after editor authority disposal",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\t\t\tconst result = await http.POST();",
    "after": "\t\t\tconst result = await http.POST(); owner.assert();"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "lost replacement response reconciliation pins browser actor",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\t\t\tconst latest = await API.UploadSession.Get().catch(() => undefined), confirmed = latest?.body?.relationships.files?.find(row => row.id === target.id);",
    "after": "\t\t\tconst latest = await owner.request<UploadSession>(\"/upload\", \"GET\").catch(() => undefined); owner.assert(); const confirmed = latest?.body?.relationships.files?.find(row => row.id === target.id);"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "bulk local discard cannot mutate disposed staging",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\t\t\t\tpages.forEach(revokeThumbnail);",
    "after": "\t\t\t\towner.assert(); pages.forEach(revokeThumbnail);"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "duplicate removal cannot discard unresolved accepted-upload placeholders",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\t\t\t\tconst unique = new Map<string, UploadSessionFile>();",
    "after": "\t\t\t\towner.assert();\r\n\t\t\t\tif (pages.some(page => page.upload_recovery?.state === \"unknown\")) throw new Constants.Error.Error(\"Reconcile upload first\", \"Unknown page outcomes cannot be discarded as duplicates. Reload exact owner staging or explicitly cancel this upload.\");\r\n\t\t\t\tconst unique = new Map<string, UploadSessionFile>();"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "duplicate deletion uses captured upload actor",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "const res = await API.UploadSession.DeleteFiles(uploadSession.id, duplicateIds);",
    "after": "const res = await owner.request(`/upload/${uploadSession.id}/batch`, \"DELETE\", duplicateIds);"
  },
  {
    "path": "src/routes/(app)/auth/recovery/+page.svelte",
    "label": "parent-approved recovery-only route boundary delegates retained anonymous form",
    "sha256": "20ba760776b71854e74c17aef228abd234dd5bded650c4fe863a1a94e1115fcf",
    "count": 1,
    "before": "<script lang=\"ts\">\r\n\timport { goto, replaceState } from \"$app/navigation\";\r\n\timport { page } from \"$app/state\";\r\n\timport { PUBLIC_SITE_NAME } from \"$env/static/public\";\r\n\timport { Constants, Kratos, Mutex } from \"$lib\";\r\n\timport { Button } from \"$lib/components/ui/button\";\r\n\timport * as Card from \"$lib/components/ui/card\";\r\n\timport * as Field from \"$lib/components/ui/field\";\r\n\timport { Input } from \"$lib/components/ui/input\";\r\n\timport Spinner from \"$lib/components/ui/spinner/spinner.svelte\";\r\n\timport { ErrorSessionAlreadyAvailable, type UINode } from \"$lib/Kratos\";\r\n\timport { Session } from \"$lib/stores\";\r\n\timport type {\r\n\t\tFrontendApiUpdateRecoveryFlowRequest,\r\n\t\tGenericError,\r\n\t\tNeedsPrivilegedSessionError,\r\n\t\tRecoveryFlow,\r\n\t\tUiNode\r\n\t} from \"@ory/client\";\r\n\timport { AxiosError, type AxiosResponse } from \"axios\";\r\n\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { onMount } from \"svelte\";\r\n\timport { toast } from \"svelte-sonner\";\r\n\timport { fade } from \"svelte/transition\";\r\n\r\n\tconst id = $props.id();\r\n\tconst mutex = new Mutex();\r\n\tconst { pathname } = window.location;\r\n\r\n\tlet isMounted = $state(false);\r\n\tlet ref = $state<HTMLFormElement>();\r\n\tlet flow = $state<RecoveryFlow>();\r\n\tlet nodes = $state<{\r\n\t\tcsrf_token?: UINode;\r\n\t\temail?: UINode;\r\n\t}>({});\r\n\r\n\tonMount(async () => {\r\n\t\tconst redirect = decodeURIComponent(page.url.searchParams.get(\"redirect\") ?? \"/\");\r\n\t\tif (await Session.isValid()) {\r\n\t\t\treturn goto(redirect);\r\n\t\t}\r\n\r\n\t\ttry {\r\n\t\t\tlet promise: Promise<AxiosResponse<RecoveryFlow>>;\r\n\t\t\tif (page.url.searchParams.has(\"flow\")) {\r\n\t\t\t\tpromise = Kratos.getRecoveryFlow({ id: page.url.searchParams.get(\"flow\")! });\r\n\t\t\t} else {\r\n\t\t\t\tpromise = Kratos.createBrowserRecoveryFlow({ returnTo: redirect });\r\n\t\t\t}\r\n\r\n\t\t\tconst res = await promise.catch(async (err: AxiosError<RecoveryFlow & GenericError>) => {\r\n\t\t\t\tif (\r\n\t\t\t\t\terr.status === StatusCodes.BAD_REQUEST &&\r\n\t\t\t\t\t<string>err.response?.data.error?.id === ErrorSessionAlreadyAvailable\r\n\t\t\t\t) {\r\n\t\t\t\t\tthrow ErrorSessionAlreadyAvailable;\r\n\t\t\t\t}\r\n\t\t\t\treturn err.status === StatusCodes.GONE && err.response?.data\r\n\t\t\t\t\t? Kratos.createBrowserRecoveryFlow({ returnTo: err.response.data.return_to || redirect })\r\n\t\t\t\t\t: err;\r\n\t\t\t});\r\n\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tif (res instanceof AxiosError) {\r\n\t\t\t\tif (res.response?.data.error?.message) {\r\n\t\t\t\t\ttoast.error(res.response.data.error.message, { duration: 10000 });\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} else if (res.status === StatusCodes.OK && res.data) {\r\n\t\t\t\tconst q = new URLSearchParams(window.location.search);\r\n\t\t\t\tq.set(\"flow\", res.data.id);\r\n\t\t\t\tq.delete(\"redirect\");\r\n\r\n\t\t\t\tflow = res.data;\r\n\t\t\t\treplaceState(window.location.pathname + q.toString(), page.state);\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (e === ErrorSessionAlreadyAvailable) goto(redirect);\r\n\t\t\telse if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t}\r\n\t\tisMounted = true;\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tflow?.ui.messages?.forEach(message => {\r\n\t\t\tswitch (message.type) {\r\n\t\t\t\tcase \"success\":\r\n\t\t\t\t\ttoast.success(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t\tcase \"info\":\r\n\t\t\t\t\ttoast.info(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t\tdefault:\r\n\t\t\t\t\ttoast.error(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t}\r\n\t\t});\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tif (flow?.ui.nodes) {\r\n\t\t\tnodes = flow.ui.nodes.reduce(\r\n\t\t\t\t(acc, node) => {\r\n\t\t\t\t\tconst name = <string>(<any>node.attributes).name;\r\n\t\t\t\t\tif ([\"csrf_token\", \"email\"].includes(name)) {\r\n\t\t\t\t\t\t(<UiNode>acc[<keyof typeof nodes>name.replaceAll(\"traits.\", \"\")]) = node;\r\n\t\t\t\t\t}\r\n\t\t\t\t\treturn acc;\r\n\t\t\t\t},\r\n\t\t\t\t{} as typeof nodes\r\n\t\t\t);\r\n\t\t}\r\n\t});\r\n\r\n\tconst updateFlow = async (method: \"link\", body: any) => {\r\n\t\tif (!flow || !ref) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tconst request: FrontendApiUpdateRecoveryFlowRequest = {\r\n\t\t\tflow: flow.id,\r\n\t\t\tupdateRecoveryFlowBody: {\r\n\t\t\t\tmethod: method,\r\n\t\t\t\tcsrf_token: ref.csrf_token.value,\r\n\t\t\t\t...body\r\n\t\t\t}\r\n\t\t};\r\n\r\n\t\ttry {\r\n\t\t\ttype T =\r\n\t\t\t\t| AxiosResponse<RecoveryFlow & NeedsPrivilegedSessionError>\r\n\t\t\t\t| AxiosError<RecoveryFlow & NeedsPrivilegedSessionError>;\r\n\t\t\tconst res: T = await Kratos.updateRecoveryFlow(request).catch(err => err);\r\n\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tconst data = res instanceof AxiosError ? res.response?.data : res.data;\r\n\t\t\tif (data?.redirect_browser_to) {\r\n\t\t\t\twindow.location.href = data.redirect_browser_to;\r\n\t\t\t} else if (data?.ui) {\r\n\t\t\t\tflow = data;\r\n\t\t\t} else if (data?.error?.reason) {\r\n\t\t\t\ttoast.error(data.error.reason, { duration: 10000 });\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t}\r\n\t};\r\n\r\n\tconst onSubmit = async (ev: SubmitEvent) => {\r\n\t\tev.preventDefault();\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tif (!ref) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\t\t\treturn updateFlow(\"link\", {\r\n\t\t\t\temail: ref.email.value\r\n\t\t\t});\r\n\t\t});\r\n\t};\r\n</script>\r\n\r\n<svelte:head>\r\n\t<title>Account Recovery - {PUBLIC_SITE_NAME}</title>\r\n</svelte:head>\r\n\r\n{#if isMounted}\r\n\t<Card.Root class=\"relative rounded\">\r\n\t\t<Card.Header class=\"text-center\">\r\n\t\t\t<Card.Title class=\"text-xl\">Account Recovery</Card.Title>\r\n\t\t\t<Card.Description>Enter your email address to receive a recovery link</Card.Description>\r\n\t\t</Card.Header>\r\n\t\t<Card.Content>\r\n\t\t\t<form onsubmit={onSubmit} bind:this={ref}>\r\n\t\t\t\t<Field.Group>\r\n\t\t\t\t\t{#if nodes.email}\r\n\t\t\t\t\t\t{@const isInvalid = !!nodes.email.messages?.length}\r\n\t\t\t\t\t\t<Field.Field data-invalid={isInvalid}>\r\n\t\t\t\t\t\t\t<Field.Label for=\"email-{id}\">Email</Field.Label>\r\n\t\t\t\t\t\t\t<Input\r\n\t\t\t\t\t\t\t\tclass=\"border-none !ring-0 !ring-transparent\"\r\n\t\t\t\t\t\t\t\tid=\"email-{id}\"\r\n\t\t\t\t\t\t\t\ttype=\"email\"\r\n\t\t\t\t\t\t\t\tname=\"email\"\r\n\t\t\t\t\t\t\t\tvalue={nodes.email.attributes.value}\r\n\t\t\t\t\t\t\t\tautocomplete={nodes.email.attributes.autocomplete}\r\n\t\t\t\t\t\t\t\tminlength={3}\r\n\t\t\t\t\t\t\t\tmaxlength={320}\r\n\t\t\t\t\t\t\t\trequired={nodes.email.attributes.required}\r\n\t\t\t\t\t\t\t\tdisabled={nodes.email.attributes.type === \"hidden\" || $mutex}\r\n\t\t\t\t\t\t\t\taria-invalid={isInvalid}\r\n\t\t\t\t\t\t\t/>\r\n\t\t\t\t\t\t\t{#each nodes.email.messages as message}\r\n\t\t\t\t\t\t\t\t<Field.Error>{message.text}</Field.Error>\r\n\t\t\t\t\t\t\t{/each}\r\n\t\t\t\t\t\t</Field.Field>\r\n\t\t\t\t\t{/if}\r\n\t\t\t\t\t<Field.Field>\r\n\t\t\t\t\t\t<Button type=\"submit\" disabled={$mutex}>Send Recovery Link</Button>\r\n\t\t\t\t\t</Field.Field>\r\n\t\t\t\t</Field.Group>\r\n\t\t\t\t{#if nodes.csrf_token}\r\n\t\t\t\t\t<input type=\"hidden\" name=\"csrf_token\" value={nodes.csrf_token.attributes.value} required />\r\n\t\t\t\t{/if}\r\n\t\t\t</form>\r\n\t\t</Card.Content>\r\n\t\t{#if $mutex}\r\n\t\t\t<div class=\"absolute inset-0 flex bg-black/25\" transition:fade={{ duration: 100 }}>\r\n\t\t\t\t<Spinner class=\"m-auto size-10\" />\r\n\t\t\t</div>\r\n\t\t{/if}\r\n\t</Card.Root>\r\n{:else}\r\n\t<Spinner class=\"m-auto size-10\" />\r\n{/if}\r\n",
    "after": "<script lang=\"ts\">\r\n\timport RecoveryBoundary from \"$platform/RecoveryBoundary.svelte\";\r\n</script>\r\n\r\n<RecoveryBoundary />\r\n",
    "approved_recovery_extension": "reset_only_boundary"
  },
  {
    "path": "src/lib/stores/Session.ts",
    "label": "Session reads reject reset-only capability and malformed identity",
    "sha256": "8baf2edac726343a1f1445c53afa7f9a5338c66fad41ffbbe6ac7285d5a25498",
    "count": 1,
    "before": "if (res?.status === StatusCodes.OK && res.body) {",
    "after": "if (res?.status === StatusCodes.OK && res.body?.identity?.id && res.body.identity.username) {"
  },
  {
    "path": "src/lib/stores/Session.ts",
    "label": "reset-only capability is cleared rather than hydrated as Session",
    "sha256": "8baf2edac726343a1f1445c53afa7f9a5338c66fad41ffbbe6ac7285d5a25498",
    "count": 1,
    "before": "} else if (res?.status === StatusCodes.UNAUTHORIZED) {",
    "after": "} else if (res?.status === StatusCodes.UNAUTHORIZED || res?.status === StatusCodes.FORBIDDEN || res?.status === StatusCodes.OK && !res.body?.identity?.id) {"
  },
  {
    "path": "src/lib/stores/Session.ts",
    "label": "session initializer tracks newer writes across awaited authority checks",
    "sha256": "8baf2edac726343a1f1445c53afa7f9a5338c66fad41ffbbe6ac7285d5a25498",
    "count": 1,
    "before": "const mutex = new Mutex();",
    "after": "const mutex = new Mutex();\r\nlet sessionGeneration = 0;\r\nstore.subscribe(() => { sessionGeneration++; });"
  },
  {
    "path": "src/lib/stores/Session.ts",
    "label": "initializer never restores a prelogout response or its private preferences",
    "sha256": "8baf2edac726343a1f1445c53afa7f9a5338c66fad41ffbbe6ac7285d5a25498",
    "count": 1,
    "before": "\t\tconst res = await API.Auth.Check();",
    "after": "\t\tconst generation = sessionGeneration;\r\n\t\tconst res = await API.Auth.Check();\r\n\t\tif (generation !== sessionGeneration) return;"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "round6 page-edit submit prevents native navigation before mutex or awaits",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "\tconst createUploadSession = (ev: SubmitEvent) =>\r\n\t\tmutex.exec(async () => {",
    "after": "\tconst createUploadSession = (ev: SubmitEvent) => {\r\n\t\tev.preventDefault(); ev.stopPropagation(); ev.stopImmediatePropagation();\r\n\t\treturn createUploadSessionOwned(ev);\r\n\t};\r\n\tconst createUploadSessionOwned = (ev: SubmitEvent) =>\r\n\t\tmutex.exec(async () => {"
  },
  {
    "path": "src/lib/components/upload/UploadChapterEdit.svelte",
    "label": "round6 dirty metadata cannot silently enter pages-only staging",
    "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
    "count": 1,
    "before": "\tconst createUploadSessionOwned = (ev: SubmitEvent) =>\r\n\t\tmutex.exec(async () => {",
    "after": "\tconst createUploadSessionOwned = (ev: SubmitEvent) =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tif (isChanged) { toast.info(\"Save or revert chapter metadata first\", { description: \"Your displayed metadata and groups were kept. Page editing cannot begin until these changes are saved or reverted.\" }); return; }"
  },
  {
    "path": "src/lib/API/Report.ts",
    "label": "round6 report API uses captured actor transport",
    "sha256": "469d4ecec0ae2ab0229b57c5e0e59e9c176d5fe6ebce0c3a73c5f449d3d951d8",
    "count": 1,
    "before": "import { Constants, HTTP } from \"$lib\";",
    "after": "import { Constants, HTTP } from \"$lib\";\r\nimport { authenticatedMutation, type MutationIntent } from \"$platform/authenticated-mutation\";"
  },
  {
    "path": "src/lib/API/Report.ts",
    "label": "round6 every report mutation is actor-bound without replay",
    "sha256": "469d4ecec0ae2ab0229b57c5e0e59e9c176d5fe6ebce0c3a73c5f449d3d951d8",
    "count": 1,
    "before": "export const Create = (payload: CreateReportRequest) => {\r\n\tconst http = new HTTP(`${PUBLIC_API_URL}/report`, payload);\r\n\thttp.setWithCredentials(true);\r\n\treturn http.POST();\r\n};\r\n\r\nexport const Delete = (id: string) => {\r\n\tconst http = new HTTP(`${PUBLIC_API_URL}/report/${id}`);\r\n\thttp.setWithCredentials(true);\r\n\treturn http.DELETE();\r\n};\r\n\r\nexport const Accept = (id: string, notes?: string) => {\r\n\tconst http = new HTTP<Report>(`${PUBLIC_API_URL}/report/${id}/accept`, { notes });\r\n\thttp.setWithCredentials(true);\r\n\treturn http.POST();\r\n};\r\n\r\nexport const Reject = (id: string, notes?: string) => {\r\n\tconst http = new HTTP<Report>(`${PUBLIC_API_URL}/report/${id}/reject`, { notes });\r\n\thttp.setWithCredentials(true);\r\n\treturn http.POST();\r\n};\r\n",
    "after": "export const Create = (payload: CreateReportRequest, intent?: MutationIntent) => authenticatedMutation<void>(\"/report\", \"POST\", payload, intent);\r\nexport const Delete = (id: string, intent?: MutationIntent) => authenticatedMutation<void>(`/report/${encodeURIComponent(id)}`, \"DELETE\", undefined, intent);\r\nexport const Accept = (id: string, notes?: string, intent?: MutationIntent) => authenticatedMutation<Report>(`/report/${encodeURIComponent(id)}/accept`, \"POST\", { notes }, intent);\r\nexport const Reject = (id: string, notes?: string, intent?: MutationIntent) => authenticatedMutation<Report>(`/report/${encodeURIComponent(id)}/reject`, \"POST\", { notes }, intent);\r\n"
  },
  {
    "path": "src/lib/components/ReportSearchEntry.svelte",
    "label": "round6 report row owns mutation lifetime",
    "sha256": "dcbb141a5dbb92d29f15996544b4e02070ef32d1200aaf84392487ae32961e2d",
    "count": 1,
    "before": "\timport { toast } from \"svelte-sonner\";",
    "after": "\timport { toast } from \"svelte-sonner\";\r\n\timport { createMutationOwner } from \"$platform/authenticated-mutation\";"
  },
  {
    "path": "src/lib/components/ReportSearchEntry.svelte",
    "label": "round6 report row invalidates target changes",
    "sha256": "dcbb141a5dbb92d29f15996544b4e02070ef32d1200aaf84392487ae32961e2d",
    "count": 1,
    "before": "\tconst mutex = new Mutex();",
    "after": "\tconst mutex = new Mutex();\r\n\tconst mutationOwner = createMutationOwner(() => report.id);\r\n\t$effect(() => { report.id; mutationOwner.invalidate(); });"
  },
  {
    "path": "src/lib/components/ReportSearchEntry.svelte",
    "label": "round6 report results cannot mutate another row or account",
    "sha256": "dcbb141a5dbb92d29f15996544b4e02070ef32d1200aaf84392487ae32961e2d",
    "count": 1,
    "before": "\tlet isDeleting = $state(false);\r\n\tconst deleteReport = () =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tisDeleting = true;\r\n\t\t\ttry {\r\n\t\t\t\tconst res = await API.Report.Delete(report.id);\r\n\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\tconst idx = entries.findIndex(e => e.id === report.id);\r\n\t\t\t\t\tif (idx >= 0) {\r\n\t\t\t\t\t\tentries.splice(idx, 1);\r\n\t\t\t\t\t\tentries = entries;\r\n\t\t\t\t\t}\r\n\t\t\t\t\ttoast.success(\"Report deleted successfully\");\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else if (res?.status === StatusCodes.CONFLICT) {\r\n\t\t\t\t\tthrow Constants.Error.UpdateConflict;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t} finally {\r\n\t\t\t\tisDeleting = false;\r\n\t\t\t}\r\n\t\t});\r\n\r\n\tlet mutexState = $state<Constants.Report.Status>();\r\n\tconst updateStatus = (status: Constants.Report.Status, notes?: string) =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tmutexState = status;\r\n\t\t\ttry {\r\n\t\t\t\tconst res = await (status === Constants.Report.Status.Refused\r\n\t\t\t\t\t? API.Report.Reject(report.id, notes)\r\n\t\t\t\t\t: API.Report.Accept(report.id, notes));\r\n\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\ttoast.success(\r\n\t\t\t\t\t\t`Report #${report.id} has been ${status === Constants.Report.Status.Refused ? \"refused\" : \"accepted\"}`\r\n\t\t\t\t\t);\r\n\r\n\t\t\t\t\treport = {\r\n\t\t\t\t\t\t...res.body,\r\n\t\t\t\t\t\trelationships: report.relationships\r\n\t\t\t\t\t};\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t} finally {\r\n\t\t\t\tmutexState = undefined;\r\n\t\t\t}\r\n\t\t});\r\n",
    "after": "\tlet isDeleting = $state(false);\r\n\tlet mutexState = $state<Constants.Report.Status>();\r\n\tconst deleteReport = () => {\r\n\t\tif (mutex.isLocked()) return;\r\n\t\tconst intent = mutationOwner.capture(), reportId = report.id;\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tisDeleting = true;\r\n\t\t\ttry {\r\n\t\t\t\tintent.assert(); const res = await API.Report.Delete(reportId, intent); intent.assert();\r\n\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\tconst idx = entries.findIndex(e => e.id === reportId);\r\n\t\t\t\t\tif (idx >= 0) { entries.splice(idx, 1); entries = entries; }\r\n\t\t\t\t\ttoast.success(\"Report deleted successfully\");\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) throw Constants.Error.Forbidden;\r\n\t\t\t\telse if (res?.status === StatusCodes.TOO_MANY_REQUESTS) throw Constants.Error.TooManyRequests;\r\n\t\t\t\telse if (res?.status === StatusCodes.CONFLICT) throw Constants.Error.UpdateConflict;\r\n\t\t\t\telse throw Constants.Error.Unknown;\r\n\t\t\t} catch (cause) {\r\n\t\t\t\tif (!intent.alive()) return;\r\n\t\t\t\tconst error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown;\r\n\t\t\t\ttoast.error(error.title, { description: error.body });\r\n\t\t\t} finally { isDeleting = false; intent.release(); }\r\n\t\t});\r\n\t};\r\n\tconst updateStatus = (status: Constants.Report.Status, notes?: string) => {\r\n\t\tif (mutex.isLocked()) return;\r\n\t\tconst intent = mutationOwner.capture(), reportId = report.id, relationships = report.relationships;\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tmutexState = status;\r\n\t\t\ttry {\r\n\t\t\t\tintent.assert();\r\n\t\t\t\tconst res = await (status === Constants.Report.Status.Refused ? API.Report.Reject(reportId, notes, intent) : API.Report.Accept(reportId, notes, intent));\r\n\t\t\t\tintent.assert();\r\n\t\t\t\tif (res?.status === StatusCodes.OK && res.body?.id === reportId) {\r\n\t\t\t\t\ttoast.success(`Report #${reportId} has been ${status === Constants.Report.Status.Refused ? \"refused\" : \"accepted\"}`);\r\n\t\t\t\t\treport = { ...res.body, relationships };\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) throw Constants.Error.Forbidden;\r\n\t\t\t\telse if (res?.status === StatusCodes.TOO_MANY_REQUESTS) throw Constants.Error.TooManyRequests;\r\n\t\t\t\telse if (res?.status === StatusCodes.CONFLICT) throw Constants.Error.UpdateConflict;\r\n\t\t\t\telse throw Constants.Error.Unknown;\r\n\t\t\t} catch (cause) {\r\n\t\t\t\tif (!intent.alive()) return;\r\n\t\t\t\tconst error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown;\r\n\t\t\t\ttoast.error(error.title, { description: error.body });\r\n\t\t\t} finally { mutexState = undefined; intent.release(); }\r\n\t\t});\r\n\t};\r\n"
  },
  {
    "path": "src/lib/components/ReportDialog.svelte",
    "label": "round6 report dialog owns authenticated mutation context",
    "sha256": "12fc7e4392f038dc0cabd22a192208eae8c6e78db2adb06c5c1b730e8c2beca7",
    "count": 1,
    "before": "\timport { onDestroy } from \"svelte\";",
    "after": "\timport { onDestroy } from \"svelte\";\r\n\timport { createMutationOwner } from \"$platform/authenticated-mutation\";"
  },
  {
    "path": "src/lib/components/ReportDialog.svelte",
    "label": "round6 report dialog actor subject lifetime",
    "sha256": "12fc7e4392f038dc0cabd22a192208eae8c6e78db2adb06c5c1b730e8c2beca7",
    "count": 1,
    "before": "\tconst mutex = new Mutex();",
    "after": "\tconst mutex = new Mutex();\r\n\tconst reportMutationOwner = createMutationOwner(() => JSON.stringify([isOpen, entityType, entity?.id, page.url.href]));"
  },
  {
    "path": "src/lib/components/ReportDialog.svelte",
    "label": "round6 changing report subject cancels unsent intent",
    "sha256": "12fc7e4392f038dc0cabd22a192208eae8c6e78db2adb06c5c1b730e8c2beca7",
    "count": 1,
    "before": "\t\treportContext = context; const generation = ++reportGeneration;",
    "after": "\t\treportContext = context; reportMutationOwner.invalidate(); const generation = ++reportGeneration;"
  },
  {
    "path": "src/lib/components/ReportDialog.svelte",
    "label": "round6 report submission freezes actor payload and completion target",
    "sha256": "12fc7e4392f038dc0cabd22a192208eae8c6e78db2adb06c5c1b730e8c2beca7",
    "count": 1,
    "before": "\tconst onSubmit = (e?: SubmitEvent) => {\r\n\t\te?.preventDefault();\r\n\t\te?.stopPropagation();\r\n\t\te?.stopImmediatePropagation();\r\n\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tif (!entity || !entityType || !reasons?.length || !selectedReasonId || !isValid) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tconst submittedGeneration = reportGeneration;\r\n\t\t\ttry {\r\n\t\t\t\tconst res = await API.Report.Create({\r\n\t\t\t\t\tentity_type: entityType,\r\n\t\t\t\t\tentity_id: typeof entity.id === \"string\" ? entity.id : entity.id.toString(),\r\n\t\t\t\t\treason_id: selectedReasonId,\r\n\t\t\t\t\tdetails: additionalExplanation\r\n\t\t\t\t});\r\n\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\tif (submittedGeneration === reportGeneration) isOpen = false;\r\n\t\t\t\t\ttoast.success(\"Report submitted successfully\");\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t}\r\n\t\t});\r\n\t};\r\n",
    "after": "\tconst onSubmit = (e?: SubmitEvent) => {\r\n\t\te?.preventDefault(); e?.stopPropagation(); e?.stopImmediatePropagation();\r\n\t\tif (mutex.isLocked() || !isOpen || !entity || !entityType || !reasons?.length || !selectedReasonId || !isValid) return;\r\n\t\tconst intent = reportMutationOwner.capture();\r\n\t\tconst payload = { entity_type: entityType, entity_id: String(entity.id), reason_id: selectedReasonId, details: additionalExplanation };\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\ttry {\r\n\t\t\t\tintent.assert(); const res = await API.Report.Create(payload, intent); intent.assert();\r\n\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) { isOpen = false; toast.success(\"Report submitted successfully\"); }\r\n\t\t\t\telse if (res?.status === StatusCodes.FORBIDDEN) throw Constants.Error.Forbidden;\r\n\t\t\t\telse if (res?.status === StatusCodes.TOO_MANY_REQUESTS) throw Constants.Error.TooManyRequests;\r\n\t\t\t\telse if (res?.status === StatusCodes.CONFLICT) throw Constants.Error.UpdateConflict;\r\n\t\t\t\telse throw Constants.Error.Unknown;\r\n\t\t\t} catch (cause) {\r\n\t\t\t\tif (!intent.alive()) return;\r\n\t\t\t\tconst error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown;\r\n\t\t\t\ttoast.error(error.title, { description: error.body });\r\n\t\t\t} finally { intent.release(); }\r\n\t\t});\r\n\t};\r\n"
  },
  {
    "path": "src/lib/stores/Session.ts",
    "label": "round7 pure native mutation authority binding import",
    "sha256": "8baf2edac726343a1f1445c53afa7f9a5338c66fad41ffbbe6ac7285d5a25498",
    "count": 1,
    "before": "import API from \"$lib/API\";",
    "after": "import API from \"$lib/API\";\r\nimport { bindNativeMutationAuthority } from \"$platform/mutation-authority\";"
  },
  {
    "path": "src/lib/stores/Session.ts",
    "label": "round7 register synchronous actual Session authority before initialization",
    "sha256": "8baf2edac726343a1f1445c53afa7f9a5338c66fad41ffbbe6ac7285d5a25498",
    "count": 1,
    "before": "const store = writable<CheckSessionResponse | undefined>();",
    "after": "const store = writable<CheckSessionResponse | undefined>();\r\nbindNativeMutationAuthority(store);"
  },
  {
    "path": "src/lib/HTTP.ts",
    "label": "round7 native mutation transport imports pure authority registry",
    "sha256": "765a7ee6804ec81472c945e83a24b0b4343c0d58e8dbb2a2ab2421b681ba2fd1",
    "count": 1,
    "before": "import { Error as UserVisibleError } from \"$lib/constants/Error\";",
    "after": "import { Error as UserVisibleError } from \"$lib/constants/Error\";\r\nimport { PUBLIC_API_URL } from \"$env/static/public\";\r\nimport { captureNativeAuthority, isNativeAPIRequest, nativeAuthority, sameNativeAuthority } from \"$platform/mutation-authority\";"
  },
  {
    "path": "src/lib/HTTP.ts",
    "label": "round7 capture native actor and immutable payload before limiter wait",
    "sha256": "765a7ee6804ec81472c945e83a24b0b4343c0d58e8dbb2a2ab2421b681ba2fd1",
    "count": 1,
    "before": "\t\tthis.isAlreadyOpened = true;\r\n\t\treturn new Promise((resolve, reject) => {",
    "after": "\t\tthis.isAlreadyOpened = true;\r\n\t\tconst authority = this.method !== \"GET\" && this.xhr.withCredentials && isNativeAPIRequest(this.url, PUBLIC_API_URL) ? captureNativeAuthority() : undefined;\r\n\t\tif (authority) {\r\n\t\t\tif (!authority.actor) throw new HTTPError(401, \"A normal browser session is required for this native mutation. Recovery capability and anonymous state do not grant this authority.\");\r\n\t\t\tconst expected = Object.entries(this.headers).filter(([name]) => name.toLowerCase() === \"x-weebdex-expected-account\");\r\n\t\t\tif (expected.length > 1 || expected.length && expected[0][1] !== authority.actor) throw new HTTPError(409, \"The captured mutation actor no longer matches this browser authority. No operation was sent.\", \"Previous mutation context changed\");\r\n\t\t\tif (!expected.length) this.headers[\"X-WeebDex-Expected-Account\"] = authority.actor;\r\n\t\t}\r\n\t\t// Freeze JSON/FormData intent now, rather than re-reading mutable UI data when a token arrives.\r\n\t\tconst admittedPayload = this.body === undefined ? undefined : this.body instanceof FormData ? (() => { const copy = new FormData(); for (const [name, value] of this.body.entries()) copy.append(name, value); return copy; })() : JSON.stringify(this.body);\r\n\t\treturn new Promise((resolve, reject) => {"
  },
  {
    "path": "src/lib/HTTP.ts",
    "label": "round7 terminal cleanup releases native authority listener",
    "sha256": "765a7ee6804ec81472c945e83a24b0b4343c0d58e8dbb2a2ab2421b681ba2fd1",
    "count": 1,
    "before": "\t\t\tlet sentMutation = false;",
    "after": "\t\t\tlet sentMutation = false;\r\n\t\t\tlet unsubscribeAuthority: (() => void) | undefined;"
  },
  {
    "path": "src/lib/HTTP.ts",
    "label": "round7 native mutation authority observer cannot survive terminal request",
    "sha256": "765a7ee6804ec81472c945e83a24b0b4343c0d58e8dbb2a2ab2421b681ba2fd1",
    "count": 1,
    "before": "\t\t\t\twindow.clearTimeout(timer);\r\n\t\t\t\twindow.clearTimeout(this.timeout);",
    "after": "\t\t\t\twindow.clearTimeout(timer);\r\n\t\t\t\twindow.clearTimeout(this.timeout);\r\n\t\t\t\tunsubscribeAuthority?.(); unsubscribeAuthority = undefined;"
  },
  {
    "path": "src/lib/HTTP.ts",
    "label": "round7 reject stale native authority before response body attachment",
    "sha256": "765a7ee6804ec81472c945e83a24b0b4343c0d58e8dbb2a2ab2421b681ba2fd1",
    "count": 1,
    "before": "\t\t\t\tif (terminal || this.cancelled) return;\r\n\t\t\t\tif (!this.xhr.status) return onTransportFailure();",
    "after": "\t\t\t\tif (terminal || this.cancelled) return;\r\n\t\t\t\tif (authority && !sameNativeAuthority(authority)) return fail(\"Browser authority changed before the response was reconciled.\");\r\n\t\t\t\tif (!this.xhr.status) return onTransportFailure();"
  },
  {
    "path": "src/lib/HTTP.ts",
    "label": "round7 reject stale actor before consuming request budget",
    "sha256": "765a7ee6804ec81472c945e83a24b0b4343c0d58e8dbb2a2ab2421b681ba2fd1",
    "count": 1,
    "before": "\t\t\t\t// Poll only our cancellable timer. Never queue an uncancellable limiter promise.",
    "after": "\t\t\t\tif (authority && !sameNativeAuthority(authority)) return fail(\"Browser authority changed before this mutation was sent.\");\r\n\t\t\t\t// Poll only our cancellable timer. Never queue an uncancellable limiter promise."
  },
  {
    "path": "src/lib/HTTP.ts",
    "label": "round7 send only admission-snapshotted payload",
    "sha256": "765a7ee6804ec81472c945e83a24b0b4343c0d58e8dbb2a2ab2421b681ba2fd1",
    "count": 1,
    "before": "\t\t\t\t\tconst payload = this.body === undefined ? undefined : this.body instanceof FormData ? this.body : JSON.stringify(this.body);",
    "after": "\t\t\t\t\tconst payload = admittedPayload;"
  },
  {
    "path": "src/lib/HTTP.ts",
    "label": "round7 final native actor fence immediately before actual send",
    "sha256": "765a7ee6804ec81472c945e83a24b0b4343c0d58e8dbb2a2ab2421b681ba2fd1",
    "count": 1,
    "before": "\t\t\t\t\tif (terminal || this.cancelled) return;\r\n\t\t\t\t\tattempts++;",
    "after": "\t\t\t\t\tif (terminal || this.cancelled) return;\r\n\t\t\t\t\tif (authority && !sameNativeAuthority(authority)) return fail(\"Browser authority changed before this mutation was sent.\");\r\n\t\t\t\t\tattempts++;"
  },
  {
    "path": "src/lib/HTTP.ts",
    "label": "round7 native authority changes settle and cancel queued or sent mutation once",
    "sha256": "765a7ee6804ec81472c945e83a24b0b4343c0d58e8dbb2a2ab2421b681ba2fd1",
    "count": 1,
    "before": "\t\t\tif (this.cancelled) return fail(\"The request was cancelled before it was sent.\");",
    "after": "\t\t\tif (authority) unsubscribeAuthority = nativeAuthority.subscribe(() => { if (!sameNativeAuthority(authority)) fail(\"The browser account/session authority changed.\"); });\r\n\t\t\tif (this.cancelled) return fail(\"The request was cancelled before it was sent.\");"
  },
  {
    "path": "src/lib/HTTP.ts",
    "label": "round7 authority mismatch settles before authority listener registration",
    "sha256": "765a7ee6804ec81472c945e83a24b0b4343c0d58e8dbb2a2ab2421b681ba2fd1",
    "count": 1,
    "before": "\t\t\tif (authority) unsubscribeAuthority = nativeAuthority.subscribe(() => { if (!sameNativeAuthority(authority)) fail(\"The browser account/session authority changed.\"); });",
    "after": "\t\t\tif (authority && !sameNativeAuthority(authority)) return fail(\"The browser account/session authority changed before sending.\");\r\n\t\t\tif (authority) unsubscribeAuthority = nativeAuthority.subscribe(() => { if (!sameNativeAuthority(authority)) fail(\"The browser account/session authority changed.\"); });"
  },
  {
    "path": "src/lib/API/Thread.ts",
    "label": "round7 Thread transport imports explicit actor and personal read seams",
    "sha256": "aeaecc40d17e2d8111423e41a6c9c11f0665e01990b205747c7b5b91139844c4",
    "count": 1,
    "before": "import { HTTP } from \"$lib\";",
    "after": "import { HTTP } from \"$lib\";\r\nimport { authenticatedMutation, type MutationIntent } from \"$platform/authenticated-mutation\";\r\nimport { readThreadContext } from \"$platform/thread-read\";"
  },
  {
    "path": "src/lib/API/Thread.ts",
    "label": "round7 Thread GetThreadVote exact context transport",
    "sha256": "aeaecc40d17e2d8111423e41a6c9c11f0665e01990b205747c7b5b91139844c4",
    "count": 1,
    "before": "export const GetThreadVote = (threadId: string) => {\r\n\tconst http = new HTTP<{ vote: number }>(`${PUBLIC_API_URL}/thread/${threadId}/vote`);\r\n\thttp.setWithCredentials(true);\r\n\treturn http.GET();\r\n};",
    "after": "export const GetThreadVote = (threadId: string, signal?: AbortSignal) => readThreadContext<{ vote: number }>(`/thread/${encodeURIComponent(threadId)}/vote`, signal);"
  },
  {
    "path": "src/lib/API/Thread.ts",
    "label": "round7 Thread VoteThread exact context transport",
    "sha256": "aeaecc40d17e2d8111423e41a6c9c11f0665e01990b205747c7b5b91139844c4",
    "count": 1,
    "before": "export const VoteThread = (threadId: string, vote: -1 | 1) => {\r\n\tconst http = new HTTP(`${PUBLIC_API_URL}/thread/${threadId}/vote`, { vote });\r\n\thttp.setWithCredentials(true);\r\n\treturn http.POST();\r\n};",
    "after": "export const VoteThread = (threadId: string, vote: -1 | 1, intent?: MutationIntent) => authenticatedMutation<void>(`/thread/${encodeURIComponent(threadId)}/vote`, \"POST\", { vote }, intent);"
  },
  {
    "path": "src/lib/API/Thread.ts",
    "label": "round7 Thread UnvoteThread exact context transport",
    "sha256": "aeaecc40d17e2d8111423e41a6c9c11f0665e01990b205747c7b5b91139844c4",
    "count": 1,
    "before": "export const UnvoteThread = (threadId: string) => {\r\n\tconst http = new HTTP(`${PUBLIC_API_URL}/thread/${threadId}/vote`);\r\n\thttp.setWithCredentials(true);\r\n\treturn http.DELETE();\r\n};",
    "after": "export const UnvoteThread = (threadId: string, intent?: MutationIntent) => authenticatedMutation<void>(`/thread/${encodeURIComponent(threadId)}/vote`, \"DELETE\", undefined, intent);"
  },
  {
    "path": "src/lib/API/Thread.ts",
    "label": "round7 Thread Replies exact context transport",
    "sha256": "aeaecc40d17e2d8111423e41a6c9c11f0665e01990b205747c7b5b91139844c4",
    "count": 1,
    "before": "export const Replies = (threadId: string, searchParams: URLSearchParams) => {\r\n\tconst http = new HTTP<ReplyFeedResponse>(`${PUBLIC_API_URL}/thread/${threadId}/replies${searchParams.toString()}`);\r\n\thttp.setWithCredentials(true);\r\n\treturn http.GET();\r\n};",
    "after": "export const Replies = (threadId: string, q: URLSearchParams) => { const query = [...q.entries()].map(([k, v]) => encodeURIComponent(k) + \"=\" + encodeURIComponent(v)).join(\"&\"); return readThreadContext<ReplyFeedResponse>(`/thread/${encodeURIComponent(threadId)}/replies` + (query ? \"?\" + query : \"\")); };"
  },
  {
    "path": "src/lib/API/Thread.ts",
    "label": "round7 Thread GetReply exact context transport",
    "sha256": "aeaecc40d17e2d8111423e41a6c9c11f0665e01990b205747c7b5b91139844c4",
    "count": 1,
    "before": "export const GetReply = (threadId: string, replyId: string) => {\r\n\tconst http = new HTTP<Reply>(`${PUBLIC_API_URL}/thread/${threadId}/replies/${replyId}`);\r\n\thttp.setWithCredentials(true);\r\n\treturn http.GET();\r\n};",
    "after": "export const GetReply = (threadId: string, replyId: string) => readThreadContext<Reply>(`/thread/${encodeURIComponent(threadId)}/replies/${encodeURIComponent(replyId)}`);"
  },
  {
    "path": "src/lib/API/Thread.ts",
    "label": "round7 Thread CreateReply exact context transport",
    "sha256": "aeaecc40d17e2d8111423e41a6c9c11f0665e01990b205747c7b5b91139844c4",
    "count": 1,
    "before": "export const CreateReply = (threadId: string, payload: CreateReplyRequest) => {\r\n\tconst http = new HTTP<Reply>(`${PUBLIC_API_URL}/thread/${threadId}/replies`, payload);\r\n\thttp.setWithCredentials(true);\r\n\treturn http.POST();\r\n};",
    "after": "export const CreateReply = (threadId: string, payload: CreateReplyRequest, intent?: MutationIntent) => authenticatedMutation<Reply>(`/thread/${encodeURIComponent(threadId)}/replies`, \"POST\", payload, intent);"
  },
  {
    "path": "src/lib/API/Thread.ts",
    "label": "round7 Thread UpdateReply exact context transport",
    "sha256": "aeaecc40d17e2d8111423e41a6c9c11f0665e01990b205747c7b5b91139844c4",
    "count": 1,
    "before": "export const UpdateReply = (threadId: string, replyId: number, payload: { body: string; version: number }) => {\r\n\tconst http = new HTTP<Reply>(`${PUBLIC_API_URL}/thread/${threadId}/replies/${replyId}`, payload);\r\n\thttp.setWithCredentials(true);\r\n\treturn http.PUT();\r\n};",
    "after": "export const UpdateReply = (threadId: string, replyId: number, payload: { body: string; version: number }, intent?: MutationIntent) => authenticatedMutation<Reply>(`/thread/${encodeURIComponent(threadId)}/replies/${replyId}`, \"PUT\", payload, intent);"
  },
  {
    "path": "src/lib/API/Thread.ts",
    "label": "round7 Thread RemoveReply exact context transport",
    "sha256": "aeaecc40d17e2d8111423e41a6c9c11f0665e01990b205747c7b5b91139844c4",
    "count": 1,
    "before": "export const RemoveReply = (threadId: string, replyId: number) => {\r\n\tconst http = new HTTP(`${PUBLIC_API_URL}/thread/${threadId}/replies/${replyId}`);\r\n\thttp.setWithCredentials(true);\r\n\treturn http.DELETE();\r\n};",
    "after": "export const RemoveReply = (threadId: string, replyId: number, intent?: MutationIntent) => authenticatedMutation<void>(`/thread/${encodeURIComponent(threadId)}/replies/${replyId}`, \"DELETE\", undefined, intent);"
  },
  {
    "path": "src/lib/API/Thread.ts",
    "label": "round7 Thread DeleteReply exact context transport",
    "sha256": "aeaecc40d17e2d8111423e41a6c9c11f0665e01990b205747c7b5b91139844c4",
    "count": 1,
    "before": "export const DeleteReply = (threadId: string, replyId: number) => {\r\n\tconst http = new HTTP(`${PUBLIC_API_URL}/thread/${threadId}/replies/${replyId}/hard`);\r\n\thttp.setWithCredentials(true);\r\n\treturn http.DELETE();\r\n};",
    "after": "export const DeleteReply = (threadId: string, replyId: number, intent?: MutationIntent) => authenticatedMutation<void>(`/thread/${encodeURIComponent(threadId)}/replies/${replyId}/hard`, \"DELETE\", undefined, intent);"
  },
  {
    "path": "src/lib/API/Thread.ts",
    "label": "round7 Thread VoteReply exact context transport",
    "sha256": "aeaecc40d17e2d8111423e41a6c9c11f0665e01990b205747c7b5b91139844c4",
    "count": 1,
    "before": "export const VoteReply = (threadId: string, replyId: number, vote: -1 | 1) => {\r\n\tconst http = new HTTP(`${PUBLIC_API_URL}/thread/${threadId}/replies/${replyId}/vote`, { vote });\r\n\thttp.setWithCredentials(true);\r\n\treturn http.POST();\r\n};",
    "after": "export const VoteReply = (threadId: string, replyId: number, vote: -1 | 1, intent?: MutationIntent) => authenticatedMutation<void>(`/thread/${encodeURIComponent(threadId)}/replies/${replyId}/vote`, \"POST\", { vote }, intent);"
  },
  {
    "path": "src/lib/API/Thread.ts",
    "label": "round7 Thread UnvoteReply exact context transport",
    "sha256": "aeaecc40d17e2d8111423e41a6c9c11f0665e01990b205747c7b5b91139844c4",
    "count": 1,
    "before": "export const UnvoteReply = (threadId: string, replyId: number) => {\r\n\tconst http = new HTTP(`${PUBLIC_API_URL}/thread/${threadId}/replies/${replyId}/vote`);\r\n\thttp.setWithCredentials(true);\r\n\treturn http.DELETE();\r\n};",
    "after": "export const UnvoteReply = (threadId: string, replyId: number, intent?: MutationIntent) => authenticatedMutation<void>(`/thread/${encodeURIComponent(threadId)}/replies/${replyId}/vote`, \"DELETE\", undefined, intent);"
  },
  {
    "path": "src/lib/components/comments/context.svelte.ts",
    "label": "round7 discussion context reads actual current thread prop",
    "sha256": "4a89adcfd64a02938626298baf56ef7230f3323fcc8124ee38367f29a082d452",
    "count": 1,
    "before": "\tthread: Thread;",
    "after": "\tthread: () => Thread;"
  },
  {
    "path": "src/lib/components/comments/context.svelte.ts",
    "label": "round7 discussion context getter cannot retain previous prop snapshot",
    "sha256": "4a89adcfd64a02938626298baf56ef7230f3323fcc8124ee38367f29a082d452",
    "count": 1,
    "before": "\t\treturn this.props.thread;",
    "after": "\t\treturn this.props.thread();"
  },
  {
    "path": "src/lib/components/comments/comments-provider.svelte",
    "label": "round7 discussion provider observes actual browser authority",
    "sha256": "741c156112cb90a69b090ad8e3a4f2c1ec7f5ba47d43331ad7f8caabef9e6b41",
    "count": 1,
    "before": "\timport { onMount } from \"svelte\";",
    "after": "\timport { onMount } from \"svelte\";\r\n\timport { nativeAuthority } from \"$platform/mutation-authority\";"
  },
  {
    "path": "src/lib/components/comments/comments-provider.svelte",
    "label": "round7 discussion provider exposes current thread without snapshot",
    "sha256": "741c156112cb90a69b090ad8e3a4f2c1ec7f5ba47d43331ad7f8caabef9e6b41",
    "count": 1,
    "before": "\tsetComments({",
    "after": "\tconst comments = setComments({"
  },
  {
    "path": "src/lib/components/comments/comments-provider.svelte",
    "label": "round7 current discussion prop retained as getter",
    "sha256": "741c156112cb90a69b090ad8e3a4f2c1ec7f5ba47d43331ad7f8caabef9e6b41",
    "count": 1,
    "before": "\t\tthread,\r\n\t\tstats:",
    "after": "\t\tthread: () => thread,\r\n\t\tstats:"
  },
  {
    "path": "src/lib/components/comments/comments-provider.svelte",
    "label": "round7 discussion target and authority reset private transient state",
    "sha256": "741c156112cb90a69b090ad8e3a4f2c1ec7f5ba47d43331ad7f8caabef9e6b41",
    "count": 1,
    "before": "\tonMount(() => {",
    "after": "\tlet discussionContext = \"\";\r\n\t$effect(() => {\r\n\t\tconst context = JSON.stringify([thread.id, $nativeAuthority.actor, $nativeAuthority.epoch]);\r\n\t\tif (context === discussionContext) return;\r\n\t\tdiscussionContext = context; stats = { ...thread.relationships.stats }; voteState = undefined; feed = undefined;\r\n\t\tcomments.body = \"\"; comments.isEdit = false; comments.editTarget = undefined;\r\n\t});\r\n\tonMount(() => {"
  },
  {
    "path": "src/lib/components/comments/comments-provider.svelte",
    "label": "approved round7 discussion child lifetime keyed by actual target and authority",
    "sha256": "741c156112cb90a69b090ad8e3a4f2c1ec7f5ba47d43331ad7f8caabef9e6b41",
    "count": 1,
    "before": "{@render children()}",
    "after": "{#key JSON.stringify([thread.id, $nativeAuthority.actor, $nativeAuthority.epoch])}\r\n\t{@render children()}\r\n{/key}",
    "approved_discussion_context_exception": "discussion_target_authority_key"
  },
  {
    "path": "src/lib/components/comments/comments-feed.svelte",
    "label": "round7 discussion feed owns read completion lifetime",
    "sha256": "d698d665298f64fb076715779fe998a38556c67f63564a635cc2a05ce08831cd",
    "count": 1,
    "before": "\timport { onMount } from \"svelte\";",
    "after": "\timport { onDestroy, onMount } from \"svelte\";\r\n\timport { nativeAuthority } from \"$platform/mutation-authority\";"
  },
  {
    "path": "src/lib/components/comments/comments-feed.svelte",
    "label": "round7 discussion feed full target query authority generation without private history cache",
    "sha256": "d698d665298f64fb076715779fe998a38556c67f63564a635cc2a05ce08831cd",
    "count": 1,
    "before": "\tonMount(async () => {\r\n\t\tif ((page.state as any)._$Replies_feed) {\r\n\t\t\tcomments.feed = (page.state as any)._$Replies_feed as typeof comments.feed;\r\n\t\t} else {\r\n\t\t\ttry {\r\n\t\t\t\tconst q = new URLSearchParams();\r\n\t\t\t\tif (page.url.searchParams.has(\"page\")) {\r\n\t\t\t\t\tconst p = page.url.searchParams.get(\"page\");\r\n\t\t\t\t\tif (Number(p) > 1) q.set(\"page\", p!);\r\n\t\t\t\t}\r\n\t\t\t\tif (page.url.searchParams.has(\"sort\")) {\r\n\t\t\t\t\tconst sort = page.url.searchParams.get(\"sort\");\r\n\t\t\t\t\tif (sort === \"oldest\") q.set(\"order\", \"asc\");\r\n\t\t\t\t}\r\n\r\n\t\t\t\tconst res = await API.Thread.Replies(comments.thread.id, q);\r\n\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\tres.body.data ??= [];\r\n\t\t\t\t\tcomments.feed = res.body;\r\n\t\t\t\t\treplaceState(window.location.href, {\r\n\t\t\t\t\t\t...page.state,\r\n\t\t\t\t\t\t_$Replies_feed: $state.snapshot(comments.feed)\r\n\t\t\t\t\t});\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) error = e;\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\terror = Constants.Error.Unknown;\r\n\t\t\t\t}\r\n\t\t\t}\r\n\t\t}\r\n\t\tisMounted = true;\r\n\t});\r\n",
    "after": "\tlet feedGeneration = 0, feedDisposed = false;\r\n\tonDestroy(() => { feedDisposed = true; feedGeneration++; });\r\n\t$effect(() => {\r\n\t\tconst threadId = comments.thread.id, href = page.url.href, authority = $nativeAuthority;\r\n\t\tconst generation = ++feedGeneration;\r\n\t\tcomments.feed = undefined; error = undefined; isMounted = false;\r\n\t\tconst alive = () => !feedDisposed && generation === feedGeneration && comments.thread.id === threadId && page.url.href === href && $nativeAuthority === authority;\r\n\t\tconst q = new URLSearchParams();\r\n\t\tconst targetPage = page.url.searchParams.get(\"page\"); if (Number(targetPage) > 1) q.set(\"page\", targetPage!);\r\n\t\tif (page.url.searchParams.get(\"sort\") === \"oldest\") q.set(\"order\", \"asc\");\r\n\t\tvoid API.Thread.Replies(threadId, q).then(res => {\r\n\t\t\tif (!alive()) return;\r\n\t\t\tif (res?.status === StatusCodes.OK && res.body) { res.body.data ??= []; comments.feed = res.body; }\r\n\t\t\telse throw res?.status === StatusCodes.TOO_MANY_REQUESTS ? Constants.Error.TooManyRequests : Constants.Error.Unknown;\r\n\t\t}).catch(cause => { if (alive()) error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; }).finally(() => { if (alive()) isMounted = true; });\r\n\t});\r\n"
  },
  {
    "path": "src/lib/components/comments/comments.svelte",
    "label": "round7 discussion statistics owns lifetime",
    "sha256": "09c5e198006e3e6f6b12821edb467e8446b29de8fe0c4e2a0627846335a8648f",
    "count": 1,
    "before": "\timport { onMount, tick } from \"svelte\";",
    "after": "\timport { onDestroy, onMount, tick } from \"svelte\";"
  },
  {
    "path": "src/lib/components/comments/comments.svelte",
    "label": "round7 fresh statistics cannot attach to previous discussion",
    "sha256": "09c5e198006e3e6f6b12821edb467e8446b29de8fe0c4e2a0627846335a8648f",
    "count": 1,
    "before": "\tonMount(async () => {\r\n\t\tif (lastThreadId === comments.thread.id) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\ttry {\r\n\t\t\tconst res = await API.Statistics.Thread(comments.thread.id);\r\n\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\tcomments.stats = res.body;\r\n\t\t\t\tlastThreadId = comments.thread.id;\r\n\t\t\t}\r\n\t\t} catch (e) {\r\n\t\t\tconsole.error(e);\r\n\t\t}\r\n\t});\r\n",
    "after": "\tlet disposed = false;\r\n\tonDestroy(() => { disposed = true; });\r\n\tonMount(async () => {\r\n\t\tconst threadId = comments.thread.id;\r\n\t\ttry { const res = await API.Statistics.Thread(threadId); if (!disposed && comments.thread.id === threadId && res?.status === StatusCodes.OK && res.body) comments.stats = res.body; }\r\n\t\tcatch (cause) { if (!disposed) console.error(cause); }\r\n\t});\r\n"
  },
  {
    "path": "src/lib/components/comments/comments-create.svelte",
    "label": "round7 component owns discussion mutation authority",
    "sha256": "df005e448d2fddc2741024d421d31c20c89160ff05fd8dd70979ff93b8d28db1",
    "count": 1,
    "before": "\timport { toast } from \"svelte-sonner\";",
    "after": "\timport { toast } from \"svelte-sonner\";\r\n\timport { createMutationOwner } from \"$platform/authenticated-mutation\";"
  },
  {
    "path": "src/lib/components/comments/comments-create.svelte",
    "label": "round7 discussion target change aborts unsent component intents",
    "sha256": "df005e448d2fddc2741024d421d31c20c89160ff05fd8dd70979ff93b8d28db1",
    "count": 1,
    "before": "\tconst comments = useComments();",
    "after": "\tconst comments = useComments();\r\n\tconst mutationOwner = createMutationOwner(() => JSON.stringify([comments.thread.id, page.url.href]));\r\n\t$effect(() => { JSON.stringify([comments.thread.id, page.url.href]); mutationOwner.invalidate(); });"
  },
  {
    "path": "src/lib/components/comments/comments-edit-dialog.svelte",
    "label": "round7 component owns discussion mutation authority",
    "sha256": "a2fdd2c5d036caded65638461b17fa95c3d8b1da55b4b0ef7dedf902e38972ac",
    "count": 1,
    "before": "\timport { toast } from \"svelte-sonner\";",
    "after": "\timport { toast } from \"svelte-sonner\";\r\n\timport { createMutationOwner } from \"$platform/authenticated-mutation\";"
  },
  {
    "path": "src/lib/components/comments/comments-edit-dialog.svelte",
    "label": "round7 edit actor is a real browser Session",
    "sha256": "a2fdd2c5d036caded65638461b17fa95c3d8b1da55b4b0ef7dedf902e38972ac",
    "count": 1,
    "before": "\timport { useComments } from \"./context.svelte\";",
    "after": "\timport { useComments } from \"./context.svelte\";\r\n\timport { Session } from \"$lib/stores\";"
  },
  {
    "path": "src/lib/components/comments/comments-edit-dialog.svelte",
    "label": "round7 discussion target change aborts unsent component intents",
    "sha256": "a2fdd2c5d036caded65638461b17fa95c3d8b1da55b4b0ef7dedf902e38972ac",
    "count": 1,
    "before": "\tconst comments = useComments();",
    "after": "\tconst comments = useComments();\r\n\tconst mutationOwner = createMutationOwner(() => JSON.stringify([comments.thread.id, comments.isEdit, comments.editTarget?.id, page.url.href]));\r\n\t$effect(() => { JSON.stringify([comments.thread.id, comments.isEdit, comments.editTarget?.id, page.url.href]); mutationOwner.invalidate(); });"
  },
  {
    "path": "src/lib/components/comments/comments-entry.svelte",
    "label": "round7 component owns discussion mutation authority",
    "sha256": "1bc293dfc50ca3456ffa25ed37348302e538f96876d0e1b88c58b61443b05b36",
    "count": 1,
    "before": "\timport { toast } from \"svelte-sonner\";",
    "after": "\timport { toast } from \"svelte-sonner\";\r\n\timport { createMutationOwner } from \"$platform/authenticated-mutation\";"
  },
  {
    "path": "src/lib/components/comments/comments-entry.svelte",
    "label": "round7 discussion target change aborts unsent component intents",
    "sha256": "1bc293dfc50ca3456ffa25ed37348302e538f96876d0e1b88c58b61443b05b36",
    "count": 1,
    "before": "\tconst comments = useComments();",
    "after": "\tconst comments = useComments();\r\n\tconst mutationOwner = createMutationOwner(() => JSON.stringify([comments.thread.id, reply.id, page.url.href]));\r\n\t$effect(() => { JSON.stringify([comments.thread.id, reply.id, page.url.href]); mutationOwner.invalidate(); });"
  },
  {
    "path": "src/lib/components/comments/comments-votes.svelte",
    "label": "round7 component owns discussion mutation authority",
    "sha256": "5d7b67c964c3d38c42b7fc6c064be82dfef0c351258a5ff6f576fd702a88a14c",
    "count": 1,
    "before": "\timport { toast } from \"svelte-sonner\";",
    "after": "\timport { toast } from \"svelte-sonner\";\r\n\timport { createMutationOwner } from \"$platform/authenticated-mutation\";\r\n\timport { nativeAuthority, captureNativeAuthority, sameNativeReadAuthority } from \"$platform/mutation-authority\";"
  },
  {
    "path": "src/lib/components/comments/comments-votes.svelte",
    "label": "round7 thread vote target includes navigation context",
    "sha256": "5d7b67c964c3d38c42b7fc6c064be82dfef0c351258a5ff6f576fd702a88a14c",
    "count": 1,
    "before": "\timport { API, Constants, Mutex } from \"$lib\";",
    "after": "\timport { API, Constants, Mutex } from \"$lib\";\r\n\timport { page } from \"$app/state\";"
  },
  {
    "path": "src/lib/components/comments/comments-votes.svelte",
    "label": "round7 discussion target change aborts unsent component intents",
    "sha256": "5d7b67c964c3d38c42b7fc6c064be82dfef0c351258a5ff6f576fd702a88a14c",
    "count": 1,
    "before": "\tconst comments = useComments();",
    "after": "\tconst comments = useComments();\r\n\tconst mutationOwner = createMutationOwner(() => JSON.stringify([comments.thread.id, page.url.href]));\r\n\t$effect(() => { JSON.stringify([comments.thread.id, page.url.href]); mutationOwner.invalidate(); });"
  },
  {
    "path": "src/lib/components/comments/comments-create.svelte",
    "label": "round7 CreateReply admission and completion bind target actor payload",
    "sha256": "df005e448d2fddc2741024d421d31c20c89160ff05fd8dd70979ff93b8d28db1",
    "count": 1,
    "before": "\tconst onSubmit = (ev: SubmitEvent) => {\r\n\t\tev.preventDefault();\r\n\t\tev.stopPropagation();\r\n\t\tev.stopImmediatePropagation();\r\n\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tif (!comments.body.trim().length) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\ttry {\r\n\t\t\t\tconst res = await API.Thread.CreateReply(comments.thread.id, { body: comments.body });\r\n\t\t\t\tif (res?.status === StatusCodes.CREATED && res.body) {\r\n\t\t\t\t\ttoast.success(\"Comment posted successfully\");\r\n\t\t\t\t\treplaceState(window.location.href, {\r\n\t\t\t\t\t\t...page.state,\r\n\t\t\t\t\t\t_$Replies_feed: undefined\r\n\t\t\t\t\t});\r\n\r\n\t\t\t\t\tcomments.body = \"\";\r\n\t\t\t\t\tkey.refresh();\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t});\r\n\t};\r\n\r\n",
    "after": "\tconst onSubmit = (ev: SubmitEvent) => {\r\n\t\tev.preventDefault(); ev.stopPropagation(); ev.stopImmediatePropagation();\r\n\t\tif (mutex.isLocked() || !$Session || !comments.body.trim().length) return;\r\n\t\tconst intent = mutationOwner.capture(); const threadId = comments.thread.id, payload = { body: comments.body };\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\ttry {\r\n\t\t\t\tintent.assert(); const res = await API.Thread.CreateReply(threadId, payload, intent); intent.assert();\r\n\t\t\t\tif (res?.status === StatusCodes.CREATED && res.body) { comments.body = \"\"; toast.success(\"Comment posted successfully\"); key.refresh(); }\r\n\t\t\t\telse if (res?.status === StatusCodes.FORBIDDEN) throw Constants.Error.Forbidden;\r\n\t\t\t\telse if (res?.status === StatusCodes.CONFLICT) throw Constants.Error.UpdateConflict;\r\n\t\t\t\telse if (res?.status === StatusCodes.TOO_MANY_REQUESTS) throw Constants.Error.TooManyRequests;\r\n\t\t\t\telse throw Constants.Error.Unknown;\r\n\t\t\t} catch (cause) { if (!intent.alive()) return; const error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; toast.error(error.title, { description: error.body }); }\r\n\t\t\tfinally { intent.release(); }\r\n\t\t});\r\n\t};\r\n\r\n"
  },
  {
    "path": "src/lib/components/comments/comments-edit-dialog.svelte",
    "label": "round7 UpdateReply admission and completion bind target actor payload",
    "sha256": "a2fdd2c5d036caded65638461b17fa95c3d8b1da55b4b0ef7dedf902e38972ac",
    "count": 1,
    "before": "\tconst onSubmit = (ev: SubmitEvent) => {\r\n\t\tev.preventDefault();\r\n\t\tev.stopPropagation();\r\n\t\tev.stopImmediatePropagation();\r\n\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tif (!comments.editTarget || !body) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\ttry {\r\n\t\t\t\tconst res = await API.Thread.UpdateReply(comments.thread.id, comments.editTarget.id, {\r\n\t\t\t\t\tbody,\r\n\t\t\t\t\tversion: comments.editTarget.version\r\n\t\t\t\t});\r\n\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\ttoast.success(\"Comment updated successfully\");\r\n\t\t\t\t\treplaceState(window.location.href, {\r\n\t\t\t\t\t\t...page.state,\r\n\t\t\t\t\t\t_$Replies_feed: undefined\r\n\t\t\t\t\t});\r\n\r\n\t\t\t\t\tcomments.isEdit = false;\r\n\t\t\t\t\tkey.refresh();\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t});\r\n\t};\r\n\r\n",
    "after": "\tconst onSubmit = (ev: SubmitEvent) => {\r\n\t\tev.preventDefault(); ev.stopPropagation(); ev.stopImmediatePropagation();\r\n\t\tif (mutex.isLocked() || !$Session || !comments.isEdit || !comments.editTarget || !body) return;\r\n\t\tconst intent = mutationOwner.capture(); const threadId = comments.thread.id, replyId = comments.editTarget!.id, payload = { body, version: comments.editTarget!.version };\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\ttry {\r\n\t\t\t\tintent.assert(); const res = await API.Thread.UpdateReply(threadId, replyId, payload, intent); intent.assert();\r\n\t\t\t\tif (res?.status === StatusCodes.OK && res.body) { comments.isEdit = false; toast.success(\"Comment updated successfully\"); key.refresh(); }\r\n\t\t\t\telse if (res?.status === StatusCodes.FORBIDDEN) throw Constants.Error.Forbidden;\r\n\t\t\t\telse if (res?.status === StatusCodes.CONFLICT) throw Constants.Error.UpdateConflict;\r\n\t\t\t\telse if (res?.status === StatusCodes.TOO_MANY_REQUESTS) throw Constants.Error.TooManyRequests;\r\n\t\t\t\telse throw Constants.Error.Unknown;\r\n\t\t\t} catch (cause) { if (!intent.alive()) return; const error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; toast.error(error.title, { description: error.body }); }\r\n\t\t\tfinally { intent.release(); }\r\n\t\t});\r\n\t};\r\n\r\n"
  },
  {
    "path": "src/lib/components/comments/comments-entry.svelte",
    "label": "round7 reply vote delete and rollback data stay in admitted context",
    "sha256": "1bc293dfc50ca3456ffa25ed37348302e538f96876d0e1b88c58b61443b05b36",
    "count": 1,
    "before": "\tlet mutexVoteState = $state<-1 | 1>();\r\n\tconst onVote = (value: -1 | 1) => {\r\n\t\tif (reply.relationships.vote_state === value) {\r\n\t\t\treturn onDeleteVote(value);\r\n\t\t}\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tmutexVoteState = value;\r\n\t\t\ttry {\r\n\t\t\t\tconst res = await API.Thread.VoteReply(comments.thread.id, reply.id, value);\r\n\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\tif (reply.relationships.vote_state === -1) {\r\n\t\t\t\t\t\treply.relationships.stats.down--;\r\n\t\t\t\t\t} else if (reply.relationships.vote_state === 1) {\r\n\t\t\t\t\t\treply.relationships.stats.up--;\r\n\t\t\t\t\t}\r\n\t\t\t\t\treply.relationships.stats[value === 1 ? \"up\" : \"down\"]++;\r\n\t\t\t\t\treply.relationships.vote_state = value;\r\n\t\t\t\t\treply = reply;\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t\t} else if (res?.status === StatusCodes.CONFLICT) {\r\n\t\t\t\t\tthrow Constants.Error.UpdateConflict;\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t} finally {\r\n\t\t\t\tmutexVoteState = undefined;\r\n\t\t\t}\r\n\t\t});\r\n\t};\r\n\r\n\tconst onDeleteVote = (value: -1 | 1) =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tmutexVoteState = value;\r\n\t\t\ttry {\r\n\t\t\t\tconst res = await API.Thread.UnvoteReply(comments.thread.id, reply.id);\r\n\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\tif (reply.relationships.vote_state === 1) {\r\n\t\t\t\t\t\treply.relationships.stats.up--;\r\n\t\t\t\t\t} else if (reply.relationships.vote_state === -1) {\r\n\t\t\t\t\t\treply.relationships.stats.down--;\r\n\t\t\t\t\t}\r\n\t\t\t\t\treply.relationships.vote_state = undefined;\r\n\t\t\t\t\treply = reply;\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t\t} else if (res?.status === StatusCodes.CONFLICT) {\r\n\t\t\t\t\tthrow Constants.Error.UpdateConflict;\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t} finally {\r\n\t\t\t\tmutexVoteState = undefined;\r\n\t\t\t}\r\n\t\t});\r\n\r\n\tconst onRemove = () =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tif (!$Session) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\ttry {\r\n\t\t\t\tconst res = await API.Thread.RemoveReply(comments.thread.id, reply.id);\r\n\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\ttoast.success(\"Comment deleted successfully\");\r\n\t\t\t\t\treplaceState(window.location.href, {\r\n\t\t\t\t\t\t...page.state,\r\n\t\t\t\t\t\t_$Replies_feed: undefined\r\n\t\t\t\t\t});\r\n\r\n\t\t\t\t\tkey.refresh();\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t}\r\n\t\t});\r\n\r\n\tconst onDelete = () =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tif (!$Session) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\ttry {\r\n\t\t\t\tconst res = await API.Thread.DeleteReply(comments.thread.id, reply.id);\r\n\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\ttoast.success(\"Comment deleted successfully\");\r\n\t\t\t\t\treplaceState(window.location.href, {\r\n\t\t\t\t\t\t...page.state,\r\n\t\t\t\t\t\t_$Replies_feed: undefined\r\n\t\t\t\t\t});\r\n\r\n\t\t\t\t\tkey.refresh();\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t}\r\n\t\t});\r\n",
    "after": "\tlet mutexVoteState = $state<-1 | 1>();\r\n\tconst mutateReply = (operation: \"vote\" | \"unvote\" | \"remove\" | \"delete\", value?: -1 | 1) => {\r\n\t\tif (mutex.isLocked() || !$Session) return;\r\n\t\tconst intent = mutationOwner.capture(), threadId = comments.thread.id, replyId = reply.id, previousVote = reply.relationships.vote_state;\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tmutexVoteState = value;\r\n\t\t\ttry {\r\n\t\t\t\tintent.assert();\r\n\t\t\t\tconst res = await (operation === \"vote\" ? API.Thread.VoteReply(threadId, replyId, value!, intent) : operation === \"unvote\" ? API.Thread.UnvoteReply(threadId, replyId, intent) : operation === \"remove\" ? API.Thread.RemoveReply(threadId, replyId, intent) : API.Thread.DeleteReply(threadId, replyId, intent));\r\n\t\t\t\tintent.assert();\r\n\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\tif (operation === \"vote\" || operation === \"unvote\") {\r\n\t\t\t\t\t\tconst next = operation === \"vote\" ? value : undefined;\r\n\t\t\t\t\t\tconst stats = { ...reply.relationships.stats };\r\n\t\t\t\t\t\tif (previousVote === 1) stats.up--; else if (previousVote === -1) stats.down--;\r\n\t\t\t\t\t\tif (next === 1) stats.up++; else if (next === -1) stats.down++;\r\n\t\t\t\t\t\treply = { ...reply, relationships: { ...reply.relationships, stats, vote_state: next } };\r\n\t\t\t\t\t} else { toast.success(\"Comment deleted successfully\"); key.refresh(); }\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) throw Constants.Error.Forbidden;\r\n\t\t\t\telse if (res?.status === StatusCodes.CONFLICT) throw Constants.Error.UpdateConflict;\r\n\t\t\t\telse if (res?.status === StatusCodes.TOO_MANY_REQUESTS) throw Constants.Error.TooManyRequests;\r\n\t\t\t\telse throw Constants.Error.Unknown;\r\n\t\t\t} catch (cause) { if (!intent.alive()) return; const error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; toast.error(error.title, { description: error.body }); }\r\n\t\t\tfinally { mutexVoteState = undefined; intent.release(); }\r\n\t\t});\r\n\t};\r\n\tconst onVote = (value: -1 | 1) => mutateReply(reply.relationships.vote_state === value ? \"unvote\" : \"vote\", value);\r\n\tconst onDeleteVote = (value: -1 | 1) => mutateReply(\"unvote\", value);\r\n\tconst onRemove = () => mutateReply(\"remove\");\r\n\tconst onDelete = () => mutateReply(\"delete\");\r\n"
  },
  {
    "path": "src/lib/components/comments/comments-votes.svelte",
    "label": "round9 stable personal vote read and acknowledged authoritative counts",
    "sha256": "5d7b67c964c3d38c42b7fc6c064be82dfef0c351258a5ff6f576fd702a88a14c",
    "count": 1,
    "before": "\tonMount(() =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tif ((await Session.isValid()) && comments.voteState === undefined) {\r\n\t\t\t\ttry {\r\n\t\t\t\t\tconst res = await API.Thread.GetThreadVote(comments.thread.id);\r\n\t\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\t\tcomments.voteState = res.body.vote;\r\n\t\t\t\t\t\tif (comments.stats) {\r\n\t\t\t\t\t\t\tif (comments.stats.up === 0 && comments.voteState === 1) {\r\n\t\t\t\t\t\t\t\tcomments.stats.up++;\r\n\t\t\t\t\t\t\t} else if (comments.stats.down === 0 && comments.voteState === -1) {\r\n\t\t\t\t\t\t\t\tcomments.stats.down++;\r\n\t\t\t\t\t\t\t}\r\n\t\t\t\t\t\t}\r\n\t\t\t\t\t} else if (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\t\tcomments.voteState = 0;\r\n\t\t\t\t\t}\r\n\t\t\t\t} catch (e) {}\r\n\t\t\t}\r\n\t\t})\r\n\t);\r\n\r\n\tlet mutexVoteState = $state<-1 | 1>();\r\n\tconst onVote = (value: -1 | 1) => {\r\n\t\tif (comments.voteState === value) {\r\n\t\t\treturn onDeleteVote(value);\r\n\t\t}\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tif (!comments.thread || !comments.stats) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tif (!(await Session.isValid())) {\r\n\t\t\t\ttoast.error(\"Please sign in to perform this action\");\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tmutexVoteState = value;\r\n\t\t\ttry {\r\n\t\t\t\tconst res = await API.Thread.VoteThread(comments.thread.id, value);\r\n\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\tif (comments.voteState === -1) {\r\n\t\t\t\t\t\tcomments.stats.down--;\r\n\t\t\t\t\t} else if (comments.voteState === 1) {\r\n\t\t\t\t\t\tcomments.stats.up--;\r\n\t\t\t\t\t}\r\n\t\t\t\t\tcomments.stats[value === 1 ? \"up\" : \"down\"]++;\r\n\t\t\t\t\tcomments.voteState = value;\r\n\t\t\t\t\tcomments.stats = comments.stats;\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t\t} else if (res?.status === StatusCodes.CONFLICT) {\r\n\t\t\t\t\tthrow Constants.Error.UpdateConflict;\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t} finally {\r\n\t\t\t\tmutexVoteState = undefined;\r\n\t\t\t}\r\n\t\t});\r\n\t};\r\n\r\n\tconst onDeleteVote = (value: -1 | 1) =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tif (!comments.stats) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tmutexVoteState = value;\r\n\t\t\ttry {\r\n\t\t\t\tconst res = await API.Thread.UnvoteThread(comments.thread.id);\r\n\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\tcomments.stats[value === 1 ? \"up\" : \"down\"]--;\r\n\t\t\t\t\tcomments.voteState = undefined;\r\n\t\t\t\t\tcomments.stats = comments.stats;\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t\t} else if (res?.status === StatusCodes.CONFLICT) {\r\n\t\t\t\t\tthrow Constants.Error.UpdateConflict;\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t} finally {\r\n\t\t\t\tmutexVoteState = undefined;\r\n\t\t\t}\r\n\t\t});\r\n",
    "after": "  let voteReadDisposed = false, voteReadGeneration = 0, voteReadController: AbortController | undefined;\r\n  onDestroy(() => { voteReadDisposed = true; voteReadGeneration++; voteReadController?.abort(); });\r\n  $effect(() => {\r\n    const threadId = comments.thread.id, actor = $nativeAuthority.actor, epoch = $nativeAuthority.epoch;\r\n    const generation = ++voteReadGeneration, authority = captureNativeAuthority();\r\n    voteReadController?.abort(); const controller = new AbortController(); voteReadController = controller;\r\n    comments.voteState = undefined;\r\n    if (!actor) return () => controller.abort();\r\n    void API.Thread.GetThreadVote(threadId, controller.signal).then(res => {\r\n      if (voteReadDisposed || controller.signal.aborted || generation !== voteReadGeneration || threadId !== comments.thread.id || !sameNativeReadAuthority(authority)) return;\r\n      if (res?.status === StatusCodes.OK && res.body && [-1, 0, 1].includes(res.body.vote)) comments.voteState = res.body.vote;\r\n      else if (res?.status === StatusCodes.NO_CONTENT) comments.voteState = 0;\r\n    }).catch(() => { /* Unknown personal state must be freshly read before a write. */ });\r\n    return () => controller.abort();\r\n  });\r\n  let mutexVoteState = $state<-1 | 1>();\r\n  const mutateThreadVote = (value: -1 | 1) => {\r\n    if (mutex.isLocked()) return;\r\n    if (!$Session) { toast.error(\"Please sign in to perform this action\"); return; }\r\n    if (!comments.stats) return;\r\n    const intent = mutationOwner.capture(), threadId = comments.thread.id;\r\n    return mutex.exec(async () => {\r\n      mutexVoteState = value;\r\n      try {\r\n        intent.assert();\r\n        // A click never interprets missing/stale personal state as an absent vote.\r\n        const personal = await API.Thread.GetThreadVote(threadId, intent.signal); intent.assert();\r\n        const previous = personal?.status === StatusCodes.NO_CONTENT ? 0 : personal?.status === StatusCodes.OK ? personal.body?.vote : undefined;\r\n        if (previous === undefined || ![-1, 0, 1].includes(previous)) throw new Constants.Error.Error(\"Vote status unavailable\", \"The current vote could not be confirmed. No change was submitted.\");\r\n        const remove = previous === value;\r\n        const res = await (remove ? API.Thread.UnvoteThread(threadId, intent) : API.Thread.VoteThread(threadId, value, intent)); intent.assert();\r\n        if (res?.status === StatusCodes.NO_CONTENT) {\r\n          voteReadGeneration++; voteReadController?.abort();\r\n          comments.voteState = remove ? 0 : value;\r\n          const current = await API.Thread.Get(threadId); intent.assert();\r\n          if (current?.status !== StatusCodes.OK || !current.body?.relationships.stats) throw new Constants.Error.Error(\"Vote acknowledged\", \"Your vote change succeeded, but current counts could not be loaded. Reload this discussion; the change was not replayed.\");\r\n          comments.stats = { ...current.body.relationships.stats };\r\n        } else if (res?.status === StatusCodes.FORBIDDEN) throw Constants.Error.Forbidden;\r\n        else if (res?.status === StatusCodes.CONFLICT) throw Constants.Error.UpdateConflict;\r\n        else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) throw Constants.Error.TooManyRequests;\r\n        else throw Constants.Error.Unknown;\r\n      } catch (cause) { if (!intent.alive()) return; const error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; toast.error(error.title, { description: error.body }); }\r\n      finally { mutexVoteState = undefined; intent.release(); }\r\n    });\r\n  };\r\n  const onVote = (value: -1 | 1) => mutateThreadVote(value);\r\n  const onDeleteVote = (value: -1 | 1) => mutateThreadVote(value);\r\n"
  },
  {
    "path": "src/lib/components/comments/comments-votes.svelte",
    "label": "round7 thread vote read cleanup import",
    "sha256": "5d7b67c964c3d38c42b7fc6c064be82dfef0c351258a5ff6f576fd702a88a14c",
    "count": 1,
    "before": "\timport { onMount } from \"svelte\";",
    "after": "\timport { onDestroy, onMount } from \"svelte\";"
  },
  {
    "path": "src/lib/API/Settings.ts",
    "label": "round7 preference API accepts explicit original actor intent",
    "sha256": "c44c225d9bf9b3a7b17836dfbe100daa1bf533e6a9d09b4055887222175b956e",
    "count": 1,
    "before": "import { HTTP } from \"$lib\";",
    "after": "import { HTTP } from \"$lib\";\r\nimport { authenticatedMutation, type MutationIntent } from \"$platform/authenticated-mutation\";"
  },
  {
    "path": "src/lib/API/Settings.ts",
    "label": "round7 preferences update actor transport",
    "sha256": "c44c225d9bf9b3a7b17836dfbe100daa1bf533e6a9d09b4055887222175b956e",
    "count": 1,
    "before": "export const Update = (payload: UpdateSettingsRequest) => {\r\n\tconst http = new HTTP<UserPreferences>(`${PUBLIC_API_URL}/settings`, payload);\r\n\thttp.setWithCredentials(true);\r\n\treturn http.PUT();\r\n};\r\n",
    "after": "export const Update = (payload: UpdateSettingsRequest, intent?: MutationIntent) => authenticatedMutation<UserPreferences>(\"/settings\", \"PUT\", payload, intent);\r\n"
  },
  {
    "path": "src/routes/(app)/settings/preferences/+page.svelte",
    "label": "round7 settings preference response uses guarded application seam",
    "sha256": "0d77b163a1ec200ec7cf07951e475f413127b5a49f3f1ef5f137cf3c3baa8b7b",
    "count": 1,
    "before": "\timport { toast } from \"svelte-sonner\";",
    "after": "\timport { toast } from \"svelte-sonner\";\r\n\timport { createMutationOwner } from \"$platform/authenticated-mutation\";\r\n\timport { applyAccountPreferences, captureAccountPreferences } from \"$platform/account-preferences\";\r\n\timport { nativeAuthority } from \"$platform/mutation-authority\";"
  },
  {
    "path": "src/routes/(app)/settings/preferences/+page.svelte",
    "label": "round7 settings page owns preference intent and resets draft on authority switch",
    "sha256": "0d77b163a1ec200ec7cf07951e475f413127b5a49f3f1ef5f137cf3c3baa8b7b",
    "count": 1,
    "before": "\tconst mutex = new Mutex();",
    "after": "\tconst mutex = new Mutex();\r\n\tconst preferenceOwner = createMutationOwner(() => window.location.href);\r\n\t$effect(() => { $nativeAuthority; onReset(); });"
  },
  {
    "path": "src/routes/(app)/settings/preferences/+page.svelte",
    "label": "round7 settings response cannot restore previous account preferences",
    "sha256": "0d77b163a1ec200ec7cf07951e475f413127b5a49f3f1ef5f137cf3c3baa8b7b",
    "count": 1,
    "before": "\tconst onSubmit = (ev: SubmitEvent) => {\r\n\t\tev.preventDefault();\r\n\t\tev.stopPropagation();\r\n\t\tev.stopImmediatePropagation();\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tif (!$prefs || !draft || !isChanged) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\ttry {\r\n\t\t\t\tif ($Session) {\r\n\t\t\t\t\tconst payload: UpdateSettingsRequest = { version: $prefs.version };\r\n\t\t\t\t\tObject.entries(draft).forEach(([key, value]) => {\r\n\t\t\t\t\t\tif (!equal(value, $prefs![key as keyof typeof $prefs])) {\r\n\t\t\t\t\t\t\tif ([\"uploaders\", \"groups\"].includes(key)) {\r\n\t\t\t\t\t\t\t\t(payload[key as keyof UpdateSettingsRequest] as string[]) =\r\n\t\t\t\t\t\t\t\t\t(value as { id: string }[])?.map(({ id }) => id) ?? [];\r\n\t\t\t\t\t\t\t} else if (Array.isArray(value)) {\r\n\t\t\t\t\t\t\t\t(payload[key as keyof UpdateSettingsRequest] as any[]) = Array.from(value as any[]);\r\n\t\t\t\t\t\t\t} else {\r\n\t\t\t\t\t\t\t\t(payload[key as keyof UpdateSettingsRequest] as any) = value;\r\n\t\t\t\t\t\t\t}\r\n\t\t\t\t\t\t}\r\n\t\t\t\t\t});\r\n\t\t\t\t\tif (Object.keys(payload).length <= 1) {\r\n\t\t\t\t\t\treturn;\r\n\t\t\t\t\t}\r\n\r\n\t\t\t\t\tconst res = await API.Settings.Update(payload);\r\n\t\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\t\ttoast.success(\"Your changes have been saved.\");\r\n\t\t\t\t\t\t$prefs = $Session.preferences = res.body;\r\n\t\t\t\t\t\tonReset();\r\n\t\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t\t} else {\r\n\t\t\t\t\tObject.entries(draft).forEach(([key, value]) => {\r\n\t\t\t\t\t\tif (!equal(value, $prefs![key as keyof typeof $prefs])) {\r\n\t\t\t\t\t\t\tif (Array.isArray(value)) {\r\n\t\t\t\t\t\t\t\t($prefs![key as keyof typeof $prefs] as any[]) = Array.from(value as any[]);\r\n\t\t\t\t\t\t\t} else {\r\n\t\t\t\t\t\t\t\t($prefs![key as keyof typeof $prefs] as any) = value;\r\n\t\t\t\t\t\t\t}\r\n\t\t\t\t\t\t}\r\n\t\t\t\t\t});\r\n\r\n\t\t\t\t\ttoast.success(\"Your changes have been saved.\");\r\n\t\t\t\t\t$prefs = $prefs;\r\n\t\t\t\t\tonReset();\r\n\t\t\t\t}\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) {\r\n\t\t\t\t\ttoast.error(e.title, { description: e.body });\r\n\t\t\t\t} else {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t}\r\n\t\t});\r\n\t};\r\n",
    "after": "\tconst onSubmit = (ev: SubmitEvent) => {\r\n\t\tev.preventDefault(); ev.stopPropagation(); ev.stopImmediatePropagation();\r\n\t\tif (mutex.isLocked() || !$prefs || !draft || !isChanged) return;\r\n\t\tconst submitted = $state.snapshot(draft);\r\n\t\tif (!$Session) { prefs.set(structuredClone(submitted)); onReset(); toast.success(\"Your changes have been saved.\"); return; }\r\n\t\tconst intent = preferenceOwner.capture(), selected = captureAccountPreferences();\r\n\t\tconst payload: UpdateSettingsRequest = { version: selected.initial.version };\r\n\t\tObject.entries(submitted).forEach(([key, value]) => {\r\n\t\t\tif (equal(value, selected.initial[key as keyof UserPreferences])) return;\r\n\t\t\tif ([\"uploaders\", \"groups\"].includes(key)) (payload[key as keyof UpdateSettingsRequest] as string[]) = (value as { id: string }[])?.map(({ id }) => id) ?? [];\r\n\t\t\telse (payload[key as keyof UpdateSettingsRequest] as any) = Array.isArray(value) ? [...value] : value;\r\n\t\t});\r\n\t\tif (Object.keys(payload).length <= 1) { intent.release(); return; }\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\ttry {\r\n\t\t\t\tintent.assert(); const res = await API.Settings.Update(payload, intent); intent.assert();\r\n\t\t\t\tif (res?.status === StatusCodes.OK && res.body) { applyAccountPreferences(selected.actor, selected.authority, res.body, intent); onReset(); toast.success(\"Your changes have been saved.\"); }\r\n\t\t\t\telse if (res?.status === StatusCodes.TOO_MANY_REQUESTS) throw Constants.Error.TooManyRequests;\r\n\t\t\t\telse if (res?.status === StatusCodes.CONFLICT) throw Constants.Error.UpdateConflict;\r\n\t\t\t\telse throw Constants.Error.Unknown;\r\n\t\t\t} catch (cause) { if (!intent.alive()) return; const error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; toast.error(error.title, { description: error.body }); }\r\n\t\t\tfinally { intent.release(); }\r\n\t\t});\r\n\t};\r\n"
  },
  {
    "path": "src/routes/(app)/group/[id]/Group.svelte",
    "label": "round7 group preference block owns actor target and apply seam",
    "sha256": "ece279c948c849fb298992cc883b3dbb4492e2d5748acdee430fed276ae3dfcc",
    "count": 1,
    "before": "\timport { toast } from \"svelte-sonner\";",
    "after": "\timport { toast } from \"svelte-sonner\";\r\n\timport { createMutationOwner } from \"$platform/authenticated-mutation\";\r\n\timport { applyAccountPreferences, captureAccountPreferences } from \"$platform/account-preferences\";\r\n\timport { createActorFollowState } from \"$platform/actor-follow-state\";"
  },
  {
    "path": "src/routes/(app)/group/[id]/Group.svelte",
    "label": "round7 group preference mutation target lifetime",
    "sha256": "ece279c948c849fb298992cc883b3dbb4492e2d5748acdee430fed276ae3dfcc",
    "count": 1,
    "before": "\tconst group = writable<ScanlationGroup>(data);",
    "after": "\tconst group = writable<ScanlationGroup>(data);\r\n\tconst preferenceOwner = createMutationOwner(() => JSON.stringify([$group.id, page.url.href]));"
  },
  {
    "path": "src/routes/(app)/group/[id]/Group.svelte",
    "label": "round7 group blocked arrays stay unchanged before real current acknowledgement",
    "sha256": "ece279c948c849fb298992cc883b3dbb4492e2d5748acdee430fed276ae3dfcc",
    "count": 1,
    "before": "\tlet isUpdatingBlockStatus = $state(false);\r\n\tconst updateBlockStatus = () =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tif (!(await Session.isValid())) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tisUpdatingBlockStatus = true;\r\n\t\t\ttry {\r\n\t\t\t\tconst idx = $prefs.groups.findIndex(({ id }) => id === $group.id);\r\n\t\t\t\tif (idx >= 0) $prefs.groups.splice(idx, 1);\r\n\t\t\t\telse $prefs.groups.push({ id: $group.id, name: $group.name, version: $group.version });\r\n\r\n\t\t\t\tif ($Session) {\r\n\t\t\t\t\tconst payload: UpdateSettingsRequest = {\r\n\t\t\t\t\t\tversion: $prefs.version,\r\n\t\t\t\t\t\tgroups: $prefs.groups.map(({ id }) => id)\r\n\t\t\t\t\t};\r\n\t\t\t\t\tif (Object.keys(payload).length <= 1) {\r\n\t\t\t\t\t\treturn;\r\n\t\t\t\t\t}\r\n\r\n\t\t\t\t\tconst res = await API.Settings.Update(payload);\r\n\t\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\t\t$prefs = $Session.preferences = res.body;\r\n\t\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t\t}\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t} finally {\r\n\t\t\t\tisUpdatingBlockStatus = false;\r\n\t\t\t}\r\n\t\t});\r\n",
    "after": "\tlet isUpdatingBlockStatus = $state(false);\r\n\tconst updateBlockStatus = () => {\r\n\t\tif (mutex.isLocked() || !$Session || (!isBlocked && !followState.knownAbsent())) return;\r\n\t\tconst intent = preferenceOwner.capture(), selected = captureAccountPreferences(), target = { id: $group.id, name: $group.name, version: $group.version };\r\n\t\tconst next = selected.initial.groups.filter(row => row.id !== target.id);\r\n\t\tif (next.length === selected.initial.groups.length) next.push(target);\r\n\t\tconst payload: UpdateSettingsRequest = { version: selected.initial.version, groups: next.map(row => row.id) };\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tisUpdatingBlockStatus = true;\r\n\t\t\ttry {\r\n\t\t\t\tintent.assert(); const res = await API.Settings.Update(payload, intent); intent.assert();\r\n\t\t\t\tif (res?.status === StatusCodes.OK && res.body) applyAccountPreferences(selected.actor, selected.authority, res.body, intent);\r\n\t\t\t\telse if (res?.status === StatusCodes.TOO_MANY_REQUESTS) throw Constants.Error.TooManyRequests;\r\n\t\t\t\telse if (res?.status === StatusCodes.CONFLICT) throw Constants.Error.UpdateConflict;\r\n\t\t\t\telse throw Constants.Error.Unknown;\r\n\t\t\t} catch (cause) { if (!intent.alive()) return; const error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; toast.error(error.title, { description: error.body }); }\r\n\t\t\tfinally { isUpdatingBlockStatus = false; intent.release(); }\r\n\t\t});\r\n\t};\r\n"
  },
  {
    "path": "src/routes/(app)/user/[id]/User.svelte",
    "label": "round7 user preference block owns actor target and apply seam",
    "sha256": "f8232429c11dcc2dc08fb34f1c3fc31d262cc547b4becf485b884133ed6db0bd",
    "count": 1,
    "before": "\timport { toast } from \"svelte-sonner\";",
    "after": "\timport { toast } from \"svelte-sonner\";\r\n\timport { createMutationOwner } from \"$platform/authenticated-mutation\";\r\n\timport { applyAccountPreferences, captureAccountPreferences } from \"$platform/account-preferences\";\r\n\timport { createActorFollowState } from \"$platform/actor-follow-state\";"
  },
  {
    "path": "src/routes/(app)/user/[id]/User.svelte",
    "label": "round7 user preference mutation target lifetime",
    "sha256": "f8232429c11dcc2dc08fb34f1c3fc31d262cc547b4becf485b884133ed6db0bd",
    "count": 1,
    "before": "\tconst user = writable<User>(data);",
    "after": "\tconst user = writable<User>(data);\r\n\tconst preferenceOwner = createMutationOwner(() => JSON.stringify([$user.id, page.url.href]));"
  },
  {
    "path": "src/routes/(app)/user/[id]/User.svelte",
    "label": "round7 user blocked arrays stay unchanged before real current acknowledgement",
    "sha256": "f8232429c11dcc2dc08fb34f1c3fc31d262cc547b4becf485b884133ed6db0bd",
    "count": 1,
    "before": "\tlet isUpdatingBlockStatus = $state(false);\r\n\tconst updateBlockStatus = () =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tif (!(await Session.isValid())) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tisUpdatingBlockStatus = true;\r\n\t\t\ttry {\r\n\t\t\t\tconst idx = $prefs.uploaders.findIndex(({ id }) => id === $user.id);\r\n\t\t\t\tif (idx >= 0) $prefs.uploaders.splice(idx, 1);\r\n\t\t\t\telse $prefs.uploaders.push({ id: $user.id, name: $user.name, version: $user.version });\r\n\r\n\t\t\t\tif ($Session) {\r\n\t\t\t\t\tconst payload: UpdateSettingsRequest = {\r\n\t\t\t\t\t\tversion: $prefs.version,\r\n\t\t\t\t\t\tuploaders: $prefs.uploaders.map(({ id }) => id)\r\n\t\t\t\t\t};\r\n\t\t\t\t\tif (Object.keys(payload).length <= 1) {\r\n\t\t\t\t\t\treturn;\r\n\t\t\t\t\t}\r\n\r\n\t\t\t\t\tconst res = await API.Settings.Update(payload);\r\n\t\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\t\t$prefs = $Session.preferences = res.body;\r\n\t\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t\t}\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t} finally {\r\n\t\t\t\tisUpdatingBlockStatus = false;\r\n\t\t\t}\r\n\t\t});\r\n",
    "after": "\tlet isUpdatingBlockStatus = $state(false);\r\n\tconst updateBlockStatus = () => {\r\n\t\tif (mutex.isLocked() || !$Session || (!isBlocked && !followState.knownAbsent())) return;\r\n\t\tconst intent = preferenceOwner.capture(), selected = captureAccountPreferences(), target = { id: $user.id, name: $user.name, version: $user.version };\r\n\t\tconst next = selected.initial.uploaders.filter(row => row.id !== target.id);\r\n\t\tif (next.length === selected.initial.uploaders.length) next.push(target);\r\n\t\tconst payload: UpdateSettingsRequest = { version: selected.initial.version, uploaders: next.map(row => row.id) };\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tisUpdatingBlockStatus = true;\r\n\t\t\ttry {\r\n\t\t\t\tintent.assert(); const res = await API.Settings.Update(payload, intent); intent.assert();\r\n\t\t\t\tif (res?.status === StatusCodes.OK && res.body) applyAccountPreferences(selected.actor, selected.authority, res.body, intent);\r\n\t\t\t\telse if (res?.status === StatusCodes.TOO_MANY_REQUESTS) throw Constants.Error.TooManyRequests;\r\n\t\t\t\telse if (res?.status === StatusCodes.CONFLICT) throw Constants.Error.UpdateConflict;\r\n\t\t\t\telse throw Constants.Error.Unknown;\r\n\t\t\t} catch (cause) { if (!intent.alive()) return; const error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; toast.error(error.title, { description: error.body }); }\r\n\t\t\tfinally { isUpdatingBlockStatus = false; intent.release(); }\r\n\t\t});\r\n\t};\r\n"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "round7 thumbnail resources are discarded only after acknowledged removal",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\timport { toast } from \"svelte-sonner\";",
    "after": "\timport { toast } from \"svelte-sonner\";\r\n\timport { releaseDiscardedThumbnails } from \"$platform/blob-lifetime\";"
  },
  {
    "path": "src/lib/components/upload/UploadChapterPages.svelte",
    "label": "round7 duplicate deletion failure keeps original pages and working thumbnails",
    "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
    "count": 1,
    "before": "\tconst deleteDuplicate = () =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tif (!uploadSession) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tisDeletingDuplicate = true;\r\n\t\t\ttry {\r\n\t\t\t\towner.assert();\r\n\t\t\t\tif (pages.some(page => page.upload_recovery?.state === \"unknown\")) throw new Constants.Error.Error(\"Reconcile upload first\", \"Unknown page outcomes cannot be discarded as duplicates. Reload exact owner staging or explicitly cancel this upload.\");\r\n\t\t\t\tconst unique = new Map<string, UploadSessionFile>();\r\n\t\t\t\tconst duplicateIds: string[] = [];\r\n\r\n\t\t\t\tpages.forEach(e => {\r\n\t\t\t\t\tconst key = e.file_hash ?? e.original_file_name!;\r\n\t\t\t\t\tif (unique.has(key)) {\r\n\t\t\t\t\t\tif (!e.file && !e.error) {\r\n\t\t\t\t\t\t\tduplicateIds.push(e.id);\r\n\t\t\t\t\t\t}\r\n\t\t\t\t\t\trevokeThumbnail(e);\r\n\t\t\t\t\t} else {\r\n\t\t\t\t\t\tunique.set(key, e);\r\n\t\t\t\t\t}\r\n\t\t\t\t});\r\n\r\n\t\t\t\tif (duplicateIds.length) {\r\n\t\t\t\t\tconst res = await owner.request(`/upload/${uploadSession.id}/batch`, \"DELETE\", duplicateIds);\r\n\t\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t\t}\r\n\r\n\t\t\t\tpages = Array.from(unique.values());\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t} finally {\r\n\t\t\t\tisDeletingDuplicate = false;\r\n\t\t\t}\r\n\t\t});\r\n\r\n",
    "after": "\tconst deleteDuplicate = () => mutex.exec(async () => {\r\n\t\tif (!uploadSession || uploadSession.can_mutate === false) return;\r\n\t\tconst sessionId = uploadSession.id;\r\n\t\tisDeletingDuplicate = true;\r\n\t\ttry {\r\n\t\t\towner.assert();\r\n\t\t\tif (pages.some(page => page.upload_recovery?.state === \"unknown\")) throw new Constants.Error.Error(\"Reconcile upload first\", \"Unknown page outcomes cannot be discarded as duplicates. Reload exact owner staging or explicitly cancel this upload.\");\r\n\t\t\tconst unique = new Map<string | symbol, UploadSessionFile>(), discarded: UploadSessionFile[] = [], remoteIds: string[] = [];\r\n\t\t\tfor (const row of pages) {\r\n\t\t\t\t// Only acknowledged full server hashes prove content equality.\r\n\t\t\t\tconst key = !row.file && !row.error && /^[0-9a-f]{64}$/.test(row.file_hash ?? \"\") ? row.file_hash : Symbol(\"unproven page\");\r\n\t\t\t\tif (unique.has(key)) { discarded.push(row); if (!row.file && !row.error) remoteIds.push(row.id); }\r\n\t\t\t\telse unique.set(key, row);\r\n\t\t\t}\r\n\t\t\tif (remoteIds.length) {\r\n\t\t\t\tconst res = await owner.request(`/upload/${sessionId}/batch`, \"DELETE\", remoteIds); owner.assert();\r\n\t\t\t\tif (res?.status !== StatusCodes.NO_CONTENT) throw res?.status === StatusCodes.FORBIDDEN ? Constants.Error.Forbidden : res?.status === StatusCodes.TOO_MANY_REQUESTS ? Constants.Error.TooManyRequests : res?.status === StatusCodes.CONFLICT ? Constants.Error.UpdateConflict : Constants.Error.Unknown;\r\n\t\t\t}\r\n\t\t\towner.assert(); if (uploadSession?.id !== sessionId) throw Constants.Error.UpdateConflict;\r\n\t\t\tconst kept = Array.from(unique.values()); pages = kept;\r\n\t\t\treleaseDiscardedThumbnails(discarded, kept);\r\n\t\t\tif (kept.some(row => row.file || row.error || !/^[0-9a-f]{64}$/.test(row.file_hash ?? \"\"))) toast.info(\"Unproven duplicates were kept\", { description: \"Only acknowledged identical content hashes qualify. Matching filenames do not prove identical images; no upload was replayed.\" });\r\n\t\t} catch (cause) { const error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; toast.error(error.title, { description: error.body }); }\r\n\t\tfinally { isDeletingDuplicate = false; }\r\n\t});\r\n\r\n"
  },
  {
    "path": "src/lib/API/Group.ts",
    "label": "round8 Group follow API explicit action admission",
    "sha256": "34a3cdff780916df8e4f500ce6aa0e1ff6eedc6e728514de6f364605fd6433ff",
    "count": 1,
    "before": "import { HTTP } from \"$lib\";",
    "after": "import { HTTP } from \"$lib\";\r\nimport { authenticatedMutation, type MutationIntent } from \"$platform/authenticated-mutation\";\r\nimport { readThreadContext } from \"$platform/thread-read\";"
  },
  {
    "path": "src/lib/API/Group.ts",
    "label": "round8 Group Follow admitted actor target transport",
    "sha256": "34a3cdff780916df8e4f500ce6aa0e1ff6eedc6e728514de6f364605fd6433ff",
    "count": 1,
    "before": "export const Follow = (id: string) => {\r\n\tconst http = new HTTP(`${PUBLIC_API_URL}/user/follows/groups/${id}`);\r\n\thttp.setWithCredentials(true);\r\n\treturn http.POST();\r\n};",
    "after": "export const Follow = (id: string, intent?: MutationIntent) => authenticatedMutation<void>(`/user/follows/groups/${encodeURIComponent(id)}`, \"POST\", undefined, intent);"
  },
  {
    "path": "src/lib/API/Group.ts",
    "label": "round8 Group Unfollow admitted actor target transport",
    "sha256": "34a3cdff780916df8e4f500ce6aa0e1ff6eedc6e728514de6f364605fd6433ff",
    "count": 1,
    "before": "export const Unfollow = (id: string) => {\r\n\tconst http = new HTTP(`${PUBLIC_API_URL}/user/follows/groups/${id}`);\r\n\thttp.setWithCredentials(true);\r\n\treturn http.DELETE();\r\n};",
    "after": "export const Unfollow = (id: string, intent?: MutationIntent) => authenticatedMutation<void>(`/user/follows/groups/${encodeURIComponent(id)}`, \"DELETE\", undefined, intent);"
  },
  {
    "path": "src/lib/API/Group.ts",
    "label": "round8 Group private follow status current authority read",
    "sha256": "34a3cdff780916df8e4f500ce6aa0e1ff6eedc6e728514de6f364605fd6433ff",
    "count": 1,
    "before": "export const GetFollowStatus = (id: string) => {\r\n\tconst http = new HTTP(`${PUBLIC_API_URL}/user/follows/groups/${id}`);\r\n\thttp.setWithCredentials(true);\r\n\treturn http.GET();\r\n};",
    "after": "export const GetFollowStatus = (id: string) => readThreadContext<void>(`/user/follows/groups/${encodeURIComponent(id)}`);"
  },
  {
    "path": "src/routes/(app)/group/[id]/Group.svelte",
    "label": "round8 Group follow freezes user action before every await",
    "sha256": "ece279c948c849fb298992cc883b3dbb4492e2d5748acdee430fed276ae3dfcc",
    "count": 1,
    "before": "\tconst updateFollowStatus = () =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tif (!(await Session.isValid())) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tisUpdatingFollowStatus = true;\r\n\t\t\ttry {\r\n\t\t\t\tconst res = await (isFollowed ? API.Group.Unfollow : API.Group.Follow)($group.id);\r\n\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\tisFollowed = !isFollowed;\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t} finally {\r\n\t\t\t\tisUpdatingFollowStatus = false;\r\n\t\t\t}\r\n\t\t});\r\n\r\n",
    "after": "\tconst updateFollowStatus = () => followState.activate();\r\n\r\n"
  },
  {
    "path": "src/routes/(app)/group/[id]/Group.svelte",
    "label": "round8 Group follow read cannot attach to disposed or switched owner",
    "sha256": "ece279c948c849fb298992cc883b3dbb4492e2d5748acdee430fed276ae3dfcc",
    "count": 1,
    "before": "\tconst getFollowStatus = async () => {\r\n\t\tif (!(await Session.isValid())) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tconst res = await API.Group.GetFollowStatus($group.id);\r\n\t\tisFollowed = res?.status === StatusCodes.NO_CONTENT;\r\n\t};\r\n\r\n",
    "after": ""
  },
  {
    "path": "src/lib/API/User.ts",
    "label": "round8 User follow API explicit action admission",
    "sha256": "b4762d35d1b4033e2550641a64c9fd6ab565fa6fc374eaf326415732c5593f90",
    "count": 1,
    "before": "import { HTTP } from \"$lib\";",
    "after": "import { HTTP } from \"$lib\";\r\nimport { authenticatedMutation, type MutationIntent } from \"$platform/authenticated-mutation\";\r\nimport { readThreadContext } from \"$platform/thread-read\";"
  },
  {
    "path": "src/lib/API/User.ts",
    "label": "round8 User Follow admitted actor target transport",
    "sha256": "b4762d35d1b4033e2550641a64c9fd6ab565fa6fc374eaf326415732c5593f90",
    "count": 1,
    "before": "export const Follow = (id: string) => {\r\n\tconst http = new HTTP(`${PUBLIC_API_URL}/user/follows/users/${id}`);\r\n\thttp.setWithCredentials(true);\r\n\treturn http.POST();\r\n};",
    "after": "export const Follow = (id: string, intent?: MutationIntent) => authenticatedMutation<void>(`/user/follows/users/${encodeURIComponent(id)}`, \"POST\", undefined, intent);"
  },
  {
    "path": "src/lib/API/User.ts",
    "label": "round8 User Unfollow admitted actor target transport",
    "sha256": "b4762d35d1b4033e2550641a64c9fd6ab565fa6fc374eaf326415732c5593f90",
    "count": 1,
    "before": "export const Unfollow = (id: string) => {\r\n\tconst http = new HTTP(`${PUBLIC_API_URL}/user/follows/users/${id}`);\r\n\thttp.setWithCredentials(true);\r\n\treturn http.DELETE();\r\n};",
    "after": "export const Unfollow = (id: string, intent?: MutationIntent) => authenticatedMutation<void>(`/user/follows/users/${encodeURIComponent(id)}`, \"DELETE\", undefined, intent);"
  },
  {
    "path": "src/lib/API/User.ts",
    "label": "round8 User private follow status current authority read",
    "sha256": "b4762d35d1b4033e2550641a64c9fd6ab565fa6fc374eaf326415732c5593f90",
    "count": 1,
    "before": "export const GetFollowStatus = (id: string) => {\r\n\tconst http = new HTTP(`${PUBLIC_API_URL}/user/follows/users/${id}`);\r\n\thttp.setWithCredentials(true);\r\n\treturn http.GET();\r\n};",
    "after": "export const GetFollowStatus = (id: string) => readThreadContext<void>(`/user/follows/users/${encodeURIComponent(id)}`);"
  },
  {
    "path": "src/routes/(app)/user/[id]/User.svelte",
    "label": "round8 User follow freezes user action before every await",
    "sha256": "f8232429c11dcc2dc08fb34f1c3fc31d262cc547b4becf485b884133ed6db0bd",
    "count": 1,
    "before": "\tconst updateFollowStatus = () =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tif (!(await Session.isValid())) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tisUpdatingFollowStatus = true;\r\n\t\t\ttry {\r\n\t\t\t\tconst res = await (isFollowed ? API.User.Unfollow : API.User.Follow)($user.id);\r\n\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\tisFollowed = !isFollowed;\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t} finally {\r\n\t\t\t\tisUpdatingFollowStatus = false;\r\n\t\t\t}\r\n\t\t});\r\n\r\n",
    "after": "\tconst updateFollowStatus = () => followState.activate();\r\n\r\n"
  },
  {
    "path": "src/routes/(app)/user/[id]/User.svelte",
    "label": "round8 User follow read cannot attach to disposed or switched owner",
    "sha256": "f8232429c11dcc2dc08fb34f1c3fc31d262cc547b4becf485b884133ed6db0bd",
    "count": 1,
    "before": "\tconst getFollowStatus = async () => {\r\n\t\tif (!(await Session.isValid())) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\ttry {\r\n\t\t\tconst res = await API.User.GetFollowStatus($user.id);\r\n\t\t\tisFollowed = res?.status === StatusCodes.NO_CONTENT;\r\n\t\t} catch (e) {\r\n\t\t\tconsole.error(e);\r\n\t\t}\r\n\t};\r\n\r\n",
    "after": ""
  },
  {
    "path": "src/routes/(app)/forums/threads/[id]/Thread.svelte",
    "label": "round8 existing thread share behavior import",
    "sha256": "31d385f93ec915026be432686bd211af76df1301314e17349ae55af76cb1f3bf",
    "count": 1,
    "before": "\timport { Share2 } from \"@lucide/svelte\";",
    "after": "\timport { Share2 } from \"@lucide/svelte\";\r\n\timport { onDestroy } from \"svelte\";\r\n\timport { toast } from \"svelte-sonner\";"
  },
  {
    "path": "src/routes/(app)/forums/threads/[id]/Thread.svelte",
    "label": "round8 canonical same-instance share honest user gesture",
    "sha256": "31d385f93ec915026be432686bd211af76df1301314e17349ae55af76cb1f3bf",
    "count": 1,
    "before": "\tconst comments = useComments();",
    "after": "\tconst comments = useComments();\r\n\tlet sharing = false, shareDisposed = false;\r\n\tonDestroy(() => { shareDisposed = true; });\r\n\tconst shareCurrentThread = async () => {\r\n\t\tif (sharing || shareDisposed) return;\r\n\t\tconst threadId = comments.thread.id, url = new URL(`/forums/threads/${encodeURIComponent(threadId)}`, window.location.origin).href;\r\n\t\tconst alive = () => !shareDisposed && comments.thread.id === threadId;\r\n\t\tsharing = true;\r\n\t\ttry {\r\n\t\t\tif (typeof navigator.share === \"function\") { await navigator.share({ title: document.title, url }); if (alive()) toast.success(\"Thread shared\"); }\r\n\t\t\telse { if (!navigator.clipboard?.writeText) throw Error(\"Clipboard sharing is unavailable. Copy the thread URL from the address bar.\"); await navigator.clipboard.writeText(url); if (alive()) toast.success(\"Thread link copied\"); }\r\n\t\t} catch (failure) {\r\n\t\t\tif (!alive()) return;\r\n\t\t\tif (failure instanceof DOMException && failure.name === \"AbortError\") toast.info(\"Sharing cancelled\");\r\n\t\t\telse toast.error(\"Thread could not be shared\", { description: failure instanceof Error ? failure.message : \"Sharing or clipboard permission was unavailable. No external post was made.\" });\r\n\t\t} finally { sharing = false; }\r\n\t};"
  },
  {
    "path": "src/routes/(app)/forums/threads/[id]/Thread.svelte",
    "label": "approved round8 existing share button handler and accessible name",
    "sha256": "31d385f93ec915026be432686bd211af76df1301314e17349ae55af76cb1f3bf",
    "count": 1,
    "before": "\t\t\t\t\t\t<button>\r\n\t\t\t\t\t\t\t<Share2 size=\"14\" />",
    "after": "\t\t\t\t\t\t<button type=\"button\" title=\"Share thread\" aria-label=\"Share thread\" onclick={shareCurrentThread}>\r\n\t\t\t\t\t\t\t<Share2 size=\"14\" />",
    "approved_share_control_exception": "existing_thread_share_behavior"
  },
  {
    "path": "src/lib/API/Manga.ts",
    "label": "round9 Manga search accepts owned cancellation",
    "sha256": "774f88cdb48ae17ddd08dfc532082ce59069b3263525db9ee21b8e5ab4466193",
    "count": 1,
    "before": "export const Search = (searchParams: URLSearchParams) => {",
    "after": "export const Search = (searchParams: URLSearchParams, signal?: AbortSignal) => {\r\n\tif (signal) return readPublicSearch<MangaFeedResponse>(\"/manga\", searchParams, signal);"
  },
  {
    "path": "src/lib/API/Manga.ts",
    "label": "round9 Manga search cancellation import",
    "sha256": "774f88cdb48ae17ddd08dfc532082ce59069b3263525db9ee21b8e5ab4466193",
    "count": 1,
    "before": "import { PUBLIC_API_URL } from \"$env/static/public\";",
    "after": "import { PUBLIC_API_URL } from \"$env/static/public\";\r\nimport { readPublicSearch } from \"$platform/latest-search\";"
  },
  {
    "path": "src/lib/API/Author.ts",
    "label": "round9 Author search accepts owned cancellation",
    "sha256": "48a8f9f7aaac6a5562044118a5242e72f1ba05e057ebf371bd08251e87192360",
    "count": 1,
    "before": "export const Search = (searchParams: URLSearchParams) => {",
    "after": "export const Search = (searchParams: URLSearchParams, signal?: AbortSignal) => {\r\n\tif (signal) return readPublicSearch<TagFeedResponse>(\"/author\", searchParams, signal);"
  },
  {
    "path": "src/lib/API/Author.ts",
    "label": "round9 Author search cancellation import",
    "sha256": "48a8f9f7aaac6a5562044118a5242e72f1ba05e057ebf371bd08251e87192360",
    "count": 1,
    "before": "import { PUBLIC_API_URL } from \"$env/static/public\";",
    "after": "import { PUBLIC_API_URL } from \"$env/static/public\";\r\nimport { readPublicSearch } from \"$platform/latest-search\";"
  },
  {
    "path": "src/lib/API/Group.ts",
    "label": "round9 Group search accepts owned cancellation",
    "sha256": "34a3cdff780916df8e4f500ce6aa0e1ff6eedc6e728514de6f364605fd6433ff",
    "count": 1,
    "before": "export const Search = (searchParams: URLSearchParams) => {",
    "after": "export const Search = (searchParams: URLSearchParams, signal?: AbortSignal) => {\r\n\tif (signal) return readPublicSearch<ScanlationGroupFeedResponse>(\"/group\", searchParams, signal);"
  },
  {
    "path": "src/lib/API/Group.ts",
    "label": "round9 Group search cancellation import",
    "sha256": "34a3cdff780916df8e4f500ce6aa0e1ff6eedc6e728514de6f364605fd6433ff",
    "count": 1,
    "before": "import { PUBLIC_API_URL } from \"$env/static/public\";",
    "after": "import { PUBLIC_API_URL } from \"$env/static/public\";\r\nimport { readPublicSearch } from \"$platform/latest-search\";"
  },
  {
    "path": "src/lib/API/User.ts",
    "label": "round9 User search accepts owned cancellation",
    "sha256": "b4762d35d1b4033e2550641a64c9fd6ab565fa6fc374eaf326415732c5593f90",
    "count": 1,
    "before": "export const Search = (searchParams: URLSearchParams) => {",
    "after": "export const Search = (searchParams: URLSearchParams, signal?: AbortSignal) => {\r\n\tif (signal) return readPublicSearch<UserFeedResponse>(\"/user\", searchParams, signal, true);"
  },
  {
    "path": "src/lib/API/User.ts",
    "label": "round9 User search cancellation import",
    "sha256": "b4762d35d1b4033e2550641a64c9fd6ab565fa6fc374eaf326415732c5593f90",
    "count": 1,
    "before": "import { PUBLIC_API_URL } from \"$env/static/public\";",
    "after": "import { PUBLIC_API_URL } from \"$env/static/public\";\r\nimport { readPublicSearch } from \"$platform/latest-search\";"
  },
  {
    "path": "src/lib/components/SearchDialog.svelte",
    "label": "round9 global search latest-query owner import",
    "sha256": "5c85175c8fb0a34e31e975ea22dbe6f89cbcf0868ea80c72e6d37474e813ba9a",
    "count": 1,
    "before": "\timport { API, Mutex } from \"$lib\";",
    "after": "\timport { API, Mutex } from \"$lib\";\r\n\timport { createLatestSearch } from \"$platform/latest-search\";\r\n\timport { toast } from \"svelte-sonner\";"
  },
  {
    "path": "src/lib/components/SearchDialog.svelte",
    "label": "round9 global search aborts old query and never drops latest",
    "sha256": "5c85175c8fb0a34e31e975ea22dbe6f89cbcf0868ea80c72e6d37474e813ba9a",
    "count": 1,
    "before": "\tconst mutex = new Mutex();\r\n\tlet timeout = $state(0);\r\n\r\n\tconst searchManga = async (title: string) => {\r\n\t\ttry {\r\n\t\t\tconst q = new URLSearchParams();\r\n\t\t\tprefs.applyMangaPreferences(q);\r\n\r\n\t\t\tq.set(\"title\", title);\r\n\t\t\tq.set(\"sort\", \"relevance\");\r\n\t\t\tq.set(\"limit\", \"5\");\r\n\r\n\t\t\tconst res = await API.Manga.Search(q);\r\n\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\tmangaFeed = res.body;\r\n\t\t\t}\r\n\t\t} catch (e) {\r\n\t\t\tconsole.error(e);\r\n\t\t}\r\n\t};\r\n\r\n\tconst searchAuthors = async (name: string) => {\r\n\t\ttry {\r\n\t\t\tconst q = new URLSearchParams();\r\n\t\t\tq.set(\"name\", name);\r\n\t\t\tq.set(\"limit\", \"5\");\r\n\r\n\t\t\tconst res = await API.Author.Search(q);\r\n\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\tauthorFeed = res.body;\r\n\t\t\t}\r\n\t\t} catch (e) {\r\n\t\t\tconsole.error(e);\r\n\t\t}\r\n\t};\r\n\r\n\tconst searchGroups = async (name: string) => {\r\n\t\ttry {\r\n\t\t\tconst q = new URLSearchParams();\r\n\t\t\tq.set(\"name\", name);\r\n\t\t\tq.set(\"limit\", \"5\");\r\n\r\n\t\t\tconst res = await API.Group.Search(q);\r\n\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\tgroupFeed = res.body;\r\n\t\t\t}\r\n\t\t} catch (e) {\r\n\t\t\tconsole.error(e);\r\n\t\t}\r\n\t};\r\n\r\n\tconst onSearch = async () => {\r\n\t\tif (!query.length) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tawait Promise.all([searchManga(query), searchAuthors(query), searchGroups(query)]);\r\n\t};\r\n\r\n\tconst onInput = () => {\r\n\t\twindow.clearTimeout(timeout);\r\n\t\ttimeout = window.setTimeout(\r\n\t\t\t() =>\r\n\t\t\t\tmutex.exec(() => {\r\n\t\t\t\t\tmangaFeed = undefined;\r\n\t\t\t\t\tauthorFeed = undefined;\r\n\t\t\t\t\tgroupFeed = undefined;\r\n\t\t\t\t\treturn onSearch();\r\n\t\t\t\t}),\r\n\t\t\t500\r\n\t\t);\r\n\t};\r\n\r\n\tconst onSubmit = () => {\r\n\t\twindow.clearTimeout(timeout);\r\n\t\tmutex.exec(() => {\r\n\t\t\tmangaFeed = undefined;\r\n\t\t\tauthorFeed = undefined;\r\n\t\t\tgroupFeed = undefined;\r\n\t\t\treturn onSearch();\r\n\t\t});\r\n\t};\r\n",
    "after": "  const mutex = new Mutex();\r\n  const latest = createLatestSearch(busy => busy ? mutex.lock() : mutex.unlock());\r\n  const clear = () => { mangaFeed = undefined; authorFeed = undefined; groupFeed = undefined; };\r\n  const search = (delay: number) => {\r\n    const text = query; clear();\r\n    if (!$isShowSearchDialog || !text.length) { latest.invalidate(); return; }\r\n    latest.schedule(delay, async signal => {\r\n      const mq = new URLSearchParams(); prefs.applyMangaPreferences(mq); mq.set(\"title\", text); mq.set(\"sort\", \"relevance\"); mq.set(\"limit\", \"5\");\r\n      const aq = new URLSearchParams(); aq.set(\"name\", text); aq.set(\"limit\", \"5\");\r\n      const gq = new URLSearchParams(); gq.set(\"name\", text); gq.set(\"limit\", \"5\");\r\n      const responses = await Promise.all([API.Manga.Search(mq, signal), API.Author.Search(aq, signal), API.Group.Search(gq, signal)]);\r\n      if (responses.some(res => res?.status !== StatusCodes.OK || !res.body)) throw Error(\"Search could not be completed. Try the current query again.\");\r\n      return responses;\r\n    }, responses => {\r\n      if (!$isShowSearchDialog || query !== text) return;\r\n      [mangaFeed, authorFeed, groupFeed] = responses.map(res => res!.body) as [MangaFeedResponse, TagFeedResponse, ScanlationGroupFeedResponse];\r\n    }, cause => toast.error(\"Search failed\", { description: cause instanceof Error ? cause.message : \"The current search could not be loaded.\" }));\r\n  };\r\n  $effect.pre(() => { const open = $isShowSearchDialog; untrack(() => { latest.invalidate(); clear(); if (open && query.length) search(0); }); });\r\n  const onInput = () => search(500);\r\n  const onSubmit = (event?: SubmitEvent) => { event?.preventDefault(); search(0); };\r\n"
  },
  {
    "path": "src/lib/components/SearchDialog.svelte",
    "label": "round9 global open effect untracks query-only callbacks",
    "sha256": "5c85175c8fb0a34e31e975ea22dbe6f89cbcf0868ea80c72e6d37474e813ba9a",
    "count": 1,
    "before": "\timport { StatusCodes } from \"http-status-codes\";",
    "after": "\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { untrack } from \"svelte\";"
  },
  {
    "path": "src/lib/components/EntitySearchDialog.svelte",
    "label": "round9 entity query owner import",
    "sha256": "924445fe539f874b355a7e20c134e4df8f52b29442b24119c1857f31ed41e4e2",
    "count": 1,
    "before": "\timport { toast } from \"svelte-sonner\";",
    "after": "\timport { toast } from \"svelte-sonner\";\r\n\timport { createLatestSearch } from \"$platform/latest-search\";"
  },
  {
    "path": "src/lib/components/EntitySearchDialog.svelte",
    "label": "round9 entity query callback cancellation",
    "sha256": "924445fe539f874b355a7e20c134e4df8f52b29442b24119c1857f31ed41e4e2",
    "count": 1,
    "before": "onSearch: (query: string) => Promise<FeedResponse<any>>;",
    "after": "onSearch: (query: string, signal?: AbortSignal) => Promise<FeedResponse<any>>;"
  },
  {
    "path": "src/lib/components/EntitySearchDialog.svelte",
    "label": "round9 entity search latest result and lifetime ownership",
    "sha256": "924445fe539f874b355a7e20c134e4df8f52b29442b24119c1857f31ed41e4e2",
    "count": 1,
    "before": "\tconst mutex = new Mutex();\r\n\r\n\tlet feed = $state<FeedResponse<any>>();\r\n\tlet query = $state<string>(\"\");\r\n\tlet timeout = $state(0);\r\n\r\n\tconst search = async () => {\r\n\t\tfeed = undefined;\r\n\t\tif (query.length) {\r\n\t\t\ttry {\r\n\t\t\t\tfeed = await onSearch(query);\r\n\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t}\r\n\t\t}\r\n\t};\r\n\r\n\tconst onInput = () => {\r\n\t\twindow.clearTimeout(timeout);\r\n\t\ttimeout = window.setTimeout(() => mutex.exec(search), 500);\r\n\t};\r\n\r\n\tconst onSubmit = () => {\r\n\t\twindow.clearTimeout(timeout);\r\n\t\tmutex.exec(search);\r\n\t};\r\n",
    "after": "  const mutex = new Mutex();\r\n  let feed = $state<FeedResponse<any>>();\r\n  let query = $state<string>(\"\");\r\n  const latest = createLatestSearch(busy => busy ? mutex.lock() : mutex.unlock());\r\n  const search = (delay: number) => {\r\n    const text = query, callback = onSearch; feed = undefined;\r\n    if (!text.length) { latest.invalidate(); return; }\r\n    latest.schedule(delay, signal => callback(text, signal), result => { if (query === text && callback === onSearch) feed = result; }, error => {\r\n      if (error instanceof Constants.Error.Error) toast.error(error.title, { description: error.body });\r\n      else toast.error(\"Search failed\", { description: error instanceof Error ? error.message : \"Try the current query again.\" });\r\n    });\r\n  };\r\n  $effect.pre(() => { onSearch; latest.invalidate(); feed = undefined; });\r\n  const onInput = () => search(500);\r\n  const onSubmit = (event?: SubmitEvent) => { event?.preventDefault(); search(0); };\r\n"
  },
  {
    "path": "src/lib/components/draft/manga/MetadataSection.svelte",
    "label": "round9 entity selector accepts signal",
    "sha256": "d0b045bb394261124b66a8e387f8e6215000de77e1fea2d831833515c2776041",
    "count": 1,
    "before": "const onSearch = async (query: string) => {",
    "after": "const onSearch = async (query: string, signal?: AbortSignal) => {"
  },
  {
    "path": "src/lib/components/draft/manga/MetadataSection.svelte",
    "label": "round9 entity selector forwards cancellation",
    "sha256": "d0b045bb394261124b66a8e387f8e6215000de77e1fea2d831833515c2776041",
    "count": 1,
    "before": "const res = await API.Author.Search(q);",
    "after": "const res = await API.Author.Search(q, signal);"
  },
  {
    "path": "src/lib/components/upload/UploadChapterGroups.svelte",
    "label": "round9 entity selector accepts signal",
    "sha256": "46e6be074d81f70ce14fd47a3e8a74b12542c3271b74d810db4c4aa2f9fa480a",
    "count": 1,
    "before": "const onSearch = async (query: string) => {",
    "after": "const onSearch = async (query: string, signal?: AbortSignal) => {"
  },
  {
    "path": "src/lib/components/upload/UploadChapterGroups.svelte",
    "label": "round9 entity selector forwards cancellation",
    "sha256": "46e6be074d81f70ce14fd47a3e8a74b12542c3271b74d810db4c4aa2f9fa480a",
    "count": 1,
    "before": "const res = await API.Group.Search(q);",
    "after": "const res = await API.Group.Search(q, signal);"
  },
  {
    "path": "src/routes/(app)/group/[id]/settings/+page.svelte",
    "label": "round9 entity selector accepts signal",
    "sha256": "9d7319ce7ccae8203e4b79c01453257cb009db0696e9c85080188e26e41a4a98",
    "count": 1,
    "before": "const onSearch = async (query: string) => {",
    "after": "const onSearch = async (query: string, signal?: AbortSignal) => {"
  },
  {
    "path": "src/routes/(app)/group/[id]/settings/+page.svelte",
    "label": "round9 entity selector forwards cancellation",
    "sha256": "9d7319ce7ccae8203e4b79c01453257cb009db0696e9c85080188e26e41a4a98",
    "count": 1,
    "before": "const res = await API.User.Search(q);",
    "after": "const res = await API.User.Search(q, signal);"
  },
  {
    "path": "src/lib/API/Client.ts",
    "label": "round9 client name exact backend validation import",
    "sha256": "011d557b6998176b61a260ca7c2b580181d18c7bbb88b589ce290f324d29a662",
    "count": 1,
    "before": "import { HTTP } from \"$lib\";",
    "after": "import { HTTP } from \"$lib\";\r\nimport { requireClientName } from \"$platform/client-name\";"
  },
  {
    "path": "src/lib/API/Client.ts",
    "label": "round9 client name create validation",
    "sha256": "011d557b6998176b61a260ca7c2b580181d18c7bbb88b589ce290f324d29a662",
    "count": 1,
    "before": "export const Create = (name: string) => {",
    "after": "export const Create = (name: string) => {\r\n\tname = requireClientName(name);"
  },
  {
    "path": "src/lib/API/Client.ts",
    "label": "round9 client name rename validation",
    "sha256": "011d557b6998176b61a260ca7c2b580181d18c7bbb88b589ce290f324d29a662",
    "count": 1,
    "before": "export const Update = (id: string, name: string) => {",
    "after": "export const Update = (id: string, name: string) => {\r\n\tname = requireClientName(name);"
  },
  {
    "path": "src/routes/(app)/settings/client/ClientCreateDialog.svelte",
    "label": "round9 client dialog semantic validation import",
    "sha256": "ca292670a485d3bd8ffdcf720e563a418c018b52e5d97e74c37dbe3a682fbda1",
    "count": 1,
    "before": "\timport { toast } from \"svelte-sonner\";",
    "after": "\timport { toast } from \"svelte-sonner\";\r\n\timport { validClientName, requireClientName } from \"$platform/client-name\";\r\n\timport { HTTPError } from \"$lib/HTTP\";"
  },
  {
    "path": "src/routes/(app)/settings/client/ClientCreateDialog.svelte",
    "label": "round9 client sends normalized supported name",
    "sha256": "ca292670a485d3bd8ffdcf720e563a418c018b52e5d97e74c37dbe3a682fbda1",
    "count": 1,
    "before": "const res = await API.Client.Create(name);",
    "after": "const res = await API.Client.Create(requireClientName(name));"
  },
  {
    "path": "src/routes/(app)/settings/client/ClientCreateDialog.svelte",
    "label": "round9 client actionable400 response",
    "sha256": "ca292670a485d3bd8ffdcf720e563a418c018b52e5d97e74c37dbe3a682fbda1",
    "count": 1,
    "before": "} else if (res?.status === StatusCodes.FORBIDDEN) {",
    "after": "} else if (res?.status === StatusCodes.BAD_REQUEST) {\r\n\t\t\t\t\tthrow new HTTPError(400, \"Client name must contain 1 to 64 printable Unicode characters after trimming whitespace.\", \"Invalid client name\");\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {"
  },
  {
    "path": "src/routes/(app)/settings/client/ClientCreateDialog.svelte",
    "label": "round9 client semantic error surface",
    "sha256": "ca292670a485d3bd8ffdcf720e563a418c018b52e5d97e74c37dbe3a682fbda1",
    "count": 1,
    "before": "if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });",
    "after": "if (e instanceof HTTPError) toast.error(e.title, { description: e.error });\r\n\t\t\t\telse if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });"
  },
  {
    "path": "src/routes/(app)/settings/client/ClientCreateDialog.svelte",
    "label": "round9 client name minimum",
    "sha256": "ca292670a485d3bd8ffdcf720e563a418c018b52e5d97e74c37dbe3a682fbda1",
    "count": 1,
    "before": "minlength={5}",
    "after": "minlength={1}",
    "approved_client_name_exception": "supported_client_name_validation"
  },
  {
    "path": "src/routes/(app)/settings/client/ClientCreateDialog.svelte",
    "label": "round9 client name maximum UTF16 guard",
    "sha256": "ca292670a485d3bd8ffdcf720e563a418c018b52e5d97e74c37dbe3a682fbda1",
    "count": 1,
    "before": "maxlength={255}",
    "after": "maxlength={128}",
    "approved_client_name_exception": "supported_client_name_validation"
  },
  {
    "path": "src/routes/(app)/settings/client/ClientCreateDialog.svelte",
    "label": "round9 client name supported disclosure",
    "sha256": "ca292670a485d3bd8ffdcf720e563a418c018b52e5d97e74c37dbe3a682fbda1",
    "count": 1,
    "before": "Must be between 5 and 255 characters.",
    "after": "Must contain 1 to 64 printable Unicode characters after trimming whitespace.",
    "approved_client_name_exception": "supported_client_name_validation"
  },
  {
    "path": "src/routes/(app)/settings/client/ClientCreateDialog.svelte",
    "label": "round9 client name semantic submit guard",
    "sha256": "ca292670a485d3bd8ffdcf720e563a418c018b52e5d97e74c37dbe3a682fbda1",
    "count": 1,
    "before": "disabled={$mutex || name.length < 5 || name.length > 255}",
    "after": "disabled={$mutex || !validClientName(name)}",
    "approved_client_name_exception": "supported_client_name_validation"
  },
  {
    "path": "src/routes/(app)/settings/preferences/+page.svelte",
    "label": "round9 approved existing inline search callback cancellation User",
    "sha256": "0d77b163a1ec200ec7cf07951e475f413127b5a49f3f1ef5f137cf3c3baa8b7b",
    "count": 1,
    "before": "onSearch={async query => {\r\n\t\t\t\t\t\t\t\t\t\t\t\tconst q = new URLSearchParams();\r\n\t\t\t\t\t\t\t\t\t\t\t\tq.set(\"name\", query);\r\n\t\t\t\t\t\t\t\t\t\t\t\tq.set(\"limit\", \"100\");\r\n\r\n\t\t\t\t\t\t\t\t\t\t\t\tconst res = await API.User.Search(q);\r\n\t\t\t\t\t\t\t\t\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\t\t\t\t\t\t\t\t\treturn res.body;\r\n\t\t\t\t\t\t\t\t\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\t\t\t\t\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t\t\t\t\t\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t\t\t\t\t\t\t\t\t}}",
    "after": "onSearch={async (query, signal) => {\r\n\t\t\t\t\t\t\t\t\t\t\t\tconst q = new URLSearchParams();\r\n\t\t\t\t\t\t\t\t\t\t\t\tq.set(\"name\", query);\r\n\t\t\t\t\t\t\t\t\t\t\t\tq.set(\"limit\", \"100\");\r\n\r\n\t\t\t\t\t\t\t\t\t\t\t\tconst res = await API.User.Search(q, signal);\r\n\t\t\t\t\t\t\t\t\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\t\t\t\t\t\t\t\t\treturn res.body;\r\n\t\t\t\t\t\t\t\t\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\t\t\t\t\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t\t\t\t\t\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t\t\t\t\t\t\t\t\t}}",
    "approved_search_callback_exception": "existing_entity_search_cancellation"
  },
  {
    "path": "src/routes/(app)/settings/preferences/+page.svelte",
    "label": "round9 approved existing inline search callback cancellation Group",
    "sha256": "0d77b163a1ec200ec7cf07951e475f413127b5a49f3f1ef5f137cf3c3baa8b7b",
    "count": 1,
    "before": "onSearch={async query => {\r\n\t\t\t\t\t\t\t\t\t\t\tconst q = new URLSearchParams();\r\n\t\t\t\t\t\t\t\t\t\t\tq.set(\"name\", query);\r\n\t\t\t\t\t\t\t\t\t\t\tq.set(\"limit\", \"100\");\r\n\r\n\t\t\t\t\t\t\t\t\t\t\tconst res = await API.Group.Search(q);\r\n\t\t\t\t\t\t\t\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\t\t\t\t\t\t\t\treturn res.body;\r\n\t\t\t\t\t\t\t\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\t\t\t\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t\t\t\t\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t\t\t\t\t\t\t\t}}",
    "after": "onSearch={async (query, signal) => {\r\n\t\t\t\t\t\t\t\t\t\t\tconst q = new URLSearchParams();\r\n\t\t\t\t\t\t\t\t\t\t\tq.set(\"name\", query);\r\n\t\t\t\t\t\t\t\t\t\t\tq.set(\"limit\", \"100\");\r\n\r\n\t\t\t\t\t\t\t\t\t\t\tconst res = await API.Group.Search(q, signal);\r\n\t\t\t\t\t\t\t\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\t\t\t\t\t\t\t\treturn res.body;\r\n\t\t\t\t\t\t\t\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\t\t\t\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t\t\t\t\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t\t\t\t\t\t\t\t}}",
    "approved_search_callback_exception": "existing_entity_search_cancellation"
  },
  {
    "path": "src/routes/(app)/auth/login/+page.svelte",
    "label": "round10 login form lifetime and concrete flow operation fences",
    "sha256": "0f352782af49530504142a2e6dc79e621f9433a53e581710917b386bd279c8c5",
    "count": 1,
    "before": "<script lang=\"ts\">\r\n\timport { goto, replaceState } from \"$app/navigation\";\r\n\timport { page } from \"$app/state\";\r\n\timport { PUBLIC_SITE_NAME } from \"$env/static/public\";\r\n\timport { Constants, Kratos, Mutex } from \"$lib\";\r\n\timport { Button } from \"$lib/components/ui/button\";\r\n\timport * as Card from \"$lib/components/ui/card\";\r\n\timport * as Field from \"$lib/components/ui/field\";\r\n\timport { Input } from \"$lib/components/ui/input\";\r\n\timport Spinner from \"$lib/components/ui/spinner/spinner.svelte\";\r\n\timport { ErrorSessionAlreadyAvailable, type UINode } from \"$lib/Kratos\";\r\n\timport { Session } from \"$lib/stores\";\r\n\timport type {\r\n\t\tFrontendApiUpdateLoginFlowRequest,\r\n\t\tGenericError,\r\n\t\tLoginFlow,\r\n\t\tNeedsPrivilegedSessionError,\r\n\t\tSuccessfulNativeLogin,\r\n\t\tUiNode\r\n\t} from \"@ory/client\";\r\n\timport { AxiosError, type AxiosResponse } from \"axios\";\r\n\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { onMount } from \"svelte\";\r\n\timport { toast } from \"svelte-sonner\";\r\n\timport { fade } from \"svelte/transition\";\r\n\r\n\tconst id = $props.id();\r\n\tconst mutex = new Mutex();\r\n\tconst { pathname } = window.location;\r\n\r\n\tlet isMounted = $state(false);\r\n\tlet ref = $state<HTMLFormElement>();\r\n\tlet flow = $state<LoginFlow>();\r\n\tlet nodes = $state<{\r\n\t\tidentifier?: UINode;\r\n\t\tpassword?: UINode;\r\n\t\tcsrf_token?: UINode;\r\n\t\toidc?: UINode[];\r\n\t}>({});\r\n\r\n\tonMount(async () => {\r\n\t\tconst redirect = decodeURIComponent(page.url.searchParams.get(\"redirect\") ?? \"/\");\r\n\r\n\t\ttry {\r\n\t\t\tlet promise: Promise<AxiosResponse<LoginFlow>>;\r\n\t\t\tif (page.url.searchParams.has(\"flow\")) {\r\n\t\t\t\tpromise = Kratos.getLoginFlow({ id: page.url.searchParams.get(\"flow\")! });\r\n\t\t\t} else {\r\n\t\t\t\tpromise = Kratos.createBrowserLoginFlow({ returnTo: redirect, refresh: true });\r\n\t\t\t}\r\n\r\n\t\t\tconst res = await promise.catch(async (err: AxiosError<LoginFlow & GenericError>) => {\r\n\t\t\t\tif (\r\n\t\t\t\t\terr.status === StatusCodes.BAD_REQUEST &&\r\n\t\t\t\t\t<string>err.response?.data.error?.id === ErrorSessionAlreadyAvailable\r\n\t\t\t\t) {\r\n\t\t\t\t\tthrow ErrorSessionAlreadyAvailable;\r\n\t\t\t\t}\r\n\t\t\t\treturn err.status === StatusCodes.GONE && err.response?.data\r\n\t\t\t\t\t? Kratos.createBrowserLoginFlow({ returnTo: err.response.data.return_to || redirect })\r\n\t\t\t\t\t: err;\r\n\t\t\t});\r\n\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tif (res instanceof AxiosError) {\r\n\t\t\t\tif (res.response?.data.error?.message) {\r\n\t\t\t\t\ttoast.error(res.response.data.error.message, { duration: 10000 });\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} else if (res.status === StatusCodes.OK && res.data) {\r\n\t\t\t\tconst q = new URLSearchParams(window.location.search);\r\n\t\t\t\tq.set(\"flow\", res.data.id);\r\n\t\t\t\tq.delete(\"redirect\");\r\n\r\n\t\t\t\tflow = res.data;\r\n\t\t\t\treplaceState(window.location.pathname + q.toString(), page.state);\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (e === ErrorSessionAlreadyAvailable) goto(redirect);\r\n\t\t\telse if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t}\r\n\t\tisMounted = true;\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tflow?.ui.messages?.forEach(message => {\r\n\t\t\tswitch (message.type) {\r\n\t\t\t\tcase \"success\":\r\n\t\t\t\t\ttoast.success(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t\tcase \"info\":\r\n\t\t\t\t\ttoast.info(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t\tdefault:\r\n\t\t\t\t\ttoast.error(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t}\r\n\t\t});\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tif (flow?.ui.nodes) {\r\n\t\t\tnodes = flow.ui.nodes.reduce(\r\n\t\t\t\t(acc, node) => {\r\n\t\t\t\t\tconst name = <string>(<any>node.attributes).name;\r\n\t\t\t\t\tif ([\"csrf_token\", \"identifier\", \"password\"].includes(name)) {\r\n\t\t\t\t\t\t(<UiNode>acc[<keyof typeof nodes>name.replaceAll(\"traits.\", \"\")]) = node;\r\n\t\t\t\t\t} else if ([\"provider\", \"link\", \"unlink\"].includes(name)) {\r\n\t\t\t\t\t\t((<UiNode[]>acc[\"oidc\"]) ??= []).push(node);\r\n\t\t\t\t\t}\r\n\t\t\t\t\treturn acc;\r\n\t\t\t\t},\r\n\t\t\t\t{} as typeof nodes\r\n\t\t\t);\r\n\t\t}\r\n\t});\r\n\r\n\tconst updateFlow = async (method: \"password\" | \"oidc\", body: any) => {\r\n\t\tif (!flow || !ref) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tconst request: FrontendApiUpdateLoginFlowRequest = {\r\n\t\t\tflow: flow.id,\r\n\t\t\tupdateLoginFlowBody: {\r\n\t\t\t\tmethod: method,\r\n\t\t\t\tcsrf_token: ref.csrf_token.value,\r\n\t\t\t\t...body\r\n\t\t\t}\r\n\t\t};\r\n\r\n\t\ttry {\r\n\t\t\ttype T =\r\n\t\t\t\t| AxiosResponse<LoginFlow & SuccessfulNativeLogin & NeedsPrivilegedSessionError>\r\n\t\t\t\t| AxiosError<LoginFlow & SuccessfulNativeLogin & NeedsPrivilegedSessionError>;\r\n\t\t\tconst res: T = await Kratos.updateLoginFlow(request).catch(err => err);\r\n\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tconst data = res instanceof AxiosError ? res.response?.data : res.data;\r\n\t\t\tif (data?.session?.active) {\r\n\t\t\t\tawait Session.initialize(true);\r\n\t\t\t\tif (flow.return_to && !flow.return_to?.startsWith(window.location.origin)) {\r\n\t\t\t\t\twindow.location.href = flow.return_to;\r\n\t\t\t\t} else goto(flow.return_to || \"/\");\r\n\t\t\t} else if (data?.redirect_browser_to) {\r\n\t\t\t\twindow.location.href = data.redirect_browser_to;\r\n\t\t\t} else if (data?.ui) {\r\n\t\t\t\tflow = data;\r\n\t\t\t} else if (data?.error?.reason) {\r\n\t\t\t\ttoast.error(data.error.reason, { duration: 10000 });\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t}\r\n\t};\r\n\r\n\tconst onSubmit = async (ev: SubmitEvent) => {\r\n\t\tev.preventDefault();\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tif (!ref) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\t\t\treturn updateFlow(\"password\", {\r\n\t\t\t\tidentifier: ref.identifier.value,\r\n\t\t\t\tpassword: ref.password.value\r\n\t\t\t});\r\n\t\t});\r\n\t};\r\n\r\n\tconst onOidc = async (name: string, provider: string) =>\r\n\t\tmutex.exec(async () => updateFlow(\"oidc\", { [name]: provider }));\r\n</script>",
    "after": "<script lang=\"ts\">\r\n\timport { goto, replaceState } from \"$app/navigation\";\r\n\timport { page } from \"$app/state\";\r\n\timport { createAuthFormOwner } from \"$platform/auth-form-owner\";\r\n\timport { PUBLIC_SITE_NAME } from \"$env/static/public\";\r\n\timport { Constants, Kratos, Mutex } from \"$lib\";\r\n\timport { Button } from \"$lib/components/ui/button\";\r\n\timport * as Card from \"$lib/components/ui/card\";\r\n\timport * as Field from \"$lib/components/ui/field\";\r\n\timport { Input } from \"$lib/components/ui/input\";\r\n\timport Spinner from \"$lib/components/ui/spinner/spinner.svelte\";\r\n\timport { ErrorSessionAlreadyAvailable, type UINode } from \"$lib/Kratos\";\r\n\timport { Session } from \"$lib/stores\";\r\n\timport type {\r\n\t\tFrontendApiUpdateLoginFlowRequest,\r\n\t\tGenericError,\r\n\t\tLoginFlow,\r\n\t\tNeedsPrivilegedSessionError,\r\n\t\tSuccessfulNativeLogin,\r\n\t\tUiNode\r\n\t} from \"@ory/client\";\r\n\timport { AxiosError, type AxiosResponse } from \"axios\";\r\n\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { onMount } from \"svelte\";\r\n\timport { toast } from \"svelte-sonner\";\r\n\timport { fade } from \"svelte/transition\";\r\n\r\n\tconst id = $props.id();\r\n\tconst mutex = new Mutex();\r\n\tconst authFormOwner = createAuthFormOwner();\r\n\t$effect.pre(() => { page.url.href; authFormOwner.observe(window.location.href); });\r\n\r\n\tlet isMounted = $state(false);\r\n\tlet ref = $state<HTMLFormElement>();\r\n\tlet flow = $state<LoginFlow>();\r\n\tlet nodes = $state<{\r\n\t\tidentifier?: UINode;\r\n\t\tpassword?: UINode;\r\n\t\tcsrf_token?: UINode;\r\n\t\toidc?: UINode[];\r\n\t}>({});\r\n\r\n\tonMount(async () => {\r\n\t\tconst authIntent = authFormOwner.capture();\r\n\t\tconst redirect = decodeURIComponent(page.url.searchParams.get(\"redirect\") ?? \"/\");\r\n\r\n\t\ttry {\r\n\t\t\tlet promise: Promise<AxiosResponse<LoginFlow>>;\r\n\t\t\tif (page.url.searchParams.has(\"flow\")) {\r\n\t\t\t\tpromise = Kratos.getLoginFlow({ id: page.url.searchParams.get(\"flow\")! });\r\n\t\t\t} else {\r\n\t\t\t\tpromise = Kratos.createBrowserLoginFlow({ returnTo: redirect, refresh: true });\r\n\t\t\t}\r\n\r\n\t\t\tconst res = await promise.catch(async (err: AxiosError<LoginFlow & GenericError>) => {\r\n\t\t\t\tif (!authIntent.alive()) throw Error(\"The previous authentication form is no longer current\");\r\n\t\t\t\tif (\r\n\t\t\t\t\terr.status === StatusCodes.BAD_REQUEST &&\r\n\t\t\t\t\t<string>err.response?.data.error?.id === ErrorSessionAlreadyAvailable\r\n\t\t\t\t) {\r\n\t\t\t\t\tthrow ErrorSessionAlreadyAvailable;\r\n\t\t\t\t}\r\n\t\t\t\treturn err.status === StatusCodes.GONE && err.response?.data\r\n\t\t\t\t\t? Kratos.createBrowserLoginFlow({ returnTo: err.response.data.return_to || redirect })\r\n\t\t\t\t\t: err;\r\n\t\t\t});\r\n\t\t\tif (!authIntent.alive()) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tif (res instanceof AxiosError) {\r\n\t\t\t\tif (res.response?.data.error?.message) {\r\n\t\t\t\t\ttoast.error(res.response.data.error.message, { duration: 10000 });\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} else if (res.status === StatusCodes.OK && res.data) {\r\n\t\t\t\tconst q = new URLSearchParams(window.location.search);\r\n\t\t\t\tq.set(\"flow\", res.data.id);\r\n\t\t\t\tq.delete(\"redirect\");\r\n\r\n\t\t\t\tflow = res.data;\r\n\t\t\t\tauthIntent.replaceHistory(() => replaceState(window.location.pathname + q.toString(), page.state));\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (!authIntent.alive()) return;\r\n\t\t\tif (e === ErrorSessionAlreadyAvailable) goto(redirect);\r\n\t\t\telse if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t}\r\n\t\tif (!authIntent.alive()) return;\r\n\t\tisMounted = true;\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tflow?.ui.messages?.forEach(message => {\r\n\t\t\tswitch (message.type) {\r\n\t\t\t\tcase \"success\":\r\n\t\t\t\t\ttoast.success(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t\tcase \"info\":\r\n\t\t\t\t\ttoast.info(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t\tdefault:\r\n\t\t\t\t\ttoast.error(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t}\r\n\t\t});\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tif (flow?.ui.nodes) {\r\n\t\t\tnodes = flow.ui.nodes.reduce(\r\n\t\t\t\t(acc, node) => {\r\n\t\t\t\t\tconst name = <string>(<any>node.attributes).name;\r\n\t\t\t\t\tif ([\"csrf_token\", \"identifier\", \"password\"].includes(name)) {\r\n\t\t\t\t\t\t(<UiNode>acc[<keyof typeof nodes>name.replaceAll(\"traits.\", \"\")]) = node;\r\n\t\t\t\t\t} else if ([\"provider\", \"link\", \"unlink\"].includes(name)) {\r\n\t\t\t\t\t\t((<UiNode[]>acc[\"oidc\"]) ??= []).push(node);\r\n\t\t\t\t\t}\r\n\t\t\t\t\treturn acc;\r\n\t\t\t\t},\r\n\t\t\t\t{} as typeof nodes\r\n\t\t\t);\r\n\t\t}\r\n\t});\r\n\r\n\tconst updateFlow = async (method: \"password\" | \"oidc\", body: any) => {\r\n\t\tif (!flow || !ref) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tconst authIntent = authFormOwner.capture(flow.id, () => flow?.id);\r\n\t\tif (!authIntent.alive()) return;\r\n\t\tconst returnTo = flow.return_to;\r\n\r\n\t\tconst request: FrontendApiUpdateLoginFlowRequest = {\r\n\t\t\tflow: flow.id,\r\n\t\t\tupdateLoginFlowBody: {\r\n\t\t\t\tmethod: method,\r\n\t\t\t\tcsrf_token: ref.csrf_token.value,\r\n\t\t\t\t...body\r\n\t\t\t}\r\n\t\t};\r\n\r\n\t\ttry {\r\n\t\t\ttype T =\r\n\t\t\t\t| AxiosResponse<LoginFlow & SuccessfulNativeLogin & NeedsPrivilegedSessionError>\r\n\t\t\t\t| AxiosError<LoginFlow & SuccessfulNativeLogin & NeedsPrivilegedSessionError>;\r\n\t\t\tconst res: T = await Kratos.updateLoginFlow(request).catch(err => err);\r\n\t\t\tif (!authIntent.alive()) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tconst data = res instanceof AxiosError ? res.response?.data : res.data;\r\n\t\t\tif (data?.session?.active) {\r\n\t\t\t\tawait Session.initialize(true);\r\n\t\t\t\tif (!authIntent.alive()) return;\r\n\t\t\t\tif (returnTo && !returnTo?.startsWith(window.location.origin)) {\r\n\t\t\t\t\twindow.location.href = returnTo;\r\n\t\t\t\t} else goto(returnTo || \"/\");\r\n\t\t\t} else if (data?.redirect_browser_to) {\r\n\t\t\t\twindow.location.href = data.redirect_browser_to;\r\n\t\t\t} else if (data?.ui) {\r\n\t\t\t\tif (data.id !== request.flow) throw new Constants.Error.Error(\"Authentication flow changed\", \"Reload the current form; an unrelated flow was not attached.\");\r\n\t\t\t\tflow = data;\r\n\t\t\t} else if (data?.error?.reason) {\r\n\t\t\t\ttoast.error(data.error.reason, { duration: 10000 });\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (!authIntent.alive()) return;\r\n\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t}\r\n\t};\r\n\r\n\tconst onSubmit = async (ev: SubmitEvent) => {\r\n\t\tev.preventDefault();\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tif (!ref) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\t\t\treturn updateFlow(\"password\", {\r\n\t\t\t\tidentifier: ref.identifier.value,\r\n\t\t\t\tpassword: ref.password.value\r\n\t\t\t});\r\n\t\t});\r\n\t};\r\n\r\n\tconst onOidc = async (name: string, provider: string) =>\r\n\t\tmutex.exec(async () => updateFlow(\"oidc\", { [name]: provider }));\r\n</script>"
  },
  {
    "path": "src/routes/(app)/auth/register/+page.svelte",
    "label": "round10 register form lifetime and concrete flow operation fences",
    "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
    "count": 1,
    "before": "<script lang=\"ts\">\r\n\timport { goto, replaceState } from \"$app/navigation\";\r\n\timport { page } from \"$app/state\";\r\n\timport { PUBLIC_SITE_NAME } from \"$env/static/public\";\r\n\timport { Constants, Kratos, Mutex } from \"$lib\";\r\n\timport { Button } from \"$lib/components/ui/button\";\r\n\timport * as Card from \"$lib/components/ui/card\";\r\n\timport * as Field from \"$lib/components/ui/field\";\r\n\timport { Input } from \"$lib/components/ui/input\";\r\n\timport * as InputGroup from \"$lib/components/ui/input-group\";\r\n\timport Spinner from \"$lib/components/ui/spinner/spinner.svelte\";\r\n\timport * as Tooltip from \"$lib/components/ui/tooltip\";\r\n\timport { ErrorSessionAlreadyAvailable, type UINode } from \"$lib/Kratos\";\r\n\timport { Session } from \"$lib/stores\";\r\n\timport { InfoIcon } from \"@lucide/svelte\";\r\n\timport type {\r\n\t\tFrontendApiUpdateRegistrationFlowRequest,\r\n\t\tGenericError,\r\n\t\tNeedsPrivilegedSessionError,\r\n\t\tRegistrationFlow,\r\n\t\tSuccessfulNativeRegistration,\r\n\t\tUiNode\r\n\t} from \"@ory/client\";\r\n\timport { AxiosError, type AxiosResponse } from \"axios\";\r\n\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { onMount } from \"svelte\";\r\n\timport { toast } from \"svelte-sonner\";\r\n\timport { fade } from \"svelte/transition\";\r\n\r\n\tconst id = $props.id();\r\n\tconst mutex = new Mutex();\r\n\tconst { pathname } = window.location;\r\n\r\n\tlet isMounted = $state(false);\r\n\tlet ref = $state<HTMLFormElement>();\r\n\tlet flow = $state<RegistrationFlow>();\r\n\tlet nodes = $state<{\r\n\t\tidentifier?: UINode;\r\n\t\temail?: UINode;\r\n\t\tusername?: UINode;\r\n\t\tpassword?: UINode;\r\n\t\tcsrf_token?: UINode;\r\n\t\toidc?: UINode[];\r\n\t}>({});\r\n\r\n\tonMount(async () => {\r\n\t\tconst redirect = decodeURIComponent(page.url.searchParams.get(\"redirect\") ?? \"/\");\r\n\t\tif (await Session.isValid()) {\r\n\t\t\treturn goto(redirect);\r\n\t\t}\r\n\r\n\t\ttry {\r\n\t\t\tlet promise: Promise<AxiosResponse<RegistrationFlow>>;\r\n\t\t\tif (page.url.searchParams.has(\"flow\")) {\r\n\t\t\t\tpromise = Kratos.getRegistrationFlow({ id: page.url.searchParams.get(\"flow\")! });\r\n\t\t\t} else {\r\n\t\t\t\tpromise = Kratos.createBrowserRegistrationFlow({ returnTo: redirect });\r\n\t\t\t}\r\n\r\n\t\t\tconst res = await promise.catch(async (err: AxiosError<RegistrationFlow & GenericError>) => {\r\n\t\t\t\tif (\r\n\t\t\t\t\terr.status === StatusCodes.BAD_REQUEST &&\r\n\t\t\t\t\t<string>err.response?.data.error?.id === ErrorSessionAlreadyAvailable\r\n\t\t\t\t) {\r\n\t\t\t\t\tthrow ErrorSessionAlreadyAvailable;\r\n\t\t\t\t}\r\n\t\t\t\treturn err.status === StatusCodes.GONE && err.response?.data\r\n\t\t\t\t\t? Kratos.createBrowserRegistrationFlow({ returnTo: err.response.data.return_to || redirect })\r\n\t\t\t\t\t: err;\r\n\t\t\t});\r\n\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tif (res instanceof AxiosError) {\r\n\t\t\t\tif (res.response?.data.error?.message) {\r\n\t\t\t\t\ttoast.error(res.response.data.error.message, { duration: 10000 });\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} else if (res.status === StatusCodes.OK && res.data) {\r\n\t\t\t\tconst q = new URLSearchParams(window.location.search);\r\n\t\t\t\tq.set(\"flow\", res.data.id);\r\n\t\t\t\tq.delete(\"redirect\");\r\n\r\n\t\t\t\tflow = res.data;\r\n\t\t\t\treplaceState(window.location.pathname + q.toString(), page.state);\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (e === ErrorSessionAlreadyAvailable) goto(redirect);\r\n\t\t\telse if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t}\r\n\t\tisMounted = true;\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tflow?.ui.messages?.forEach(message => {\r\n\t\t\tswitch (message.type) {\r\n\t\t\t\tcase \"success\":\r\n\t\t\t\t\ttoast.success(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t\tcase \"info\":\r\n\t\t\t\t\ttoast.info(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t\tdefault:\r\n\t\t\t\t\ttoast.error(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t}\r\n\t\t});\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tif (flow?.ui.nodes) {\r\n\t\t\tnodes = flow.ui.nodes.reduce(\r\n\t\t\t\t(acc, node) => {\r\n\t\t\t\t\tconst name = <string>(<any>node.attributes).name;\r\n\t\t\t\t\tif ([\"csrf_token\", \"traits.email\", \"traits.username\", \"password\"].includes(name)) {\r\n\t\t\t\t\t\t(<UiNode>acc[<keyof typeof nodes>name.replaceAll(\"traits.\", \"\")]) = node;\r\n\t\t\t\t\t} else if ([\"provider\", \"link\", \"unlink\"].includes(name)) {\r\n\t\t\t\t\t\t((<UiNode[]>acc[\"oidc\"]) ??= []).push(node);\r\n\t\t\t\t\t}\r\n\t\t\t\t\treturn acc;\r\n\t\t\t\t},\r\n\t\t\t\t{} as typeof nodes\r\n\t\t\t);\r\n\t\t}\r\n\t});\r\n\r\n\tconst updateFlow = async (method: \"password\" | \"oidc\", body: any) => {\r\n\t\tif (!flow || !ref) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tconst request: FrontendApiUpdateRegistrationFlowRequest = {\r\n\t\t\tflow: flow.id,\r\n\t\t\tupdateRegistrationFlowBody: {\r\n\t\t\t\tmethod: method,\r\n\t\t\t\tcsrf_token: ref.csrf_token.value,\r\n\t\t\t\t...body\r\n\t\t\t}\r\n\t\t};\r\n\r\n\t\ttry {\r\n\t\t\ttype T =\r\n\t\t\t\t| AxiosResponse<RegistrationFlow & SuccessfulNativeRegistration & NeedsPrivilegedSessionError>\r\n\t\t\t\t| AxiosError<RegistrationFlow & SuccessfulNativeRegistration & NeedsPrivilegedSessionError>;\r\n\t\t\tconst res: T = await Kratos.updateRegistrationFlow(request).catch(err => err);\r\n\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tconst data = res instanceof AxiosError ? res.response?.data : res.data;\r\n\t\t\tif (data?.session?.active) {\r\n\t\t\t\tawait Session.initialize(true);\r\n\t\t\t\tif (flow.return_to && !flow.return_to?.startsWith(window.location.origin)) {\r\n\t\t\t\t\twindow.location.href = flow.return_to;\r\n\t\t\t\t} else goto(flow.return_to || \"/\");\r\n\t\t\t} else if (data?.redirect_browser_to) {\r\n\t\t\t\twindow.location.href = data.redirect_browser_to;\r\n\t\t\t} else if (data?.ui) {\r\n\t\t\t\tflow = data;\r\n\t\t\t} else if (data?.error?.reason) {\r\n\t\t\t\ttoast.error(data.error.reason, { duration: 10000 });\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t}\r\n\t};\r\n\r\n\tconst onSubmit = async (ev: SubmitEvent) => {\r\n\t\tev.preventDefault();\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tif (!ref) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\t\t\treturn updateFlow(\"password\", {\r\n\t\t\t\tpassword: ref.password.value,\r\n\t\t\t\ttraits: {\r\n\t\t\t\t\temail: ref.email.value,\r\n\t\t\t\t\tusername: ref.username.value\r\n\t\t\t\t}\r\n\t\t\t});\r\n\t\t});\r\n\t};\r\n\r\n\tconst onOidc = async (name: string, provider: string) =>\r\n\t\tmutex.exec(async () => updateFlow(\"oidc\", { [name]: provider }));\r\n</script>",
    "after": "<script lang=\"ts\">\r\n\timport { goto, replaceState } from \"$app/navigation\";\r\n\timport { page } from \"$app/state\";\r\n\timport { createAuthFormOwner } from \"$platform/auth-form-owner\";\r\n\timport { PUBLIC_SITE_NAME } from \"$env/static/public\";\r\n\timport { Constants, Kratos, Mutex } from \"$lib\";\r\n\timport { Button } from \"$lib/components/ui/button\";\r\n\timport * as Card from \"$lib/components/ui/card\";\r\n\timport * as Field from \"$lib/components/ui/field\";\r\n\timport { Input } from \"$lib/components/ui/input\";\r\n\timport * as InputGroup from \"$lib/components/ui/input-group\";\r\n\timport Spinner from \"$lib/components/ui/spinner/spinner.svelte\";\r\n\timport * as Tooltip from \"$lib/components/ui/tooltip\";\r\n\timport { ErrorSessionAlreadyAvailable, type UINode } from \"$lib/Kratos\";\r\n\timport { Session } from \"$lib/stores\";\r\n\timport { InfoIcon } from \"@lucide/svelte\";\r\n\timport type {\r\n\t\tFrontendApiUpdateRegistrationFlowRequest,\r\n\t\tGenericError,\r\n\t\tNeedsPrivilegedSessionError,\r\n\t\tRegistrationFlow,\r\n\t\tSuccessfulNativeRegistration,\r\n\t\tUiNode\r\n\t} from \"@ory/client\";\r\n\timport { AxiosError, type AxiosResponse } from \"axios\";\r\n\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { onMount } from \"svelte\";\r\n\timport { toast } from \"svelte-sonner\";\r\n\timport { fade } from \"svelte/transition\";\r\n\r\n\tconst id = $props.id();\r\n\tconst mutex = new Mutex();\r\n\tconst authFormOwner = createAuthFormOwner();\r\n\t$effect.pre(() => { page.url.href; authFormOwner.observe(window.location.href); });\r\n\r\n\tlet isMounted = $state(false);\r\n\tlet ref = $state<HTMLFormElement>();\r\n\tlet flow = $state<RegistrationFlow>();\r\n\tlet nodes = $state<{\r\n\t\tidentifier?: UINode;\r\n\t\temail?: UINode;\r\n\t\tusername?: UINode;\r\n\t\tpassword?: UINode;\r\n\t\tcsrf_token?: UINode;\r\n\t\toidc?: UINode[];\r\n\t}>({});\r\n\r\n\tonMount(async () => {\r\n\t\tconst authIntent = authFormOwner.capture();\r\n\t\tconst redirect = decodeURIComponent(page.url.searchParams.get(\"redirect\") ?? \"/\");\r\n\t\tconst signedIn = await Session.isValid();\r\n\t\tif (!authIntent.alive()) return;\r\n\t\tif (signedIn) {\r\n\t\t\treturn goto(redirect);\r\n\t\t}\r\n\r\n\t\ttry {\r\n\t\t\tlet promise: Promise<AxiosResponse<RegistrationFlow>>;\r\n\t\t\tif (page.url.searchParams.has(\"flow\")) {\r\n\t\t\t\tpromise = Kratos.getRegistrationFlow({ id: page.url.searchParams.get(\"flow\")! });\r\n\t\t\t} else {\r\n\t\t\t\tpromise = Kratos.createBrowserRegistrationFlow({ returnTo: redirect });\r\n\t\t\t}\r\n\r\n\t\t\tconst res = await promise.catch(async (err: AxiosError<RegistrationFlow & GenericError>) => {\r\n\t\t\t\tif (!authIntent.alive()) throw Error(\"The previous authentication form is no longer current\");\r\n\t\t\t\tif (\r\n\t\t\t\t\terr.status === StatusCodes.BAD_REQUEST &&\r\n\t\t\t\t\t<string>err.response?.data.error?.id === ErrorSessionAlreadyAvailable\r\n\t\t\t\t) {\r\n\t\t\t\t\tthrow ErrorSessionAlreadyAvailable;\r\n\t\t\t\t}\r\n\t\t\t\treturn err.status === StatusCodes.GONE && err.response?.data\r\n\t\t\t\t\t? Kratos.createBrowserRegistrationFlow({ returnTo: err.response.data.return_to || redirect })\r\n\t\t\t\t\t: err;\r\n\t\t\t});\r\n\t\t\tif (!authIntent.alive()) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tif (res instanceof AxiosError) {\r\n\t\t\t\tif (res.response?.data.error?.message) {\r\n\t\t\t\t\ttoast.error(res.response.data.error.message, { duration: 10000 });\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} else if (res.status === StatusCodes.OK && res.data) {\r\n\t\t\t\tconst q = new URLSearchParams(window.location.search);\r\n\t\t\t\tq.set(\"flow\", res.data.id);\r\n\t\t\t\tq.delete(\"redirect\");\r\n\r\n\t\t\t\tflow = res.data;\r\n\t\t\t\tauthIntent.replaceHistory(() => replaceState(window.location.pathname + q.toString(), page.state));\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (!authIntent.alive()) return;\r\n\t\t\tif (e === ErrorSessionAlreadyAvailable) goto(redirect);\r\n\t\t\telse if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t}\r\n\t\tif (!authIntent.alive()) return;\r\n\t\tisMounted = true;\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tflow?.ui.messages?.forEach(message => {\r\n\t\t\tswitch (message.type) {\r\n\t\t\t\tcase \"success\":\r\n\t\t\t\t\ttoast.success(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t\tcase \"info\":\r\n\t\t\t\t\ttoast.info(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t\tdefault:\r\n\t\t\t\t\ttoast.error(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t}\r\n\t\t});\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tif (flow?.ui.nodes) {\r\n\t\t\tnodes = flow.ui.nodes.reduce(\r\n\t\t\t\t(acc, node) => {\r\n\t\t\t\t\tconst name = <string>(<any>node.attributes).name;\r\n\t\t\t\t\tif ([\"csrf_token\", \"traits.email\", \"traits.username\", \"password\"].includes(name)) {\r\n\t\t\t\t\t\t(<UiNode>acc[<keyof typeof nodes>name.replaceAll(\"traits.\", \"\")]) = node;\r\n\t\t\t\t\t} else if ([\"provider\", \"link\", \"unlink\"].includes(name)) {\r\n\t\t\t\t\t\t((<UiNode[]>acc[\"oidc\"]) ??= []).push(node);\r\n\t\t\t\t\t}\r\n\t\t\t\t\treturn acc;\r\n\t\t\t\t},\r\n\t\t\t\t{} as typeof nodes\r\n\t\t\t);\r\n\t\t}\r\n\t});\r\n\r\n\tconst updateFlow = async (method: \"password\" | \"oidc\", body: any) => {\r\n\t\tif (!flow || !ref) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tconst authIntent = authFormOwner.capture(flow.id, () => flow?.id);\r\n\t\tif (!authIntent.alive()) return;\r\n\t\tconst returnTo = flow.return_to;\r\n\r\n\t\tconst request: FrontendApiUpdateRegistrationFlowRequest = {\r\n\t\t\tflow: flow.id,\r\n\t\t\tupdateRegistrationFlowBody: {\r\n\t\t\t\tmethod: method,\r\n\t\t\t\tcsrf_token: ref.csrf_token.value,\r\n\t\t\t\t...body\r\n\t\t\t}\r\n\t\t};\r\n\r\n\t\ttry {\r\n\t\t\ttype T =\r\n\t\t\t\t| AxiosResponse<RegistrationFlow & SuccessfulNativeRegistration & NeedsPrivilegedSessionError>\r\n\t\t\t\t| AxiosError<RegistrationFlow & SuccessfulNativeRegistration & NeedsPrivilegedSessionError>;\r\n\t\t\tconst res: T = await Kratos.updateRegistrationFlow(request).catch(err => err);\r\n\t\t\tif (!authIntent.alive()) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tconst data = res instanceof AxiosError ? res.response?.data : res.data;\r\n\t\t\tif (data?.session?.active) {\r\n\t\t\t\tawait Session.initialize(true);\r\n\t\t\t\tif (!authIntent.alive()) return;\r\n\t\t\t\tif (returnTo && !returnTo?.startsWith(window.location.origin)) {\r\n\t\t\t\t\twindow.location.href = returnTo;\r\n\t\t\t\t} else goto(returnTo || \"/\");\r\n\t\t\t} else if (data?.redirect_browser_to) {\r\n\t\t\t\twindow.location.href = data.redirect_browser_to;\r\n\t\t\t} else if (data?.ui) {\r\n\t\t\t\tif (data.id !== request.flow) throw new Constants.Error.Error(\"Authentication flow changed\", \"Reload the current form; an unrelated flow was not attached.\");\r\n\t\t\t\tflow = data;\r\n\t\t\t} else if (data?.error?.reason) {\r\n\t\t\t\ttoast.error(data.error.reason, { duration: 10000 });\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (!authIntent.alive()) return;\r\n\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t}\r\n\t};\r\n\r\n\tconst onSubmit = async (ev: SubmitEvent) => {\r\n\t\tev.preventDefault();\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tif (!ref) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\t\t\treturn updateFlow(\"password\", {\r\n\t\t\t\tpassword: ref.password.value,\r\n\t\t\t\ttraits: {\r\n\t\t\t\t\temail: ref.email.value,\r\n\t\t\t\t\tusername: ref.username.value\r\n\t\t\t\t}\r\n\t\t\t});\r\n\t\t});\r\n\t};\r\n\r\n\tconst onOidc = async (name: string, provider: string) =>\r\n\t\tmutex.exec(async () => updateFlow(\"oidc\", { [name]: provider }));\r\n</script>"
  },
  {
    "path": "src/routes/(app)/auth/verification/+page.svelte",
    "label": "round10 verification form lifetime and concrete flow operation fences",
    "sha256": "0a2ea92f2157a12e8dc56b813b5fc6ec7e909459fedab9447343b99aefcfe702",
    "count": 1,
    "before": "<script lang=\"ts\">\r\n\timport { goto, replaceState } from \"$app/navigation\";\r\n\timport { page } from \"$app/state\";\r\n\timport { PUBLIC_SITE_NAME } from \"$env/static/public\";\r\n\timport { Constants, Kratos, Mutex } from \"$lib\";\r\n\timport { Button } from \"$lib/components/ui/button\";\r\n\timport * as Card from \"$lib/components/ui/card\";\r\n\timport * as Empty from \"$lib/components/ui/empty/index.js\";\r\n\timport * as Field from \"$lib/components/ui/field\";\r\n\timport { Input } from \"$lib/components/ui/input\";\r\n\timport Spinner from \"$lib/components/ui/spinner/spinner.svelte\";\r\n\timport { ErrorSessionAlreadyAvailable, type UINode } from \"$lib/Kratos\";\r\n\timport { Session } from \"$lib/stores\";\r\n\timport type {\r\n\t\tFrontendApiUpdateVerificationFlowRequest,\r\n\t\tGenericError,\r\n\t\tNeedsPrivilegedSessionError,\r\n\t\tUiNode,\r\n\t\tVerificationFlow\r\n\t} from \"@ory/client\";\r\n\timport { AxiosError, type AxiosResponse } from \"axios\";\r\n\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { onMount } from \"svelte\";\r\n\timport { toast } from \"svelte-sonner\";\r\n\timport { fade } from \"svelte/transition\";\r\n\tconst id = $props.id();\r\n\tconst mutex = new Mutex();\r\n\tconst { pathname } = window.location;\r\n\r\n\tlet isMounted = $state(false);\r\n\tlet ref = $state<HTMLFormElement>();\r\n\tlet flow = $state<VerificationFlow>();\r\n\tlet nodes = $state<{\r\n\t\tcsrf_token?: UINode;\r\n\t\temail?: UINode;\r\n\t}>({});\r\n\r\n\tonMount(async () => {\r\n\t\tconst redirect = decodeURIComponent(page.url.searchParams.get(\"redirect\") ?? \"/\");\r\n\r\n\t\ttry {\r\n\t\t\tlet promise: Promise<AxiosResponse<VerificationFlow>>;\r\n\t\t\tif (page.url.searchParams.has(\"flow\")) {\r\n\t\t\t\tpromise = Kratos.getVerificationFlow({ id: page.url.searchParams.get(\"flow\")! });\r\n\t\t\t} else {\r\n\t\t\t\tpromise = Kratos.createBrowserVerificationFlow({ returnTo: redirect });\r\n\t\t\t}\r\n\r\n\t\t\tconst res = await promise.catch(async (err: AxiosError<VerificationFlow & GenericError>) => {\r\n\t\t\t\treturn err.status === StatusCodes.GONE && err.response?.data\r\n\t\t\t\t\t? Kratos.createBrowserVerificationFlow({ returnTo: err.response.data.return_to || redirect })\r\n\t\t\t\t\t: err;\r\n\t\t\t});\r\n\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tif (res instanceof AxiosError) {\r\n\t\t\t\tif (res.response?.data.error?.message) {\r\n\t\t\t\t\ttoast.error(res.response.data.error.message, { duration: 10000 });\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} else if (res.status === StatusCodes.OK && res.data) {\r\n\t\t\t\tconst q = new URLSearchParams(window.location.search);\r\n\t\t\t\tq.set(\"flow\", res.data.id);\r\n\t\t\t\tq.delete(\"redirect\");\r\n\r\n\t\t\t\tflow = res.data;\r\n\t\t\t\treplaceState(window.location.pathname + q.toString(), page.state);\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (e === ErrorSessionAlreadyAvailable) goto(redirect);\r\n\t\t\telse if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t}\r\n\t\tisMounted = true;\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tflow?.ui.messages?.forEach(message => {\r\n\t\t\tswitch (message.type) {\r\n\t\t\t\tcase \"success\":\r\n\t\t\t\t\ttoast.success(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t\tcase \"info\":\r\n\t\t\t\t\ttoast.info(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t\tdefault:\r\n\t\t\t\t\ttoast.error(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t}\r\n\t\t});\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tif (flow?.ui.nodes) {\r\n\t\t\tnodes = flow.ui.nodes.reduce(\r\n\t\t\t\t(acc, node) => {\r\n\t\t\t\t\tconst name = <string>(<any>node.attributes).name;\r\n\t\t\t\t\tif ([\"csrf_token\", \"email\"].includes(name)) {\r\n\t\t\t\t\t\t(<UiNode>acc[<keyof typeof nodes>name.replaceAll(\"traits.\", \"\")]) = node;\r\n\t\t\t\t\t}\r\n\t\t\t\t\treturn acc;\r\n\t\t\t\t},\r\n\t\t\t\t{} as typeof nodes\r\n\t\t\t);\r\n\t\t}\r\n\t});\r\n\r\n\tconst updateFlow = async (method: \"link\", body: any) => {\r\n\t\tif (!flow || !ref) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tconst request: FrontendApiUpdateVerificationFlowRequest = {\r\n\t\t\tflow: flow.id,\r\n\t\t\tupdateVerificationFlowBody: {\r\n\t\t\t\tmethod: method,\r\n\t\t\t\tcsrf_token: ref.csrf_token.value,\r\n\t\t\t\t...body\r\n\t\t\t}\r\n\t\t};\r\n\r\n\t\ttry {\r\n\t\t\ttype T =\r\n\t\t\t\t| AxiosResponse<VerificationFlow & NeedsPrivilegedSessionError>\r\n\t\t\t\t| AxiosError<VerificationFlow & NeedsPrivilegedSessionError>;\r\n\t\t\tconst res: T = await Kratos.updateVerificationFlow(request).catch(err => err);\r\n\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tconst data = res instanceof AxiosError ? res.response?.data : res.data;\r\n\t\t\tif (data?.redirect_browser_to) {\r\n\t\t\t\twindow.location.href = data.redirect_browser_to;\r\n\t\t\t} else if (data?.ui) {\r\n\t\t\t\tflow = data;\r\n\t\t\t} else if (data?.error?.reason) {\r\n\t\t\t\ttoast.error(data.error.reason, { duration: 10000 });\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t}\r\n\t};\r\n\r\n\tconst onSubmit = async (ev: SubmitEvent) => {\r\n\t\tev.preventDefault();\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tif (!ref) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\t\t\treturn updateFlow(\"link\", {\r\n\t\t\t\temail: ref.email.value\r\n\t\t\t});\r\n\t\t});\r\n\t};\r\n</script>",
    "after": "<script lang=\"ts\">\r\n\timport { goto, replaceState } from \"$app/navigation\";\r\n\timport { page } from \"$app/state\";\r\n\timport { createAuthFormOwner } from \"$platform/auth-form-owner\";\r\n\timport { PUBLIC_SITE_NAME } from \"$env/static/public\";\r\n\timport { Constants, Kratos, Mutex } from \"$lib\";\r\n\timport { Button } from \"$lib/components/ui/button\";\r\n\timport * as Card from \"$lib/components/ui/card\";\r\n\timport * as Empty from \"$lib/components/ui/empty/index.js\";\r\n\timport * as Field from \"$lib/components/ui/field\";\r\n\timport { Input } from \"$lib/components/ui/input\";\r\n\timport Spinner from \"$lib/components/ui/spinner/spinner.svelte\";\r\n\timport { ErrorSessionAlreadyAvailable, type UINode } from \"$lib/Kratos\";\r\n\timport { Session } from \"$lib/stores\";\r\n\timport type {\r\n\t\tFrontendApiUpdateVerificationFlowRequest,\r\n\t\tGenericError,\r\n\t\tNeedsPrivilegedSessionError,\r\n\t\tUiNode,\r\n\t\tVerificationFlow\r\n\t} from \"@ory/client\";\r\n\timport { AxiosError, type AxiosResponse } from \"axios\";\r\n\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { onMount } from \"svelte\";\r\n\timport { toast } from \"svelte-sonner\";\r\n\timport { fade } from \"svelte/transition\";\r\n\tconst id = $props.id();\r\n\tconst mutex = new Mutex();\r\n\tconst authFormOwner = createAuthFormOwner();\r\n\t$effect.pre(() => { page.url.href; authFormOwner.observe(window.location.href); });\r\n\r\n\tlet isMounted = $state(false);\r\n\tlet ref = $state<HTMLFormElement>();\r\n\tlet flow = $state<VerificationFlow>();\r\n\tlet nodes = $state<{\r\n\t\tcsrf_token?: UINode;\r\n\t\temail?: UINode;\r\n\t}>({});\r\n\r\n\tonMount(async () => {\r\n\t\tconst authIntent = authFormOwner.capture();\r\n\t\tconst redirect = decodeURIComponent(page.url.searchParams.get(\"redirect\") ?? \"/\");\r\n\r\n\t\ttry {\r\n\t\t\tlet promise: Promise<AxiosResponse<VerificationFlow>>;\r\n\t\t\tif (page.url.searchParams.has(\"flow\")) {\r\n\t\t\t\tpromise = Kratos.getVerificationFlow({ id: page.url.searchParams.get(\"flow\")! });\r\n\t\t\t} else {\r\n\t\t\t\tpromise = Kratos.createBrowserVerificationFlow({ returnTo: redirect });\r\n\t\t\t}\r\n\r\n\t\t\tconst res = await promise.catch(async (err: AxiosError<VerificationFlow & GenericError>) => {\r\n\t\t\t\tif (!authIntent.alive()) throw Error(\"The previous authentication form is no longer current\");\r\n\t\t\t\treturn err.status === StatusCodes.GONE && err.response?.data\r\n\t\t\t\t\t? Kratos.createBrowserVerificationFlow({ returnTo: err.response.data.return_to || redirect })\r\n\t\t\t\t\t: err;\r\n\t\t\t});\r\n\t\t\tif (!authIntent.alive()) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tif (res instanceof AxiosError) {\r\n\t\t\t\tif (res.response?.data.error?.message) {\r\n\t\t\t\t\ttoast.error(res.response.data.error.message, { duration: 10000 });\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} else if (res.status === StatusCodes.OK && res.data) {\r\n\t\t\t\tconst q = new URLSearchParams(window.location.search);\r\n\t\t\t\tq.set(\"flow\", res.data.id);\r\n\t\t\t\tq.delete(\"redirect\");\r\n\r\n\t\t\t\tflow = res.data;\r\n\t\t\t\tauthIntent.replaceHistory(() => replaceState(window.location.pathname + q.toString(), page.state));\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (!authIntent.alive()) return;\r\n\t\t\tif (e === ErrorSessionAlreadyAvailable) goto(redirect);\r\n\t\t\telse if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t}\r\n\t\tif (!authIntent.alive()) return;\r\n\t\tisMounted = true;\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tflow?.ui.messages?.forEach(message => {\r\n\t\t\tswitch (message.type) {\r\n\t\t\t\tcase \"success\":\r\n\t\t\t\t\ttoast.success(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t\tcase \"info\":\r\n\t\t\t\t\ttoast.info(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t\tdefault:\r\n\t\t\t\t\ttoast.error(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t}\r\n\t\t});\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tif (flow?.ui.nodes) {\r\n\t\t\tnodes = flow.ui.nodes.reduce(\r\n\t\t\t\t(acc, node) => {\r\n\t\t\t\t\tconst name = <string>(<any>node.attributes).name;\r\n\t\t\t\t\tif ([\"csrf_token\", \"email\"].includes(name)) {\r\n\t\t\t\t\t\t(<UiNode>acc[<keyof typeof nodes>name.replaceAll(\"traits.\", \"\")]) = node;\r\n\t\t\t\t\t}\r\n\t\t\t\t\treturn acc;\r\n\t\t\t\t},\r\n\t\t\t\t{} as typeof nodes\r\n\t\t\t);\r\n\t\t}\r\n\t});\r\n\r\n\tconst updateFlow = async (method: \"link\", body: any) => {\r\n\t\tif (!flow || !ref) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tconst authIntent = authFormOwner.capture(flow.id, () => flow?.id);\r\n\t\tif (!authIntent.alive()) return;\r\n\t\tconst returnTo = flow.return_to;\r\n\r\n\t\tconst request: FrontendApiUpdateVerificationFlowRequest = {\r\n\t\t\tflow: flow.id,\r\n\t\t\tupdateVerificationFlowBody: {\r\n\t\t\t\tmethod: method,\r\n\t\t\t\tcsrf_token: ref.csrf_token.value,\r\n\t\t\t\t...body\r\n\t\t\t}\r\n\t\t};\r\n\r\n\t\ttry {\r\n\t\t\ttype T =\r\n\t\t\t\t| AxiosResponse<VerificationFlow & NeedsPrivilegedSessionError>\r\n\t\t\t\t| AxiosError<VerificationFlow & NeedsPrivilegedSessionError>;\r\n\t\t\tconst res: T = await Kratos.updateVerificationFlow(request).catch(err => err);\r\n\t\t\tif (!authIntent.alive()) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tconst data = res instanceof AxiosError ? res.response?.data : res.data;\r\n\t\t\tif (data?.redirect_browser_to) {\r\n\t\t\t\twindow.location.href = data.redirect_browser_to;\r\n\t\t\t} else if (data?.ui) {\r\n\t\t\t\tif (data.id !== request.flow) throw new Constants.Error.Error(\"Authentication flow changed\", \"Reload the current form; an unrelated flow was not attached.\");\r\n\t\t\t\tflow = data;\r\n\t\t\t} else if (data?.error?.reason) {\r\n\t\t\t\ttoast.error(data.error.reason, { duration: 10000 });\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (!authIntent.alive()) return;\r\n\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t}\r\n\t};\r\n\r\n\tconst onSubmit = async (ev: SubmitEvent) => {\r\n\t\tev.preventDefault();\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tif (!ref) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\t\t\treturn updateFlow(\"link\", {\r\n\t\t\t\temail: ref.email.value\r\n\t\t\t});\r\n\t\t});\r\n\t};\r\n</script>"
  },
  {
    "path": "src/routes/(app)/settings/account/+page.svelte",
    "label": "round10 account form lifetime and concrete flow operation fences",
    "sha256": "ff92851ff500757d698401a06c0f213092bbb853219bd7992546cae427d3646c",
    "count": 1,
    "before": "<script lang=\"ts\">\r\n\timport { goto, replaceState } from \"$app/navigation\";\r\n\timport { page } from \"$app/state\";\r\n\timport { PUBLIC_SITE_NAME } from \"$env/static/public\";\r\n\timport { Constants, Kratos, Mutex } from \"$lib\";\r\n\timport ErrorComponent from \"$lib/components/ErrorComponent.svelte\";\r\n\timport { Button } from \"$lib/components/ui/button\";\r\n\timport * as Empty from \"$lib/components/ui/empty\";\r\n\timport * as Field from \"$lib/components/ui/field\";\r\n\timport * as InputGroup from \"$lib/components/ui/input-group\";\r\n\timport Input from \"$lib/components/ui/input/input.svelte\";\r\n\timport { Separator } from \"$lib/components/ui/separator\";\r\n\timport { Spinner } from \"$lib/components/ui/spinner\";\r\n\timport * as Tooltip from \"$lib/components/ui/tooltip\";\r\n\timport type { UINode } from \"$lib/Kratos\";\r\n\timport { InfoIcon, Link, Unlink } from \"@lucide/svelte\";\r\n\timport { Session } from \"$lib/stores\";\r\n\timport type {\r\n\t\tFrontendApiUpdateSettingsFlowRequest,\r\n\t\tGenericError,\r\n\t\tNeedsPrivilegedSessionError,\r\n\t\tSettingsFlow,\r\n\t\tUiNode\r\n\t} from \"@ory/client\";\r\n\timport { AxiosError, type AxiosResponse } from \"axios\";\r\n\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { onMount } from \"svelte\";\r\n\timport { toast } from \"svelte-sonner\";\r\n\r\n\tconst { pathname } = window.location;\r\n\tconst id = $props.id();\r\n\tconst mutex = new Mutex();\r\n\tlet mutexState = $state<string>();\r\n\r\n\tlet isMounted = $state(false);\r\n\tlet error = $state<Constants.Error.Error>();\r\n\tlet csrfRef = $state<HTMLInputElement>();\r\n\tlet profileFormRef = $state<HTMLFormElement>();\r\n\tlet passwordFormRef = $state<HTMLFormElement>();\r\n\tlet flow = $state<SettingsFlow>();\r\n\tlet nodes = $state<{\r\n\t\temail?: UINode;\r\n\t\tusername?: UINode;\r\n\t\tpassword?: UINode;\r\n\t\tcsrf_token?: UINode;\r\n\t\toidc?: UINode[];\r\n\t}>({});\r\n\r\n\tonMount(async () => {\r\n\t\ttry {\r\n\t\t\tlet promise: Promise<AxiosResponse<SettingsFlow>>;\r\n\t\t\tif (page.url.searchParams.has(\"flow\")) {\r\n\t\t\t\tpromise = Kratos.getSettingsFlow({ id: page.url.searchParams.get(\"flow\")! });\r\n\t\t\t} else {\r\n\t\t\t\tpromise = Kratos.createBrowserSettingsFlow();\r\n\t\t\t}\r\n\r\n\t\t\tconst res = await promise.catch(async (err: AxiosError<SettingsFlow & GenericError>) => {\r\n\t\t\t\tif (err.status === StatusCodes.UNAUTHORIZED) {\r\n\t\t\t\t\tgoto(`/auth/login?redirect=${encodeURIComponent(window.location.pathname + window.location.search)}`);\r\n\t\t\t\t}\r\n\t\t\t\treturn err.status === StatusCodes.GONE && err.response?.data\r\n\t\t\t\t\t? Kratos.createBrowserSettingsFlow({ returnTo: err.response.data.return_to || window.location.pathname })\r\n\t\t\t\t\t: err;\r\n\t\t\t});\r\n\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tif (res instanceof AxiosError) {\r\n\t\t\t\tif (res.response?.data.error?.message) {\r\n\t\t\t\t\ttoast.error(res.response.data.error.message, { duration: 10000 });\r\n\t\t\t\t}\r\n\t\t\t\tthrow Constants.Error.Unknown;\r\n\t\t\t} else if (res.status === StatusCodes.OK && res.data) {\r\n\t\t\t\tconst q = new URLSearchParams(window.location.search);\r\n\t\t\t\tq.set(\"flow\", res.data.id);\r\n\t\t\t\tq.delete(\"redirect\");\r\n\r\n\t\t\t\tflow = res.data;\r\n\t\t\t\treplaceState(window.location.pathname + q.toString(), page.state);\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (e instanceof Constants.Error.Error) error = e;\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\terror = Constants.Error.Unknown;\r\n\t\t\t}\r\n\t\t}\r\n\t\tisMounted = true;\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tflow?.ui.messages?.forEach(message => {\r\n\t\t\tswitch (message.type) {\r\n\t\t\t\tcase \"success\":\r\n\t\t\t\t\ttoast.success(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t\tcase \"info\":\r\n\t\t\t\t\ttoast.info(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t\tdefault:\r\n\t\t\t\t\ttoast.error(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t}\r\n\t\t});\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tif (flow?.ui.nodes) {\r\n\t\t\tnodes = flow.ui.nodes.reduce(\r\n\t\t\t\t(acc, node) => {\r\n\t\t\t\t\tconst name = <string>(<any>node.attributes).name;\r\n\t\t\t\t\tif ([\"csrf_token\", \"traits.email\", \"traits.username\", \"password\"].includes(name)) {\r\n\t\t\t\t\t\t(<UiNode>acc[<keyof typeof nodes>name.replaceAll(\"traits.\", \"\")]) = node;\r\n\t\t\t\t\t} else if ([\"provider\", \"link\", \"unlink\"].includes(name)) {\r\n\t\t\t\t\t\t((<UiNode[]>acc[\"oidc\"]) ??= []).push(node);\r\n\t\t\t\t\t}\r\n\t\t\t\t\treturn acc;\r\n\t\t\t\t},\r\n\t\t\t\t{} as typeof nodes\r\n\t\t\t);\r\n\t\t}\r\n\t});\r\n\r\n\tconst updateFlow = async (method: \"profile\" | \"password\" | \"oidc\", body: any, state?: string) => {\r\n\t\tif (!flow || !csrfRef) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tconst request: FrontendApiUpdateSettingsFlowRequest = {\r\n\t\t\tflow: flow.id,\r\n\t\t\tupdateSettingsFlowBody: {\r\n\t\t\t\tmethod: method,\r\n\t\t\t\tcsrf_token: csrfRef.value,\r\n\t\t\t\t...body\r\n\t\t\t}\r\n\t\t};\r\n\r\n\t\tmutexState = state ?? method;\r\n\t\ttry {\r\n\t\t\ttype T =\r\n\t\t\t\t| AxiosResponse<SettingsFlow & NeedsPrivilegedSessionError>\r\n\t\t\t\t| AxiosError<SettingsFlow & NeedsPrivilegedSessionError>;\r\n\t\t\tconst res: T = await Kratos.updateSettingsFlow(request).catch(err => err);\r\n\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tconst data = res instanceof AxiosError ? res.response?.data : res.data;\r\n\t\t\tif (data?.redirect_browser_to) {\r\n\t\t\t\twindow.location.href = data.redirect_browser_to;\r\n\t\t\t} else if (data?.ui) {\r\n\t\t\t\tawait Session.initialize(true);\r\n\t\t\t\tflow = data;\r\n\t\t\t} else if (data?.error?.reason) {\r\n\t\t\t\ttoast.error(data.error.reason, { duration: 10000 });\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t} finally {\r\n\t\t\tmutexState = undefined;\r\n\t\t}\r\n\t};\r\n\r\n\tconst onUpdateInformation = (ev: SubmitEvent) => {\r\n\t\tev.preventDefault();\r\n\t\tev.stopPropagation();\r\n\t\tev.stopImmediatePropagation();\r\n\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tif (!flow || !profileFormRef) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\t\t\treturn updateFlow(\"profile\", {\r\n\t\t\t\ttraits: {\r\n\t\t\t\t\temail: profileFormRef.email.value,\r\n\t\t\t\t\tusername: profileFormRef.username.value\r\n\t\t\t\t}\r\n\t\t\t});\r\n\t\t});\r\n\t};\r\n\r\n\tconst onUpdatePassword = (ev: SubmitEvent) => {\r\n\t\tev.preventDefault();\r\n\t\tev.stopPropagation();\r\n\t\tev.stopImmediatePropagation();\r\n\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tif (!flow || !passwordFormRef) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\t\t\treturn updateFlow(\"password\", {\r\n\t\t\t\tpassword: passwordFormRef.password.value\r\n\t\t\t});\r\n\t\t});\r\n\t};\r\n\r\n\tconst onOidc = (name: string, provider: string) =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tif (!flow) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\t\t\treturn updateFlow(\r\n\t\t\t\t\"oidc\",\r\n\t\t\t\t{\r\n\t\t\t\t\t[name]: provider\r\n\t\t\t\t},\r\n\t\t\t\tprovider\r\n\t\t\t);\r\n\t\t});\r\n</script>",
    "after": "<script lang=\"ts\">\r\n\timport { goto, replaceState } from \"$app/navigation\";\r\n\timport { page } from \"$app/state\";\r\n\timport { createAuthFormOwner } from \"$platform/auth-form-owner\";\r\n\timport { PUBLIC_SITE_NAME } from \"$env/static/public\";\r\n\timport { Constants, Kratos, Mutex } from \"$lib\";\r\n\timport ErrorComponent from \"$lib/components/ErrorComponent.svelte\";\r\n\timport { Button } from \"$lib/components/ui/button\";\r\n\timport * as Empty from \"$lib/components/ui/empty\";\r\n\timport * as Field from \"$lib/components/ui/field\";\r\n\timport * as InputGroup from \"$lib/components/ui/input-group\";\r\n\timport Input from \"$lib/components/ui/input/input.svelte\";\r\n\timport { Separator } from \"$lib/components/ui/separator\";\r\n\timport { Spinner } from \"$lib/components/ui/spinner\";\r\n\timport * as Tooltip from \"$lib/components/ui/tooltip\";\r\n\timport type { UINode } from \"$lib/Kratos\";\r\n\timport { InfoIcon, Link, Unlink } from \"@lucide/svelte\";\r\n\timport { Session } from \"$lib/stores\";\r\n\timport type {\r\n\t\tFrontendApiUpdateSettingsFlowRequest,\r\n\t\tGenericError,\r\n\t\tNeedsPrivilegedSessionError,\r\n\t\tSettingsFlow,\r\n\t\tUiNode\r\n\t} from \"@ory/client\";\r\n\timport { AxiosError, type AxiosResponse } from \"axios\";\r\n\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { onMount } from \"svelte\";\r\n\timport { toast } from \"svelte-sonner\";\r\n\r\n\tconst authFormOwner = createAuthFormOwner();\r\n\t$effect.pre(() => { page.url.href; authFormOwner.observe(window.location.href); });\r\n\tconst id = $props.id();\r\n\tconst mutex = new Mutex();\r\n\tlet mutexState = $state<string>();\r\n\r\n\tlet isMounted = $state(false);\r\n\tlet error = $state<Constants.Error.Error>();\r\n\tlet csrfRef = $state<HTMLInputElement>();\r\n\tlet profileFormRef = $state<HTMLFormElement>();\r\n\tlet passwordFormRef = $state<HTMLFormElement>();\r\n\tlet flow = $state<SettingsFlow>();\r\n\tlet nodes = $state<{\r\n\t\temail?: UINode;\r\n\t\tusername?: UINode;\r\n\t\tpassword?: UINode;\r\n\t\tcsrf_token?: UINode;\r\n\t\toidc?: UINode[];\r\n\t}>({});\r\n\r\n\tonMount(async () => {\r\n\t\tconst authIntent = authFormOwner.capture();\r\n\t\ttry {\r\n\t\t\tlet promise: Promise<AxiosResponse<SettingsFlow>>;\r\n\t\t\tif (page.url.searchParams.has(\"flow\")) {\r\n\t\t\t\tpromise = Kratos.getSettingsFlow({ id: page.url.searchParams.get(\"flow\")! });\r\n\t\t\t} else {\r\n\t\t\t\tpromise = Kratos.createBrowserSettingsFlow();\r\n\t\t\t}\r\n\r\n\t\t\tconst res = await promise.catch(async (err: AxiosError<SettingsFlow & GenericError>) => {\r\n\t\t\t\tif (!authIntent.alive()) throw Error(\"The previous authentication form is no longer current\");\r\n\t\t\t\tif (err.status === StatusCodes.UNAUTHORIZED) {\r\n\t\t\t\t\tgoto(`/auth/login?redirect=${encodeURIComponent(window.location.pathname + window.location.search)}`);\r\n\t\t\t\t}\r\n\t\t\t\treturn err.status === StatusCodes.GONE && err.response?.data\r\n\t\t\t\t\t? Kratos.createBrowserSettingsFlow({ returnTo: err.response.data.return_to || window.location.pathname })\r\n\t\t\t\t\t: err;\r\n\t\t\t});\r\n\t\t\tif (!authIntent.alive()) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tif (res instanceof AxiosError) {\r\n\t\t\t\tif (res.response?.data.error?.message) {\r\n\t\t\t\t\ttoast.error(res.response.data.error.message, { duration: 10000 });\r\n\t\t\t\t}\r\n\t\t\t\tthrow Constants.Error.Unknown;\r\n\t\t\t} else if (res.status === StatusCodes.OK && res.data) {\r\n\t\t\t\tconst q = new URLSearchParams(window.location.search);\r\n\t\t\t\tq.set(\"flow\", res.data.id);\r\n\t\t\t\tq.delete(\"redirect\");\r\n\r\n\t\t\t\tflow = res.data;\r\n\t\t\t\tauthIntent.replaceHistory(() => replaceState(window.location.pathname + q.toString(), page.state));\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (!authIntent.alive()) return;\r\n\t\t\tif (e instanceof Constants.Error.Error) error = e;\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\terror = Constants.Error.Unknown;\r\n\t\t\t}\r\n\t\t}\r\n\t\tif (!authIntent.alive()) return;\r\n\t\tisMounted = true;\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tflow?.ui.messages?.forEach(message => {\r\n\t\t\tswitch (message.type) {\r\n\t\t\t\tcase \"success\":\r\n\t\t\t\t\ttoast.success(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t\tcase \"info\":\r\n\t\t\t\t\ttoast.info(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t\tdefault:\r\n\t\t\t\t\ttoast.error(message.text, { duration: 10000 });\r\n\t\t\t\t\tbreak;\r\n\t\t\t}\r\n\t\t});\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tif (flow?.ui.nodes) {\r\n\t\t\tnodes = flow.ui.nodes.reduce(\r\n\t\t\t\t(acc, node) => {\r\n\t\t\t\t\tconst name = <string>(<any>node.attributes).name;\r\n\t\t\t\t\tif ([\"csrf_token\", \"traits.email\", \"traits.username\", \"password\"].includes(name)) {\r\n\t\t\t\t\t\t(<UiNode>acc[<keyof typeof nodes>name.replaceAll(\"traits.\", \"\")]) = node;\r\n\t\t\t\t\t} else if ([\"provider\", \"link\", \"unlink\"].includes(name)) {\r\n\t\t\t\t\t\t((<UiNode[]>acc[\"oidc\"]) ??= []).push(node);\r\n\t\t\t\t\t}\r\n\t\t\t\t\treturn acc;\r\n\t\t\t\t},\r\n\t\t\t\t{} as typeof nodes\r\n\t\t\t);\r\n\t\t}\r\n\t});\r\n\r\n\tconst updateFlow = async (method: \"profile\" | \"password\" | \"oidc\", body: any, state?: string) => {\r\n\t\tif (!flow || !csrfRef) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tconst authIntent = authFormOwner.capture(flow.id, () => flow?.id);\r\n\t\tif (!authIntent.alive()) return;\r\n\t\tconst returnTo = flow.return_to;\r\n\r\n\t\tconst request: FrontendApiUpdateSettingsFlowRequest = {\r\n\t\t\tflow: flow.id,\r\n\t\t\tupdateSettingsFlowBody: {\r\n\t\t\t\tmethod: method,\r\n\t\t\t\tcsrf_token: csrfRef.value,\r\n\t\t\t\t...body\r\n\t\t\t}\r\n\t\t};\r\n\r\n\t\tmutexState = state ?? method;\r\n\t\ttry {\r\n\t\t\ttype T =\r\n\t\t\t\t| AxiosResponse<SettingsFlow & NeedsPrivilegedSessionError>\r\n\t\t\t\t| AxiosError<SettingsFlow & NeedsPrivilegedSessionError>;\r\n\t\t\tconst res: T = await Kratos.updateSettingsFlow(request).catch(err => err);\r\n\t\t\tif (!authIntent.alive()) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tconst data = res instanceof AxiosError ? res.response?.data : res.data;\r\n\t\t\tif (data?.redirect_browser_to) {\r\n\t\t\t\twindow.location.href = data.redirect_browser_to;\r\n\t\t\t} else if (data?.ui) {\r\n\t\t\t\tif (data.id !== request.flow) throw new Constants.Error.Error(\"Authentication flow changed\", \"Reload the current form; an unrelated flow was not attached.\");\r\n\t\t\t\tawait Session.initialize(true);\r\n\t\t\t\tif (!authIntent.alive()) return;\r\n\t\t\t\tflow = data;\r\n\t\t\t} else if (data?.error?.reason) {\r\n\t\t\t\ttoast.error(data.error.reason, { duration: 10000 });\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t} catch (e) {\r\n\t\t\tif (!authIntent.alive()) return;\r\n\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t} finally {\r\n\t\t\tif (authIntent.alive()) mutexState = undefined;\r\n\t\t}\r\n\t};\r\n\r\n\tconst onUpdateInformation = (ev: SubmitEvent) => {\r\n\t\tev.preventDefault();\r\n\t\tev.stopPropagation();\r\n\t\tev.stopImmediatePropagation();\r\n\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tif (!flow || !profileFormRef) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\t\t\treturn updateFlow(\"profile\", {\r\n\t\t\t\ttraits: {\r\n\t\t\t\t\temail: profileFormRef.email.value,\r\n\t\t\t\t\tusername: profileFormRef.username.value\r\n\t\t\t\t}\r\n\t\t\t});\r\n\t\t});\r\n\t};\r\n\r\n\tconst onUpdatePassword = (ev: SubmitEvent) => {\r\n\t\tev.preventDefault();\r\n\t\tev.stopPropagation();\r\n\t\tev.stopImmediatePropagation();\r\n\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tif (!flow || !passwordFormRef) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\t\t\treturn updateFlow(\"password\", {\r\n\t\t\t\tpassword: passwordFormRef.password.value\r\n\t\t\t});\r\n\t\t});\r\n\t};\r\n\r\n\tconst onOidc = (name: string, provider: string) =>\r\n\t\tmutex.exec(async () => {\r\n\t\t\tif (!flow) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\t\t\treturn updateFlow(\r\n\t\t\t\t\"oidc\",\r\n\t\t\t\t{\r\n\t\t\t\t\t[name]: provider\r\n\t\t\t\t},\r\n\t\t\t\tprovider\r\n\t\t\t);\r\n\t\t});\r\n</script>"
  },
  {
    "path": "src/routes/(app)/settings/+layout.svelte",
    "label": "round10 settings route lifetime helper",
    "sha256": "59ebe00598167420b7b5ecea5f36214373f06e436e263f7aa7015f11b268574a",
    "count": 1,
    "before": "\timport { page } from \"$app/state\";",
    "after": "\timport { page } from \"$app/state\";\r\n\timport { createAuthFormOwner } from \"$platform/auth-form-owner\";"
  },
  {
    "path": "src/routes/(app)/settings/+layout.svelte",
    "label": "round10 settings owns route admission",
    "sha256": "59ebe00598167420b7b5ecea5f36214373f06e436e263f7aa7015f11b268574a",
    "count": 1,
    "before": "\tconst { children } = $props();",
    "after": "\tconst { children } = $props();\r\n\tconst routeOwner = createAuthFormOwner();"
  },
  {
    "path": "src/routes/(app)/settings/+layout.svelte",
    "label": "round10 settings delayed validation cannot redirect a later route",
    "sha256": "59ebe00598167420b7b5ecea5f36214373f06e436e263f7aa7015f11b268574a",
    "count": 1,
    "before": "\t$effect(() => {\r\n\t\tSession.isValid().then(() => {\r\n\t\t\tconst strs = window.location.pathname.split(\"/\");\r\n\t\t\tconst k = strs[strs.length - 1];\r\n\t\t\tif (!(k in tabs) || tabs[k].hidden) {\r\n\t\t\t\tgoto(`/settings/${Object.keys(tabs)[0]}`, { replaceState: true });\r\n\t\t\t}\r\n\t\t});\r\n\t});",
    "after": "\t$effect(() => {\r\n    page.url.href; tabs; const href = window.location.href;\r\n    routeOwner.observe(href); const intent = routeOwner.capture();\r\n    const key = new URL(href).pathname.split(\"/\").at(-1)!;\r\n    void Session.isValid().then(() => {\r\n      if (!intent.alive()) return;\r\n      const currentTabs = tabs;\r\n      if (!(key in currentTabs) || currentTabs[key].hidden) goto(`/settings/${Object.keys(currentTabs)[0]}`, { replaceState: true });\r\n    }).catch(() => { /* An unavailable/stale check cannot navigate this layout. */ });\r\n  });"
  },
  {
    "path": "src/routes/(app)/auth/logout/+page.svelte",
    "label": "round10 logout route lifetime helper",
    "sha256": "fa75f6f66212865d2fe460c5160a33f22c35a9168401161bcdbcd04bf5c3e1e7",
    "count": 1,
    "before": "\timport { page } from \"$app/state\";",
    "after": "\timport { page } from \"$app/state\";\r\n\timport { createAuthFormOwner } from \"$platform/auth-form-owner\";"
  },
  {
    "path": "src/routes/(app)/auth/logout/+page.svelte",
    "label": "round10 logout captures its own route before waiting",
    "sha256": "fa75f6f66212865d2fe460c5160a33f22c35a9168401161bcdbcd04bf5c3e1e7",
    "count": 1,
    "before": "\tlet isMounted = $state(false);",
    "after": "\tconst authFormOwner = createAuthFormOwner();\r\n\t$effect.pre(() => { page.url.href; authFormOwner.observe(window.location.href); });\r\n\tlet isMounted = $state(false);"
  },
  {
    "path": "src/routes/(app)/auth/logout/+page.svelte",
    "label": "round10 logout owned initial admission",
    "sha256": "fa75f6f66212865d2fe460c5160a33f22c35a9168401161bcdbcd04bf5c3e1e7",
    "count": 1,
    "before": "\tonMount(async () => {",
    "after": "\tonMount(async () => {\r\n\t\tconst authIntent = authFormOwner.capture();"
  },
  {
    "path": "src/routes/(app)/auth/logout/+page.svelte",
    "label": "round10 logout current validation continuation",
    "sha256": "fa75f6f66212865d2fe460c5160a33f22c35a9168401161bcdbcd04bf5c3e1e7",
    "count": 1,
    "before": "if (!(await Session.isValid())) {",
    "after": "const signedIn = await Session.isValid();\r\n\t\tif (!authIntent.alive()) return;\r\n\t\tif (!signedIn) {"
  },
  {
    "path": "src/routes/(app)/auth/logout/+page.svelte",
    "label": "round10 logout actual request admission",
    "sha256": "fa75f6f66212865d2fe460c5160a33f22c35a9168401161bcdbcd04bf5c3e1e7",
    "count": 1,
    "before": "const res = await Kratos.createBrowserLogoutFlow();",
    "after": "const res = await Kratos.createBrowserLogoutFlow();\r\n\t\t\tif (!authIntent.alive()) return;"
  },
  {
    "path": "src/routes/(app)/auth/logout/+page.svelte",
    "label": "round10 logout acknowledged UI lifetime",
    "sha256": "fa75f6f66212865d2fe460c5160a33f22c35a9168401161bcdbcd04bf5c3e1e7",
    "count": 1,
    "before": "await Kratos.updateLogoutFlow({ token: res.data.logout_token });",
    "after": "await Kratos.updateLogoutFlow({ token: res.data.logout_token });\r\n\t\t\t\tif (!authIntent.alive()) return;"
  },
  {
    "path": "src/routes/(app)/auth/logout/+page.svelte",
    "label": "round10 logout stale error is not current UI",
    "sha256": "fa75f6f66212865d2fe460c5160a33f22c35a9168401161bcdbcd04bf5c3e1e7",
    "count": 1,
    "before": "} catch (e) {",
    "after": "} catch (e) {\r\n\t\t\tif (!authIntent.alive()) return;"
  },
  {
    "path": "src/lib/components/reader/context.svelte.ts",
    "label": "round10 reader navigation intent import",
    "sha256": "7ee3da2eada7042e8cd7d2d2b464dfe03cd71a7bcd51f861e0b25b7eb5c02c4d",
    "count": 1,
    "before": "import { goto } from \"$app/navigation\";",
    "after": "import { goto } from \"$app/navigation\";\r\nimport { navigateReaderLastPage, consumeReaderLastPage } from \"$platform/reader-navigation\";"
  },
  {
    "path": "src/lib/components/reader/context.svelte.ts",
    "label": "round10 remove unowned reader last-page boolean",
    "sha256": "7ee3da2eada7042e8cd7d2d2b464dfe03cd71a7bcd51f861e0b25b7eb5c02c4d",
    "count": 1,
    "before": "let isNavigateLastPage = false;",
    "after": "// Last-page intent belongs to an exact chapter navigation in reader-navigation."
  },
  {
    "path": "src/lib/components/reader/context.svelte.ts",
    "label": "round10 only matching successful reader consumes last-page intent",
    "sha256": "7ee3da2eada7042e8cd7d2d2b464dfe03cd71a7bcd51f861e0b25b7eb5c02c4d",
    "count": 1,
    "before": "\t\tthis.isViewingLastPage = isNavigateLastPage;\r\n\t\tisNavigateLastPage = false;",
    "after": "\t\tthis.isViewingLastPage = consumeReaderLastPage(props.chapter.id);"
  },
  {
    "path": "src/lib/components/reader/context.svelte.ts",
    "label": "round10 previous-page freezes exact destination",
    "sha256": "7ee3da2eada7042e8cd7d2d2b464dfe03cd71a7bcd51f861e0b25b7eb5c02c4d",
    "count": 1,
    "before": "\t\t\t\t\t\tisNavigateLastPage = true;\r\n\t\t\t\t\t\tawait this.gotoPreviousChapter();",
    "after": "\t\t\t\t\t\tconst previous = this.findChapter(this.chapterIndex + 1);\r\n\t\t\t\t\t\tif (previous) await navigateReaderLastPage(previous.id, resolve(`/chapter/${previous.id}/1`));"
  },
  {
    "path": "src/routes/(app)/+layout.svelte",
    "label": "round10 global reader navigation boundary import",
    "sha256": "8a5902f4c6a511821403362bc04bea60283d64fc06763635d492bb16c03d9e7f",
    "count": 1,
    "before": "\timport AppHeader from \"$lib/components/AppHeader.svelte\";",
    "after": "\timport AppHeader from \"$lib/components/AppHeader.svelte\";\r\n\timport { installReaderNavigationGuard } from \"$platform/reader-navigation\";"
  },
  {
    "path": "src/routes/(app)/+layout.svelte",
    "label": "round10 global boundary clears last-page intent on different navigation",
    "sha256": "8a5902f4c6a511821403362bc04bea60283d64fc06763635d492bb16c03d9e7f",
    "count": 1,
    "before": "\tconst { children } = $props();",
    "after": "\tconst { children } = $props();\r\n\tinstallReaderNavigationGuard();"
  },
  {
    "path": "src/routes/(app)/settings/preferences/+page.svelte",
    "label": "round10 preferences initialize once per authority and preserve dirty baseline",
    "sha256": "0d77b163a1ec200ec7cf07951e475f413127b5a49f3f1ef5f137cf3c3baa8b7b",
    "count": 1,
    "before": "<script lang=\"ts\">\r\n\timport { PUBLIC_SITE_NAME } from \"$env/static/public\";\r\n\timport { API, Constants, Locale, Mutex } from \"$lib\";\r\n\timport type { UpdateSettingsRequest } from \"$lib/API/Settings\";\r\n\timport EntitySearchDialog from \"$lib/components/EntitySearchDialog.svelte\";\r\n\timport TagComboBox from \"$lib/components/TagComboBox.svelte\";\r\n\timport { Button, buttonVariants } from \"$lib/components/ui/button\";\r\n\timport { Checkbox } from \"$lib/components/ui/checkbox\";\r\n\timport * as Combobox from \"$lib/components/ui/combobox\";\r\n\timport * as Dialog from \"$lib/components/ui/dialog\";\r\n\timport * as Empty from \"$lib/components/ui/empty\";\r\n\timport ImageLoader from \"$lib/components/ui/ImageLoader.svelte\";\r\n\timport { Label } from \"$lib/components/ui/label\";\r\n\timport * as Select from \"$lib/components/ui/select\";\r\n\timport { Separator } from \"$lib/components/ui/separator\";\r\n\timport { Spinner } from \"$lib/components/ui/spinner\";\r\n\timport { populateTagList, prefs, Session } from \"$lib/stores\";\r\n\timport { cn } from \"$lib/Utils\";\r\n\timport { Plus, Trash } from \"@lucide/svelte\";\r\n\timport equal from \"fast-deep-equal\";\r\n\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { onMount } from \"svelte\";\r\n\timport { toast } from \"svelte-sonner\";\r\n\timport { createMutationOwner } from \"$platform/authenticated-mutation\";\r\n\timport { applyAccountPreferences, captureAccountPreferences } from \"$platform/account-preferences\";\r\n\timport { nativeAuthority } from \"$platform/mutation-authority\";\r\n\r\n\tconst mutex = new Mutex();\r\n\tconst preferenceOwner = createMutationOwner(() => window.location.href);\r\n\t$effect(() => { $nativeAuthority; onReset(); });\r\n\tlet draft = $state<UserPreferences>();\r\n\tconst isChanged = $derived(!equal($prefs, draft));\r\n\r\n\tonMount(async () => {\r\n\t\ttry {\r\n\t\t\tawait Promise.all([Session.isValid(), populateTagList()]);\r\n\t\t} catch (e) {\r\n\t\t\tconsole.error(e);\r\n\t\t}\r\n\t\tonReset();\r\n\t});\r\n\r\n\tconst onReset = () => {\r\n\t\tif ($prefs) {\r\n\t\t\tdraft = structuredClone($prefs);\r\n\t\t}\r\n\t};\r\n\r\n\tconst onSubmit = (ev: SubmitEvent) => {\r\n\t\tev.preventDefault(); ev.stopPropagation(); ev.stopImmediatePropagation();\r\n\t\tif (mutex.isLocked() || !$prefs || !draft || !isChanged) return;\r\n\t\tconst submitted = $state.snapshot(draft);\r\n\t\tif (!$Session) { prefs.set(structuredClone(submitted)); onReset(); toast.success(\"Your changes have been saved.\"); return; }\r\n\t\tconst intent = preferenceOwner.capture(), selected = captureAccountPreferences();\r\n\t\tconst payload: UpdateSettingsRequest = { version: selected.initial.version };\r\n\t\tObject.entries(submitted).forEach(([key, value]) => {\r\n\t\t\tif (equal(value, selected.initial[key as keyof UserPreferences])) return;\r\n\t\t\tif ([\"uploaders\", \"groups\"].includes(key)) (payload[key as keyof UpdateSettingsRequest] as string[]) = (value as { id: string }[])?.map(({ id }) => id) ?? [];\r\n\t\t\telse (payload[key as keyof UpdateSettingsRequest] as any) = Array.isArray(value) ? [...value] : value;\r\n\t\t});\r\n\t\tif (Object.keys(payload).length <= 1) { intent.release(); return; }\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\ttry {\r\n\t\t\t\tintent.assert(); const res = await API.Settings.Update(payload, intent); intent.assert();\r\n\t\t\t\tif (res?.status === StatusCodes.OK && res.body) { applyAccountPreferences(selected.actor, selected.authority, res.body, intent); onReset(); toast.success(\"Your changes have been saved.\"); }\r\n\t\t\t\telse if (res?.status === StatusCodes.TOO_MANY_REQUESTS) throw Constants.Error.TooManyRequests;\r\n\t\t\t\telse if (res?.status === StatusCodes.CONFLICT) throw Constants.Error.UpdateConflict;\r\n\t\t\t\telse throw Constants.Error.Unknown;\r\n\t\t\t} catch (cause) { if (!intent.alive()) return; const error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; toast.error(error.title, { description: error.body }); }\r\n\t\t\tfinally { intent.release(); }\r\n\t\t});\r\n\t};\r\n</script>",
    "after": "<script lang=\"ts\">\r\n\timport { PUBLIC_SITE_NAME } from \"$env/static/public\";\r\n\timport { API, Constants, Locale, Mutex } from \"$lib\";\r\n\timport type { UpdateSettingsRequest } from \"$lib/API/Settings\";\r\n\timport EntitySearchDialog from \"$lib/components/EntitySearchDialog.svelte\";\r\n\timport TagComboBox from \"$lib/components/TagComboBox.svelte\";\r\n\timport { Button, buttonVariants } from \"$lib/components/ui/button\";\r\n\timport { Checkbox } from \"$lib/components/ui/checkbox\";\r\n\timport * as Combobox from \"$lib/components/ui/combobox\";\r\n\timport * as Dialog from \"$lib/components/ui/dialog\";\r\n\timport * as Empty from \"$lib/components/ui/empty\";\r\n\timport ImageLoader from \"$lib/components/ui/ImageLoader.svelte\";\r\n\timport { Label } from \"$lib/components/ui/label\";\r\n\timport * as Select from \"$lib/components/ui/select\";\r\n\timport { Separator } from \"$lib/components/ui/separator\";\r\n\timport { Spinner } from \"$lib/components/ui/spinner\";\r\n\timport { populateTagList, prefs, Session } from \"$lib/stores\";\r\n\timport { cn } from \"$lib/Utils\";\r\n\timport { Plus, Trash } from \"@lucide/svelte\";\r\n\timport equal from \"fast-deep-equal\";\r\n\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { onMount, untrack } from \"svelte\";\r\n\timport { toast } from \"svelte-sonner\";\r\n\timport { createMutationOwner } from \"$platform/authenticated-mutation\";\r\n\timport { applyAccountPreferences, captureAccountPreferences } from \"$platform/account-preferences\";\r\n\timport { nativeAuthority } from \"$platform/mutation-authority\";\r\n\r\n\tconst mutex = new Mutex();\r\n\tconst preferenceOwner = createMutationOwner(() => window.location.href);\r\n\tlet draft = $state<UserPreferences>();\r\n  let draftBaseline = $state<UserPreferences>();\r\n  let draftAuthority = \"\";\r\n  const isChanged = $derived(!equal(draftBaseline, draft));\r\n  $effect.pre(() => {\r\n    const authority = JSON.stringify([$nativeAuthority.actor, $nativeAuthority.epoch]), actual = $prefs;\r\n    untrack(() => {\r\n      if (authority !== draftAuthority || !draft || !isChanged) {\r\n        draftAuthority = authority;\r\n        draftBaseline = structuredClone(actual); draft = structuredClone(actual);\r\n      }\r\n    });\r\n  });\r\n\r\n\tonMount(async () => {\r\n\t\ttry {\r\n\t\t\tawait Promise.all([Session.isValid(), populateTagList()]);\r\n\t\t} catch (e) {\r\n\t\t\tconsole.error(e);\r\n\t\t}\r\n\t\t// Tag readiness does not reset an already-editable preference draft.\r\n\t});\r\n\r\n\tconst onReset = () => {\r\n\t\tif ($prefs) {\r\n\t\t\tdraftBaseline = structuredClone($prefs);\r\n\t\t\tdraft = structuredClone($prefs);\r\n\t\t}\r\n\t};\r\n\r\n\tconst onSubmit = (ev: SubmitEvent) => {\r\n\t\tev.preventDefault(); ev.stopPropagation(); ev.stopImmediatePropagation();\r\n\t\tif (mutex.isLocked() || !$prefs || !draft || !draftBaseline || !isChanged) return;\r\n\t\tconst submitted = $state.snapshot(draft), baseline = $state.snapshot(draftBaseline);\r\n\t\tif (!$Session) { prefs.set(structuredClone(submitted)); onReset(); toast.success(\"Your changes have been saved.\"); return; }\r\n\t\tconst intent = preferenceOwner.capture(), selected = captureAccountPreferences();\r\n\t\tconst payload: UpdateSettingsRequest = { version: baseline.version };\r\n\t\tObject.entries(submitted).forEach(([key, value]) => {\r\n\t\t\tif (equal(value, baseline[key as keyof UserPreferences])) return;\r\n\t\t\tif ([\"uploaders\", \"groups\"].includes(key)) (payload[key as keyof UpdateSettingsRequest] as string[]) = (value as { id: string }[])?.map(({ id }) => id) ?? [];\r\n\t\t\telse (payload[key as keyof UpdateSettingsRequest] as any) = Array.isArray(value) ? [...value] : value;\r\n\t\t});\r\n\t\tif (Object.keys(payload).length <= 1) { intent.release(); return; }\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\ttry {\r\n\t\t\t\tintent.assert(); const res = await API.Settings.Update(payload, intent); intent.assert();\r\n\t\t\t\tif (res?.status === StatusCodes.OK && res.body) { applyAccountPreferences(selected.actor, selected.authority, res.body, intent); onReset(); toast.success(\"Your changes have been saved.\"); }\r\n\t\t\t\telse if (res?.status === StatusCodes.TOO_MANY_REQUESTS) throw Constants.Error.TooManyRequests;\r\n\t\t\t\telse if (res?.status === StatusCodes.CONFLICT) throw Constants.Error.UpdateConflict;\r\n\t\t\t\telse throw Constants.Error.Unknown;\r\n\t\t\t} catch (cause) { if (!intent.alive()) return; const error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; toast.error(error.title, { description: error.body }); }\r\n\t\t\tfinally { intent.release(); }\r\n\t\t});\r\n\t};\r\n</script>"
  },
  {
    "path": "src/routes/(app)/auth/+layout.svelte",
    "label": "round10 auth layout watches requested route/flow",
    "sha256": "dddbb202e26095ba9f348e2892d67366b570f40c4e84e0e4062128cc0cbd59b2",
    "count": 1,
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport { page } from \"$app/state\";"
  },
  {
    "path": "src/routes/(app)/auth/+layout.svelte",
    "label": "round10 exact requested flow owns rendered form lifetime",
    "sha256": "dddbb202e26095ba9f348e2892d67366b570f40c4e84e0e4062128cc0cbd59b2",
    "count": 1,
    "before": "\t\t{@render children()}",
    "after": "\t\t{#key JSON.stringify([page.url.pathname, page.url.searchParams.get(\"flow\")])}\r\n\t\t{@render children()}\r\n\t\t{/key}",
    "approved_auth_flow_key_exception": "auth_settings_requested_flow_key"
  },
  {
    "path": "src/routes/(app)/settings/+layout.svelte",
    "label": "round10 exact requested flow owns rendered form lifetime",
    "sha256": "59ebe00598167420b7b5ecea5f36214373f06e436e263f7aa7015f11b268574a",
    "count": 1,
    "before": "\t\t{@render children?.()}",
    "after": "\t\t{#key JSON.stringify([page.url.pathname, page.url.searchParams.get(\"flow\")])}\r\n\t\t{@render children?.()}\r\n\t\t{/key}",
    "approved_auth_flow_key_exception": "auth_settings_requested_flow_key"
  },
  {
    "path": "src/lib/components/reader/context.svelte.ts",
    "sha256": "7ee3da2eada7042e8cd7d2d2b464dfe03cd71a7bcd51f861e0b25b7eb5c02c4d",
    "count": 1,
    "label": "UI15 reader tap halves use actual page surface geometry and ignore owned interactive controls",
    "before": "\treadonly listener = $derived((ev: MouseEvent) => {\r\n\t\tif (!this.mainRef) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tlet clientX = ev.clientX;\r\n\t\tif (get(this.prefs).panel_position === \"left\") {\r\n\t\t\tclientX -= 450;\r\n\t\t}\r\n\r\n\t\tif (get(this.prefs).tap_navigation || get(this.prefs).layout !== Constants.Reader.Layout.Scroll) {\r\n\t\t\tlet isPrevious = clientX <= this.mainRef.clientWidth / 2;\r\n\t\t\tif (get(this.prefs).direction === Constants.Reader.Direction.RTL) {\r\n\t\t\t\tisPrevious = !isPrevious;\r\n\t\t\t}\r\n\r\n\t\t\tif (isPrevious) this.gotoPreviousPage();\r\n\t\t\telse this.gotoNextPage();\r\n\t\t}\r\n\t});",
    "after": "\treadonly listener = $derived((ev: MouseEvent) => {\r\n\t\tif (!this.mainRef || ev.defaultPrevented || ev.button !== 0) return;\r\n\t\tconst target = ev.target instanceof Element ? ev.target : undefined;\r\n\t\tif (target?.closest('a,button,input,select,textarea,[contenteditable]:not([contenteditable=\"false\"]),[role=\"button\"],[role=\"link\"],[data-reader-no-tap]')) return;\r\n\t\tconst prefs = get(this.prefs);\r\n\t\tif (!prefs.tap_navigation && prefs.layout === Constants.Reader.Layout.Scroll) return;\r\n\t\t// Both page/spread wrappers occupy the real reading region. Its viewport\r\n\t\t// geometry already reflects a visible, collapsed or mobile sidebar.\r\n\t\tconst surface = ev.currentTarget instanceof HTMLElement && this.mainRef.contains(ev.currentTarget)\r\n\t\t\t? ev.currentTarget : this.mainRef;\r\n\t\tconst bounds = surface.getBoundingClientRect(), x = ev.clientX - bounds.left;\r\n\t\tif (!Number.isFinite(x) || bounds.width <= 0 || x < 0 || x > bounds.width) return;\r\n\t\tlet isPrevious = x <= bounds.width / 2;\r\n\t\tif (prefs.direction === Constants.Reader.Direction.RTL) isPrevious = !isPrevious;\r\n\t\tif (isPrevious) void this.gotoPreviousPage();\r\n\t\telse void this.gotoNextPage();\r\n\t});"
  },
  {
    "path": "src/routes/(app)/title/[id]/edit/+page.svelte",
    "sha256": "c49a2631b678999fd4354cb8e17f5e3843b33d57532d4dba141401bd4aae5c78",
    "count": 1,
    "label": "UI17 editor draft cover read uses explicit current actor and private flag",
    "before": "\timport { API, Constants, Utils } from \"$lib\";",
    "after": "\timport { API, Constants, Utils, Permission } from \"$lib\";\r\n\timport { createEditorCoverReader } from \"$platform/editor-cover-read\";\r\n\timport { privateFeedIdentity } from \"$platform/private-feed\";"
  },
  {
    "path": "src/lib/components/draft/manga/MangaEdit.svelte",
    "sha256": "e1f8b3a97bc414eab974258ee1e529fa0a7df5b0841573f778868770e44b3782",
    "count": 1,
    "label": "UI17 cover batch owns actor before awaits and uses shared authoritative cover reader",
    "before": "\timport API from \"$lib/API\";",
    "after": "\timport API from \"$lib/API\";\r\n\timport { HTTPError } from \"$lib/HTTP\";\r\n\timport { createMutationOwner, authenticatedMutation, type MutationIntent } from \"$platform/authenticated-mutation\";\r\n\timport { createEditorCoverReader, savedCoverDraft, mergeCurrentCoverDrafts } from \"$platform/editor-cover-read\";\r\n\timport { requireMutationAcknowledgement, reportCurrentMutationFailure } from \"$platform/mutation-feedback\";"
  },
  {
    "path": "src/lib/components/draft/manga/MangaEdit.svelte",
    "sha256": "e1f8b3a97bc414eab974258ee1e529fa0a7df5b0841573f778868770e44b3782",
    "count": 1,
    "label": "UI17 cover upload uncertainty is not blindly replayed and reconciliation remains owned",
    "before": "\tconst mutex = new Mutex();",
    "after": "\tconst mutex = new Mutex();\r\n\tconst coverMutationOwner = createMutationOwner(() => data.manga.id + \"|\" + window.location.pathname);\r\n\tconst editorCoverReader = createEditorCoverReader(() => data.manga.id);\r\n\tlet unknownCoverIds = $state<string[]>([]), unknownRelationIds = $state<string[]>([]), coverReconciliationRequired = $state(false);"
  },
  {
    "path": "src/lib/components/draft/manga/MangaEdit.svelte",
    "sha256": "e1f8b3a97bc414eab974258ee1e529fa0a7df5b0841573f778868770e44b3782",
    "count": 1,
    "label": "UI17 uncertain pending cover Files fence save without replacing retained controls",
    "before": "\t\t\tdraft.covers.every(e => e.valid) &&",
    "after": "\t\t\t!coverReconciliationRequired && !unknownRelationIds.length && draft.covers.every(e => e.valid && !unknownCoverIds.includes(e.id!)) &&"
  },
  {
    "path": "src/lib/components/draft/manga/MangaEdit.svelte",
    "sha256": "e1f8b3a97bc414eab974258ee1e529fa0a7df5b0841573f778868770e44b3782",
    "count": 1,
    "label": "UI17 sequential title cover batch captures lifetime before mutation admission",
    "before": "\tconst onSubmit = (ev?: SubmitEvent) => {\r\n\t\tev?.preventDefault();\r\n\t\tev?.stopPropagation();\r\n\t\tev?.stopImmediatePropagation();\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tif (!manga || !initial || !draft || !isChanged || !isValid) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\ttry {\r\n\t\t\t\tawait updateManga();\r\n\t\t\t\tawait processCovers();\r\n\t\t\t\tawait processRelations();\r\n\r\n\t\t\t\tonReset();\r\n\t\t\t\ttoast.success(\"Title updated successfully\");\r\n\t\t\t} catch (e) {\r\n\t\t\t\tlet prefix = \"\";\r\n\t\t\t\tswitch (mutexState) {\r\n\t\t\t\t\tcase STATE.UpdatingEntry:\r\n\t\t\t\t\t\tprefix = \"Failed to update entry\";\r\n\t\t\t\t\t\tbreak;\r\n\t\t\t\t\tcase STATE.UploadingCovers:\r\n\t\t\t\t\t\tprefix = \"Failed to upload covers\";\r\n\t\t\t\t\t\tbreak;\r\n\t\t\t\t\tcase STATE.UpdatingCovers:\r\n\t\t\t\t\t\tprefix = \"Failed to update covers\";\r\n\t\t\t\t\t\tbreak;\r\n\t\t\t\t\tcase STATE.DeletingCovers:\r\n\t\t\t\t\t\tprefix = \"Failed to delete covers\";\r\n\t\t\t\t\t\tbreak;\r\n\t\t\t\t\tcase STATE.CreatingRelations:\r\n\t\t\t\t\t\tprefix = \"Failed to create relations\";\r\n\t\t\t\t\t\tbreak;\r\n\t\t\t\t\tcase STATE.UpdatingRelations:\r\n\t\t\t\t\t\tprefix = \"Failed to update relations\";\r\n\t\t\t\t\t\tbreak;\r\n\t\t\t\t\tcase STATE.DeletingRelations:\r\n\t\t\t\t\t\tprefix = \"Failed to delete relations\";\r\n\t\t\t\t\t\tbreak;\r\n\t\t\t\t}\r\n\t\t\t\tif (prefix) {\r\n\t\t\t\t\tprefix += \": \";\r\n\t\t\t\t}\r\n\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(prefix + e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(prefix + Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t} finally {\r\n\t\t\t\tmutexState = STATE.Idle;\r\n\t\t\t}\r\n\t\t});\r\n\t};",
    "after": "\tconst onSubmit = (ev?: SubmitEvent) => {\r\n\t\tev?.preventDefault(); ev?.stopPropagation(); ev?.stopImmediatePropagation();\r\n\t\tif (mutex.isLocked() || !manga || !initial || !draft || !isChanged || !isValid) return;\r\n\t\tlet intent: MutationIntent;\r\n\t\ttry { intent = coverMutationOwner.capture(); }\r\n\t\tcatch (cause) { reportCurrentMutationFailure(\"Title update not admitted\", cause, () => true); return; }\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\ttry {\r\n\t\t\t\tintent.assert(); await updateManga(); intent.assert();\r\n\t\t\t\tawait processCovers(intent); intent.assert();\r\n\t\t\t\tawait processRelations(intent); intent.assert();\r\n\t\t\t\tonReset(); toast.success(\"Title updated successfully\");\r\n\t\t\t} catch (cause) {\r\n\t\t\t\treportCurrentMutationFailure(\"Title update not completed\", cause, intent.alive);\r\n\t\t\t} finally { mutexState = STATE.Idle; }\r\n\t\t}).finally(() => intent.release());\r\n\t};"
  },
  {
    "path": "src/lib/components/draft/manga/MangaEdit.svelte",
    "sha256": "e1f8b3a97bc414eab974258ee1e529fa0a7df5b0841573f778868770e44b3782",
    "count": 1,
    "label": "UI17 existing Revert discards pending Files only after authoritative reconciliation",
    "before": "\tconst onReset = (ev?: Event & { currentTarget: HTMLFormElement }) => {\r\n\t\tev?.preventDefault();\r\n\t\tev?.stopPropagation();\r\n\t\tev?.stopImmediatePropagation();\r\n\t\tdraft = structuredClone($state.snapshot(initial));\r\n\t};",
    "after": "\tconst onReset = (ev?: Event & { currentTarget: HTMLFormElement }) => {\r\n\t\tev?.preventDefault(); ev?.stopPropagation(); ev?.stopImmediatePropagation();\r\n\t\tif (coverReconciliationRequired) {\r\n\t\t\ttoast.error(\"Current covers need confirmation\", { description: \"A cover operation may have succeeded but its current saved state could not be read. Reload this editor before another change; no upload was replayed.\" });\r\n\t\t\treturn;\r\n\t\t}\r\n\t\tfor (const cover of draft.covers) if (cover.file && cover.thumbnail?.startsWith(\"blob:\")) URL.revokeObjectURL(cover.thumbnail);\r\n\t\tdraft = structuredClone($state.snapshot(initial)); unknownCoverIds = []; unknownRelationIds = [];\r\n\t};"
  },
  {
    "path": "src/lib/components/draft/manga/MangaEdit.svelte",
    "sha256": "e1f8b3a97bc414eab974258ee1e529fa0a7df5b0841573f778868770e44b3782",
    "count": 1,
    "label": "UI17 each cover acknowledgement commits immediately with truthful owned failure reconciliation",
    "before": "\tconst processCovers = async () => {\r\n\t\tif (!manga || !draft) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tlet tmp: CoverDraft[] = [];\r\n\t\tfor (const coverDraft of draft.covers) {\r\n\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tif (coverDraft.file) {\r\n\t\t\t\tconst form = new FormData();\r\n\t\t\t\tform.append(\"file\", coverDraft.file!);\r\n\t\t\t\tform.append(\"language\", coverDraft.language!);\r\n\t\t\t\tif (coverDraft.volume) form.append(\"volume\", coverDraft.volume);\r\n\t\t\t\tif (coverDraft.description) form.append(\"description\", coverDraft.description);\r\n\t\t\t\tif (coverDraft.main) form.append(\"main\", \"true\");\r\n\r\n\t\t\t\tmutexState = STATE.UploadingCovers;\r\n\t\t\t\tconst res = await API.Cover.Upload(manga.id, form);\r\n\t\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\t\treturn;\r\n\t\t\t\t}\r\n\r\n\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\tcoverDraft.file = undefined;\r\n\t\t\t\t\ttmp.push({\r\n\t\t\t\t\t\t...coverDraft,\r\n\t\t\t\t\t\t...res.body\r\n\t\t\t\t\t});\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else if (res?.status === StatusCodes.INTERNAL_SERVER_ERROR && res.body?.error) {\r\n\t\t\t\t\tthrow new Constants.Error.Error(\"Internal Server Error\", res.body.error);\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} else if (coverDraft.deleted) {\r\n\t\t\t\tmutexState = STATE.DeletingCovers;\r\n\t\t\t\tconst res = await API.Cover.Delete(coverDraft.id!);\r\n\t\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\t\treturn;\r\n\t\t\t\t}\r\n\r\n\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\t//\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} else {\r\n\t\t\t\tconst initialCover = initial.covers.find(e => e.id === coverDraft.id);\r\n\t\t\t\tif (!initialCover || equal(initialCover, coverDraft)) {\r\n\t\t\t\t\tif (initialCover) {\r\n\t\t\t\t\t\ttmp.push(initialCover);\r\n\t\t\t\t\t}\r\n\t\t\t\t\tcontinue;\r\n\t\t\t\t}\r\n\r\n\t\t\t\tconst payload: UpdateCoverRequest = { version: coverDraft.version };\r\n\t\t\t\tif (coverDraft.volume !== initialCover.volume) payload.volume = coverDraft.volume;\r\n\t\t\t\tif (coverDraft.description !== initialCover.description) payload.description = coverDraft.description;\r\n\t\t\t\tif (coverDraft.language !== initialCover.language) payload.language = coverDraft.language;\r\n\t\t\t\tif (coverDraft.main != initialCover.main) {\r\n\t\t\t\t\tif (coverDraft.main) payload.main = coverDraft.main;\r\n\t\t\t\t\telse if (initialCover) initialCover.main = false;\r\n\t\t\t\t}\r\n\r\n\t\t\t\tif (Object.keys(payload).length <= 1) {\r\n\t\t\t\t\ttmp.push(initialCover);\r\n\t\t\t\t\tcontinue;\r\n\t\t\t\t}\r\n\r\n\t\t\t\tmutexState = STATE.UpdatingCovers;\r\n\t\t\t\tconst res = await API.Cover.Update(coverDraft.id!, payload);\r\n\t\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\t\treturn;\r\n\t\t\t\t}\r\n\r\n\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\ttmp.push({\r\n\t\t\t\t\t\t...coverDraft,\r\n\t\t\t\t\t\t...res.body\r\n\t\t\t\t\t});\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t}\r\n\t\t}\r\n\t\tcovers = tmp;\r\n\t};",
    "after": "\tconst processCovers = async (intent: MutationIntent) => {\r\n\t\tintent.assert();\r\n\t\tconst accepted = (row: CoverDraft, body: Cover) => {\r\n\t\t\tintent.assert();\r\n\t\t\tif (!body.id || !Number.isInteger(body.version) || body.version <= 0) throw new HTTPError(502, \"No real cover ID/version was acknowledged. Read the current covers before trying again.\", \"Cover outcome unknown\");\r\n\t\t\tconst oldThumbnail = row.thumbnail;\r\n\t\t\tconst saved = savedCoverDraft(manga, body, !!row.main);\r\n\t\t\tif (saved.main) covers = covers.map(other => ({ ...other, main: false }));\r\n\t\t\tconst position = covers.findIndex(other => other.id === body.id);\r\n\t\t\tcovers = position < 0 ? [...covers, saved] : covers.map((other, index) => index === position ? saved : other);\r\n\t\t\t// Commit the actual identity/version immediately, before the next request.\r\n\t\t\tObject.assign(row, saved, { file: undefined, deleted: undefined });\r\n\t\t\tif (oldThumbnail?.startsWith(\"blob:\")) URL.revokeObjectURL(oldThumbnail);\r\n\t\t};\r\n\t\ttry {\r\n\t\t\tfor (const row of [...draft.covers]) {\r\n\t\t\t\tintent.assert();\r\n\t\t\t\tif (row.file) {\r\n\t\t\t\t\tif (unknownCoverIds.includes(row.id!)) throw new HTTPError(409, \"This upload has an unknown outcome. Inspect the current saved covers, then use Revert to discard its pending File. It was not replayed.\", \"Cover outcome unknown\");\r\n\t\t\t\t\tconst form = new FormData(); form.append(\"file\", row.file); form.append(\"language\", row.language!);\r\n\t\t\t\t\tif (row.volume) form.append(\"volume\", row.volume);\r\n\t\t\t\t\tif (row.description) form.append(\"description\", row.description);\r\n\t\t\t\t\tif (row.main) form.append(\"main\", \"true\");\r\n\t\t\t\t\tmutexState = STATE.UploadingCovers;\r\n\t\t\t\t\tlet knownRejection = false;\r\n\t\t\t\t\ttry {\r\n\t\t\t\t\t\tconst result = await authenticatedMutation<Cover>(`/cover/${encodeURIComponent(manga.id)}`, \"POST\", form, intent);\r\n\t\t\t\t\t\tknownRejection = !!result && result.status >= 400 && result.status < 500;\r\n\t\t\t\t\t\trequireMutationAcknowledgement(result, 200, true); accepted(row, result!.body!);\r\n\t\t\t\t\t} catch (cause) {\r\n\t\t\t\t\t\tif (!knownRejection && intent.alive()) { unknownCoverIds = [...unknownCoverIds, row.id!]; coverReconciliationRequired = true; }\r\n\t\t\t\t\t\tthrow cause;\r\n\t\t\t\t\t}\r\n\t\t\t\t} else if (row.deleted) {\r\n\t\t\t\t\tmutexState = STATE.DeletingCovers;\r\n\t\t\t\t\tconst result = await authenticatedMutation(`/cover/${encodeURIComponent(row.id!)}`, \"DELETE\", undefined, intent);\r\n\t\t\t\t\trequireMutationAcknowledgement(result, 204); intent.assert();\r\n\t\t\t\t\tcovers = covers.filter(other => other.id !== row.id); draft.covers = draft.covers.filter(other => other.id !== row.id);\r\n\t\t\t\t} else {\r\n\t\t\t\t\tconst before = covers.find(other => other.id === row.id);\r\n\t\t\t\t\tif (!before) throw new HTTPError(409, \"This cover has no current saved identity. Reload the editor before updating it.\");\r\n\t\t\t\t\tconst payload: UpdateCoverRequest = { version: row.version };\r\n\t\t\t\t\tif (row.volume !== before.volume) payload.volume = row.volume;\r\n\t\t\t\t\tif (row.description !== before.description) payload.description = row.description;\r\n\t\t\t\t\tif (row.language !== before.language) payload.language = row.language;\r\n\t\t\t\t\tif (row.main && row.main !== before.main) payload.main = true;\r\n\t\t\t\t\tif (Object.keys(payload).length === 1) continue;\r\n\t\t\t\t\tmutexState = STATE.UpdatingCovers;\r\n\t\t\t\t\tconst result = await authenticatedMutation<Cover>(`/cover/${encodeURIComponent(row.id!)}`, \"PUT\", payload, intent);\r\n\t\t\t\t\trequireMutationAcknowledgement(result, 200, true); accepted(row, result!.body!);\r\n\t\t\t\t}\r\n\t\t\t}\r\n\t\t} catch (cause) {\r\n\t\t\tif (!intent.alive()) throw cause;\r\n\t\t\tcoverReconciliationRequired = true;\r\n\t\t\ttry {\r\n\t\t\t\tconst previous = $state.snapshot(covers), edited = $state.snapshot(draft.covers);\r\n\t\t\t\tconst currentManga = await editorCoverReader.manga(manga, intent);\r\n\t\t\t\tconst current = await editorCoverReader.covers(currentManga, intent); intent.assert();\r\n\t\t\t\tconst fresh = current.map(cover => savedCoverDraft(currentManga, cover));\r\n\t\t\t\t// Never reset unrelated metadata or associate unknown Files by filename.\r\n\t\t\t\tcovers = fresh; draft.covers = mergeCurrentCoverDrafts(previous, edited, fresh);\r\n\t\t\t\tif ((currentManga as Manga & { can_mutate?: boolean }).can_mutate === false) window.dispatchEvent(new CustomEvent(\"weebdex-title-publication-changed\", { detail: { id: currentManga.id } }));\r\n\t\t\t\tcoverReconciliationRequired = false;\r\n\t\t\t} catch {\r\n\t\t\t\tcoverReconciliationRequired = true;\r\n\t\t\t\tif (intent.alive()) throw new HTTPError(0, \"The cover batch stopped and the current saved covers could not be confirmed. Acknowledged real IDs/versions and pending edits were retained. Reload the editor to reconcile before retrying; no upload was replayed.\", \"Cover reconciliation unavailable\");\r\n\t\t\t}\r\n\t\t\tif (unknownCoverIds.length) throw new HTTPError(0, \"A cover upload may have succeeded without its acknowledgement. Current saved covers were read; pending Files were not matched by name or replayed. Inspect the saved covers, then use Revert to discard unknown pending Files before a new deliberate upload.\", \"Cover outcome unknown\");\r\n\t\t\tthrow cause;\r\n\t\t}\r\n\t};"
  },
  {
    "path": "src/lib/components/draft/manga/MangaCreate.svelte",
    "sha256": "bf404bbe81ddc28e529c88692af18310bdf0b433b4f29b23ff19b7e8660a8149",
    "count": 1,
    "label": "UI17 created draft recovery imports use actor-owned real requests and acknowledgements",
    "before": "\timport type { CreateMangaRequest } from \"$lib/API/Manga\";",
    "after": "\timport type { CreateMangaRequest } from \"$lib/API/Manga\";\r\n\timport { HTTPError } from \"$lib/HTTP\";\r\n\timport Session from \"$lib/stores/Session\";\r\n\timport equal from \"fast-deep-equal\";\r\n\timport { createMutationOwner, authenticatedMutation, type MutationIntent } from \"$platform/authenticated-mutation\";\r\n\timport { createEditorCoverReader, savedCoverDraft, mergeCurrentCoverDrafts } from \"$platform/editor-cover-read\";\r\n\timport { requireMutationAcknowledgement, reportCurrentMutationFailure } from \"$platform/mutation-feedback\";"
  },
  {
    "path": "src/lib/components/draft/manga/MangaCreate.svelte",
    "sha256": "bf404bbe81ddc28e529c88692af18310bdf0b433b4f29b23ff19b7e8660a8149",
    "count": 1,
    "label": "UI17 creation keeps acknowledged title identity and independently acknowledged remaining steps",
    "before": "\tlet manga = $state<Manga>();",
    "after": "\tlet manga = $state<Manga>();\r\n\tconst creationOwner = createMutationOwner(() => pathname + \"|\" + window.location.pathname);\r\n\tconst createdCoverReader = createEditorCoverReader(() => manga?.id ?? \"\");\r\n\tlet creationActor = $state<string>(), creationPayload = $state.raw<CreateMangaRequest>();\r\n\tlet creationUnknown = $state(false), unknownCoverIds = $state<string[]>([]), unknownRelationKeys = $state<string[]>([]);\r\n\tlet acceptedRelationKeys = $state<string[]>([]), acknowledgedCovers = $state<CoverDraft[]>([]), recoveryReadRequired = $state(false);\r\n\tconst canRetryCreatedDraft = $derived(!!manga && (manga as Manga & { can_mutate?: boolean }).can_mutate !== false && creationActor === $Session?.identity?.id && !creationUnknown && !unknownCoverIds.length && !unknownRelationKeys.length && !recoveryReadRequired);"
  },
  {
    "path": "src/lib/components/draft/manga/MangaCreate.svelte",
    "sha256": "bf404bbe81ddc28e529c88692af18310bdf0b433b4f29b23ff19b7e8660a8149",
    "count": 1,
    "label": "UI17 user-owned retry retains created title and never repeats acknowledged steps",
    "before": "\tconst onSubmit = (ev?: SubmitEvent) => {\r\n\t\tev?.preventDefault();\r\n\t\tev?.stopPropagation();\r\n\t\tev?.stopImmediatePropagation();\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\tif (!draft || !isValid) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\ttry {\r\n\t\t\t\tawait createManga();\r\n\t\t\t\tawait uploadCovers();\r\n\t\t\t\tawait createRelations();\r\n\r\n\t\t\t\tmutexState = STATE.Done;\r\n\t\t\t} catch (e) {\r\n\t\t\t\tlet prefix = \"\";\r\n\t\t\t\tswitch (mutexState) {\r\n\t\t\t\t\tcase STATE.CreatingEntry:\r\n\t\t\t\t\t\tprefix = \"Failed to create manga entry\";\r\n\t\t\t\t\t\tbreak;\r\n\t\t\t\t\tcase STATE.UploadingCovers:\r\n\t\t\t\t\t\tprefix = \"Failed to upload covers\";\r\n\t\t\t\t\t\tbreak;\r\n\t\t\t\t\tcase STATE.CreatingRelations:\r\n\t\t\t\t\t\tprefix = \"Failed to create relations\";\r\n\t\t\t\t\t\tbreak;\r\n\t\t\t\t}\r\n\t\t\t\tif (prefix) {\r\n\t\t\t\t\tprefix += \": \";\r\n\t\t\t\t}\r\n\r\n\t\t\t\tmutexState = mutexState > STATE.CreatingEntry ? STATE.Error : STATE.Idle;\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(prefix + e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(prefix + Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t}\r\n\t\t});\r\n\t};",
    "after": "\tconst onSubmit = (ev?: SubmitEvent) => {\r\n\t\tev?.preventDefault(); ev?.stopPropagation(); ev?.stopImmediatePropagation();\r\n\t\tif (mutex.isLocked() || mutexState === STATE.Done || !draft || !isValid) return;\r\n\t\tlet intent: MutationIntent;\r\n\t\ttry {\r\n\t\t\tintent = creationOwner.capture();\r\n\t\t\tif (creationUnknown || (manga && (!canRetryCreatedDraft || intent.actor !== creationActor))) {\r\n\t\t\t\tintent.release(); throw new HTTPError(409, \"A previous draft step has an unknown outcome or belongs to another account. Inspect the actual saved draft before a new deliberate request; no create/upload was replayed.\", \"Draft recovery requires review\");\r\n\t\t\t}\r\n\t\t} catch (cause) { reportCurrentMutationFailure(\"Draft request not admitted\", cause, () => true); return; }\r\n\t\treturn mutex.exec(async () => {\r\n\t\t\ttry {\r\n\t\t\t\tintent.assert(); await createManga(intent); intent.assert();\r\n\t\t\t\tawait uploadCovers(intent); intent.assert();\r\n\t\t\t\tawait createRelations(intent); intent.assert();\r\n\t\t\t\trecoveryReadRequired = true;\r\n\t\t\t\tmanga = await createdCoverReader.manga(manga!, intent); intent.assert();\r\n\t\t\t\trecoveryReadRequired = false; mutexState = STATE.Done;\r\n\t\t\t} catch (cause) {\r\n\t\t\t\tif (!intent.alive()) return;\r\n\t\t\t\tmutexState = manga || creationUnknown ? STATE.Error : STATE.Idle;\r\n\t\t\t\treportCurrentMutationFailure(\"Draft steps not completed\", cause, intent.alive);\r\n\t\t\t}\r\n\t\t}).finally(() => intent.release());\r\n\t};"
  },
  {
    "path": "src/lib/components/draft/manga/MangaCreate.svelte",
    "sha256": "bf404bbe81ddc28e529c88692af18310bdf0b433b4f29b23ff19b7e8660a8149",
    "count": 1,
    "label": "UI17 title creation accepts frozen metadata for one owned create command",
    "before": "\tconst createManga = async () => {",
    "after": "\tconst createManga = async (intent: MutationIntent) => {"
  },
  {
    "path": "src/lib/components/draft/manga/MangaCreate.svelte",
    "sha256": "bf404bbe81ddc28e529c88692af18310bdf0b433b4f29b23ff19b7e8660a8149",
    "count": 1,
    "label": "UI17 acknowledged title ID/version survives every later cover or relation failure",
    "before": "\t\tmutexState = STATE.CreatingEntry;\r\n\t\tconst res = await API.Manga.Create(payload);\r\n\t\tif (window.location.pathname !== pathname) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\tmanga = res.body;\r\n\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t} else throw Constants.Error.Unknown;\r\n\t};",
    "after": "\t\tintent.assert();\r\n\t\tconst submitted = structuredClone($state.snapshot(payload));\r\n\t\tif (manga) {\r\n\t\t\tif (!equal(submitted, creationPayload)) throw new HTTPError(409, \"The acknowledged title metadata differs from this draft. Use Edit saved draft to submit an explicit versioned change. Title creation was not repeated.\", \"Saved draft metadata changed\");\r\n\t\t\treturn;\r\n\t\t}\r\n\t\tif (creationUnknown) throw new HTTPError(409, \"The previous title creation may have succeeded. Inspect My drafts; a second title was not created.\");\r\n\t\tmutexState = STATE.CreatingEntry;\r\n\t\tlet knownRejection = false;\r\n\t\ttry {\r\n\t\t\tconst result = await authenticatedMutation<Manga>(\"/manga\", \"POST\", submitted, intent);\r\n\t\t\tknownRejection = !!result && result.status >= 400 && result.status < 500;\r\n\t\t\trequireMutationAcknowledgement(result, 201, true); intent.assert();\r\n\t\t\tif (!result!.body!.id || !Number.isInteger(result!.body!.version) || result!.body!.version <= 0 || !result!.body!.relationships) throw new HTTPError(502, \"No real created title identity/version was confirmed. Inspect My drafts before any new create request.\", \"Title creation outcome unknown\");\r\n\t\t\t// Actual acknowledgement is retained before any subsequent cover request.\r\n\t\t\tmanga = result!.body!; creationActor = intent.actor; creationPayload = submitted;\r\n\t\t} catch (cause) { if (!knownRejection && intent.alive()) creationUnknown = true; throw cause; }\r\n\t};"
  },
  {
    "path": "src/lib/components/draft/manga/MangaCreate.svelte",
    "sha256": "bf404bbe81ddc28e529c88692af18310bdf0b433b4f29b23ff19b7e8660a8149",
    "count": 1,
    "label": "UI17 created draft uploads retain real cover identities and skip prior acknowledged Files",
    "before": "\tconst uploadCovers = async () => {\r\n\t\tif (!manga || !draft) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tmutexState = STATE.UploadingCovers;\r\n\t\tfor (const coverDraft of draft.covers) {\r\n\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tconst form = new FormData();\r\n\t\t\tform.append(\"file\", coverDraft.file!);\r\n\t\t\tform.append(\"language\", coverDraft.language!);\r\n\t\t\tif (coverDraft.volume) form.append(\"volume\", coverDraft.volume);\r\n\t\t\tif (coverDraft.description) form.append(\"description\", coverDraft.description);\r\n\t\t\tif (coverDraft.main) form.append(\"main\", \"true\");\r\n\r\n\t\t\tconst res = await API.Cover.Upload(manga.id, form);\r\n\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\tcoverDraft.file = undefined;\r\n\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t} else if (res?.status === StatusCodes.INTERNAL_SERVER_ERROR && res.body?.error) {\r\n\t\t\t\tthrow new Constants.Error.Error(\"Internal Server Error\", res.body.error);\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t}\r\n\t};",
    "after": "\tconst uploadCovers = async (intent: MutationIntent) => {\r\n\t\tintent.assert(); if (!manga) throw new HTTPError(409, \"No acknowledged saved title is available.\");\r\n\t\tmutexState = STATE.UploadingCovers;\r\n\t\ttry {\r\n\t\t\tfor (const row of draft.covers) {\r\n\t\t\t\tintent.assert(); if (!row.file) continue; // already acknowledged: never POST again\r\n\t\t\t\tif (unknownCoverIds.includes(row.id!)) throw new HTTPError(409, \"This cover upload has an unknown outcome. Inspect the saved draft rather than replaying its File.\");\r\n\t\t\t\tconst form = new FormData(); form.append(\"file\", row.file); form.append(\"language\", row.language!);\r\n\t\t\t\tif (row.volume) form.append(\"volume\", row.volume);\r\n\t\t\t\tif (row.description) form.append(\"description\", row.description);\r\n\t\t\t\tif (row.main) form.append(\"main\", \"true\");\r\n\t\t\t\tlet knownRejection = false;\r\n\t\t\t\ttry {\r\n\t\t\t\t\tconst result = await authenticatedMutation<Cover>(`/cover/${encodeURIComponent(manga.id)}`, \"POST\", form, intent);\r\n\t\t\t\t\tknownRejection = !!result && result.status >= 400 && result.status < 500;\r\n\t\t\t\t\trequireMutationAcknowledgement(result, 200, true); intent.assert();\r\n\t\t\t\t\tconst body = result!.body!;\r\n\t\t\t\t\tif (!body.id || !Number.isInteger(body.version) || body.version <= 0) throw new HTTPError(502, \"No real cover ID/version was acknowledged.\", \"Cover outcome unknown\");\r\n\t\t\t\t\tconst oldThumbnail = row.thumbnail, saved = savedCoverDraft(manga, body, !!row.main);\r\n\t\t\t\t\tif (saved.main) acknowledgedCovers = acknowledgedCovers.map(other => ({ ...other, main: false }));\r\n\t\t\t\t\tacknowledgedCovers = [...acknowledgedCovers.filter(other => other.id !== saved.id), saved];\r\n\t\t\t\t\tObject.assign(row, saved, { file: undefined, deleted: undefined });\r\n\t\t\t\t\tif (oldThumbnail?.startsWith(\"blob:\")) URL.revokeObjectURL(oldThumbnail);\r\n\t\t\t\t} catch (cause) { if (!knownRejection && intent.alive()) unknownCoverIds = [...unknownCoverIds, row.id!]; throw cause; }\r\n\t\t\t}\r\n\t\t} catch (cause) {\r\n\t\t\tif (!intent.alive()) throw cause;\r\n\t\t\trecoveryReadRequired = true;\r\n\t\t\ttry {\r\n\t\t\t\tconst previous = $state.snapshot(acknowledgedCovers), edited = $state.snapshot(draft.covers);\r\n\t\t\t\tconst currentManga = await createdCoverReader.manga(manga, intent);\r\n\t\t\t\tconst currentCovers = await createdCoverReader.covers(currentManga, intent); intent.assert();\r\n\t\t\t\tconst saved = currentCovers.map(cover => savedCoverDraft(currentManga, cover));\r\n\t\t\t\tacknowledgedCovers = saved; draft.covers = mergeCurrentCoverDrafts(previous, edited, saved); manga = currentManga; recoveryReadRequired = false;\r\n\t\t\t} catch { if (intent.alive()) throw new HTTPError(0, \"The existing title and acknowledged cover IDs were retained, but fresh owner cover data could not be confirmed. Open Edit saved draft to reconcile; no title or cover was replayed.\", \"Saved draft reconciliation unavailable\"); }\r\n\t\t\tif (unknownCoverIds.length) throw new HTTPError(0, \"A cover may have been accepted without its acknowledgement. The actual saved covers were read; pending Files were not matched by filename or replayed. Open Edit saved draft and review its current covers before another deliberate upload.\", \"Cover outcome unknown\");\r\n\t\t\tthrow cause;\r\n\t\t}\r\n\t};"
  },
  {
    "path": "src/lib/components/draft/manga/MangaCreate.svelte",
    "sha256": "bf404bbe81ddc28e529c88692af18310bdf0b433b4f29b23ff19b7e8660a8149",
    "count": 1,
    "label": "UI17 created relation acknowledgements survive later failure without replaying accepted relations",
    "before": "\tconst createRelations = async () => {\r\n\t\tif (!manga || !draft) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tmutexState = STATE.CreatingRelations;\r\n\t\tfor (const relationDraft of draft.relations) {\r\n\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tconst res = await API.Manga.CreateRelation(manga.id, {\r\n\t\t\t\trelated_id: relationDraft.id,\r\n\t\t\t\ttype: relationDraft.type\r\n\t\t\t});\r\n\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\treturn;\r\n\t\t\t}\r\n\r\n\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t//\r\n\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t}\r\n\t};",
    "after": "\tconst createRelations = async (intent: MutationIntent) => {\r\n\t\tintent.assert(); if (!manga) throw new HTTPError(409, \"No acknowledged saved title is available.\");\r\n\t\tmutexState = STATE.CreatingRelations;\r\n\t\tfor (const row of draft.relations) {\r\n\t\t\tintent.assert(); if (row.id === manga.id || row.deleted) continue;\r\n\t\t\tconst key = JSON.stringify([row.id, row.type]);\r\n\t\t\tif (acceptedRelationKeys.includes(key)) continue;\r\n\t\t\tif (unknownRelationKeys.includes(key)) throw new HTTPError(409, \"This relation has an unknown outcome. Inspect the current saved draft before another deliberate change.\");\r\n\t\t\tlet knownRejection = false;\r\n\t\t\ttry {\r\n\t\t\t\tconst result = await authenticatedMutation<Relation & { manga_version: number }>(`/manga/${encodeURIComponent(manga.id)}/relations`, \"POST\", { related_id: row.id, type: row.type, version: manga.version }, intent);\r\n\t\t\t\tknownRejection = !!result && result.status >= 400 && result.status < 500;\r\n\t\t\t\trequireMutationAcknowledgement(result, 201, true); intent.assert();\r\n\t\t\t\tif (result!.body!.id !== row.id || result!.body!.type !== row.type) throw new HTTPError(502, \"No matching relation acknowledgement was returned.\");\r\n\t\t\t\tacceptedRelationKeys = [...acceptedRelationKeys, key]; Object.assign(row, result!.body!, { new: false });\r\n\t\t\t\tif (!Number.isSafeInteger(result!.body!.manga_version) || result!.body!.manga_version <= 0) throw new HTTPError(502, \"The actual owning title version was not returned. Reconcile saved state before continuing.\");\r\n\t\t\t\tmanga.version = result!.body!.manga_version;\r\n\t\t\t} catch (cause) {\r\n\t\t\t\tif (!intent.alive()) throw cause;\r\n\t\t\t\tif (!knownRejection) unknownRelationKeys = [...unknownRelationKeys, key];\r\n\t\t\t\trecoveryReadRequired = true;\r\n\t\t\t\ttry {\r\n\t\t\t\t\tconst current = await createdCoverReader.manga(manga, intent); intent.assert();\r\n\t\t\t\t\tif (current.relationships.relations?.some(relation => relation.id === row.id && relation.type === row.type)) {\r\n\t\t\t\t\t\tacceptedRelationKeys = [...new Set([...acceptedRelationKeys, key])]; unknownRelationKeys = unknownRelationKeys.filter(other => other !== key);\r\n\t\t\t\t\t}\r\n\t\t\t\t\tmanga = current; recoveryReadRequired = false;\r\n\t\t\t\t} catch { /* retain exact acknowledged state; recovery stays fenced */ }\r\n\t\t\t\tthrow cause;\r\n\t\t\t}\r\n\t\t}\r\n\t};"
  },
  {
    "path": "src/lib/components/draft/manga/MangaCreate.svelte",
    "sha256": "bf404bbe81ddc28e529c88692af18310bdf0b433b4f29b23ff19b7e8660a8149",
    "count": 1,
    "label": "UI17 acknowledged title error screen exposes exact remaining-step retry and saved-draft recovery",
    "before": "\t\t{:else if mutexState === STATE.Error}\r\n\t\t\t<Empty.Content>\r\n\t\t\t\t<div class=\"flex flex-wrap items-center justify-center gap-2\">\r\n\t\t\t\t\t<a class={buttonVariants({ class: \"border-none\", variant: \"outline\" })} href=\"/drafts/title\">",
    "after": "\t\t{:else if mutexState === STATE.Error}\r\n\t\t\t<Empty.Content>\r\n\t\t\t\t<div class=\"flex flex-wrap items-center justify-center gap-2\">\r\n\t\t\t\t\t{#if manga}\r\n\t\t\t\t\t\t<Button variant=\"outline\" disabled={$mutex || !isValid || !canRetryCreatedDraft} onclick={() => onSubmit()}>Retry remaining draft steps</Button>\r\n\t\t\t\t\t\t<a class={buttonVariants({ class: \"border-none\", variant: \"outline\" })} href={`/title/${encodeURIComponent(manga.id)}/edit?draft=true`}>Edit saved draft</a>\r\n\t\t\t\t\t{/if}\r\n\t\t\t\t\t<a class={buttonVariants({ class: \"border-none\", variant: \"outline\" })} href=\"/drafts/title\">",
    "approved_create_recovery_exception": "acknowledged_draft_recovery_controls"
  },
  {
    "path": "src/routes/(app)/title/[id]/edit/+page.svelte",
    "sha256": "c49a2631b678999fd4354cb8e17f5e3843b33d57532d4dba141401bd4aae5c78",
    "count": 1,
    "label": "UI17 private editor head and child attach only a freshly owned title DTO",
    "before": "\tconst { data } = $props();",
    "after": "\tconst { data: loadedData } = $props();\r\n\tlet resolvedEditorManga = $state<Manga>(), resolvedEditorAuthority = $state<string>();\r\n\t// Every retained head/body expression sees only the current owned read.\r\n\tconst data = $derived({ ...loadedData, manga: resolvedEditorAuthority === $privateFeedIdentity && resolvedEditorManga?.id === loadedData.manga?.id ? resolvedEditorManga : undefined });"
  },
  {
    "path": "src/lib/components/draft/manga/MangaEdit.svelte",
    "sha256": "e1f8b3a97bc414eab974258ee1e529fa0a7df5b0841573f778868770e44b3782",
    "count": 1,
    "label": "UI17 existing relation types use versioned PUT and each mutation commits actual owner acknowledgement",
    "before": "\tconst processRelations = async () => {\r\n\t\tif (!manga || !draft) {\r\n\t\t\treturn;\r\n\t\t}\r\n\r\n\t\tlet tmp: RelationDraft[] = [];\r\n\t\tfor (const relationDraft of draft.relations) {\r\n\t\t\tif (window.location.pathname !== pathname) return;\r\n\t\t\tif (relationDraft.id === manga.id) continue;\r\n\r\n\t\t\tif (relationDraft.deleted) {\r\n\t\t\t\tmutexState = STATE.DeletingRelations;\r\n\t\t\t\tconst res = await API.Manga.DeleteRelation(manga.id, {\r\n\t\t\t\t\trelated_id: relationDraft.id\r\n\t\t\t\t});\r\n\t\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\t\treturn;\r\n\t\t\t\t}\r\n\r\n\t\t\t\tif (res?.status === StatusCodes.NO_CONTENT) {\r\n\t\t\t\t\t//\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t} else {\r\n\t\t\t\tif (!relationDraft.new) {\r\n\t\t\t\t\tconst initialRelation = initial.relations.find(e => e.id === relationDraft.id);\r\n\t\t\t\t\tif (initialRelation && equal(initialRelation, relationDraft)) {\r\n\t\t\t\t\t\ttmp.push(initialRelation);\r\n\t\t\t\t\t\tcontinue;\r\n\t\t\t\t\t}\r\n\t\t\t\t}\r\n\r\n\t\t\t\tmutexState = relationDraft.new ? STATE.CreatingRelations : STATE.UpdatingRelations;\r\n\t\t\t\tconst res = await API.Manga.CreateRelation(manga.id, {\r\n\t\t\t\t\trelated_id: relationDraft.id,\r\n\t\t\t\t\ttype: relationDraft.type\r\n\t\t\t\t});\r\n\t\t\t\tif (window.location.pathname !== pathname) {\r\n\t\t\t\t\treturn;\r\n\t\t\t\t}\r\n\r\n\t\t\t\tif (res?.status === StatusCodes.OK && res.body) {\r\n\t\t\t\t\ttmp.push(res.body);\r\n\t\t\t\t} else if (res?.status === StatusCodes.FORBIDDEN) {\r\n\t\t\t\t\tthrow Constants.Error.Forbidden;\r\n\t\t\t\t} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {\r\n\t\t\t\t\tthrow Constants.Error.TooManyRequests;\r\n\t\t\t\t} else throw Constants.Error.Unknown;\r\n\t\t\t}\r\n\t\t}\r\n\t\tmanga.relationships.relations = tmp;\r\n\t};",
    "after": "\tconst processRelations = async (intent: MutationIntent) => {\r\n\t\tintent.assert();\r\n\t\tconst applyVersion = (value: unknown) => {\r\n\t\t\tconst version = typeof value === \"string\" && /^[1-9][0-9]*$/.test(value) ? Number(value) : value;\r\n\t\t\tif (!Number.isSafeInteger(version) || Number(version) <= 0) throw new HTTPError(502, \"The actual owning title version was not acknowledged. Read current state before another relation mutation.\", \"Relation outcome unknown\");\r\n\t\t\tmanga.version = Number(version);\r\n\t\t};\r\n\t\ttry {\r\n\t\t\tfor (const row of [...draft.relations]) {\r\n\t\t\t\tintent.assert(); if (row.id === manga.id) continue;\r\n\t\t\t\tconst before = manga.relationships.relations?.find(relation => relation.id === row.id);\r\n\t\t\t\tif (unknownRelationIds.includes(row.id)) throw new HTTPError(409, \"This relation has an unknown outcome. Inspect current saved state and use Revert before a new deliberate change.\");\r\n\t\t\t\tif (row.deleted && !before) { draft.relations = draft.relations.filter(other => other.id !== row.id); continue; }\r\n\t\t\t\tif (!row.deleted && before?.type === row.type) continue;\r\n\t\t\t\tlet knownRejection = false;\r\n\t\t\t\ttry {\r\n\t\t\t\t\tif (row.deleted) {\r\n\t\t\t\t\t\tmutexState = STATE.DeletingRelations;\r\n\t\t\t\t\t\tconst result = await authenticatedMutation(`/manga/${encodeURIComponent(manga.id)}/relations`, \"DELETE\", { related_id: row.id, version: manga.version }, intent);\r\n\t\t\t\t\t\tknownRejection = !!result && result.status >= 400 && result.status < 500;\r\n\t\t\t\t\t\trequireMutationAcknowledgement(result, 204); intent.assert();\r\n\t\t\t\t\t\t// Removal acknowledgement is durable locally even if its version header is lost.\r\n\t\t\t\t\t\tmanga.relationships.relations = (manga.relationships.relations ?? []).filter(other => other.id !== row.id);\r\n\t\t\t\t\t\tdraft.relations = draft.relations.filter(other => other.id !== row.id);\r\n\t\t\t\t\t\tapplyVersion(result!.headers?.[\"x-weebdex-manga-version\"]);\r\n\t\t\t\t\t} else {\r\n\t\t\t\t\t\tconst method = before ? \"PUT\" : \"POST\";\r\n\t\t\t\t\t\tmutexState = before ? STATE.UpdatingRelations : STATE.CreatingRelations;\r\n\t\t\t\t\t\tconst result = await authenticatedMutation<Relation & { manga_version: number }>(`/manga/${encodeURIComponent(manga.id)}/relations`, method, { related_id: row.id, type: row.type, version: manga.version }, intent);\r\n\t\t\t\t\t\tknownRejection = !!result && result.status >= 400 && result.status < 500;\r\n\t\t\t\t\t\trequireMutationAcknowledgement(result, before ? 200 : 201, true); intent.assert();\r\n\t\t\t\t\t\tif (result!.body!.id !== row.id || result!.body!.type !== row.type) throw new HTTPError(502, \"No matching saved relation was acknowledged.\");\r\n\t\t\t\t\t\tconst saved = result!.body!;\r\n\t\t\t\t\t\tmanga.relationships.relations = [...(manga.relationships.relations ?? []).filter(other => other.id !== row.id), saved];\r\n\t\t\t\t\t\tObject.assign(row, saved, { new: false, deleted: undefined });\r\n\t\t\t\t\t\tapplyVersion(saved.manga_version); // never Relation.version or a guessed increment\r\n\t\t\t\t\t}\r\n\t\t\t\t} catch (cause) { if (!knownRejection && intent.alive()) unknownRelationIds = [...unknownRelationIds, row.id]; throw cause; }\r\n\t\t\t}\r\n\t\t} catch (cause) {\r\n\t\t\tif (!intent.alive()) throw cause;\r\n\t\t\tcoverReconciliationRequired = true;\r\n\t\t\ttry {\r\n\t\t\t\tconst previous = $state.snapshot(initial), edited = $state.snapshot(draft), oldCovers = $state.snapshot(covers);\r\n\t\t\t\tconst current = await editorCoverReader.manga(manga, intent);\r\n\t\t\t\tconst images = await editorCoverReader.covers(current, intent); intent.assert();\r\n\t\t\t\tconst freshCovers = images.map(cover => savedCoverDraft(current, cover));\r\n\t\t\t\tcovers = freshCovers; manga = current;\r\n\t\t\t\tconst next = $state.snapshot(initial);\r\n\t\t\t\t// Adopt fresh acknowledged fields only where no local edit was pending.\r\n\t\t\t\tfor (const key of Object.keys(next) as (keyof MangaDraft)[]) {\r\n\t\t\t\t\tif (key !== \"covers\" && key !== \"relations\" && equal(edited[key], previous[key])) (draft as any)[key] = structuredClone(next[key]);\r\n\t\t\t\t}\r\n\t\t\t\tdraft.covers = mergeCurrentCoverDrafts(oldCovers, edited.covers, freshCovers);\r\n\t\t\t\tconst saved = new Map((current.relationships.relations ?? []).map(relation => [relation.id, relation]));\r\n\t\t\t\tdraft.relations = edited.relations.flatMap(row => {\r\n\t\t\t\t\tconst fresh = saved.get(row.id), before = previous.relations.find(relation => relation.id === row.id);\r\n\t\t\t\t\tsaved.delete(row.id);\r\n\t\t\t\t\tif (row.deleted && !fresh) { unknownRelationIds = unknownRelationIds.filter(id => id !== row.id); return []; }\r\n\t\t\t\t\tif (!fresh) return row.new ? [row] : [];\r\n\t\t\t\t\tif (!row.deleted && fresh.type === row.type) unknownRelationIds = unknownRelationIds.filter(id => id !== row.id);\r\n\t\t\t\t\treturn [{ ...fresh, type: before?.type !== row.type ? row.type : fresh.type, new: false, deleted: row.deleted }];\r\n\t\t\t\t});\r\n\t\t\t\tdraft.relations = [...draft.relations, ...saved.values()]; coverReconciliationRequired = false;\r\n\t\t\t\tif ((current as Manga & { can_mutate?: boolean }).can_mutate === false) window.dispatchEvent(new CustomEvent(\"weebdex-title-publication-changed\", { detail: { id: current.id } }));\r\n\t\t\t} catch { if (intent.alive()) throw new HTTPError(0, \"Earlier acknowledged relations and pending edits were retained, but current owner state could not be read. Reload this editor before another mutation; no relation was replayed.\", \"Relation reconciliation unavailable\"); }\r\n\t\t\tif (unknownRelationIds.length) throw new HTTPError(0, \"A relation change has an unresolved outcome. Current saved state was read; inspect it and use Revert to discard the unresolved local intent before a new deliberate change. No mutation was replayed.\", \"Relation outcome unknown\");\r\n\t\t\tthrow cause;\r\n\t\t}\r\n\t};"
  },
  {
    "path": "src/lib/permission/manga.ts",
    "sha256": "fdf62a1220d8e13f446f4fc070788c78aded93baeef360ae88cee2b67827e799",
    "count": 1,
    "label": "UI18 relation control eligibility matches native live unlocked permission or actual draft-owner authority",
    "before": "export const canEditRelations = (manga?: Manga): boolean => {\r\n\tif (!Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && (manga?.locked || !manga?.deleted_at)) {\r\n\t\treturn false;\r\n\t}\r\n\treturn Session.hasPerm(\"manga.relation.edit\");\r\n};",
    "after": "export const canEditRelations = (manga?: Manga): boolean => {\r\n\tconst actor = get(Session);\r\n\tif (!actor?.identity?.id) return false;\r\n\t// The create form has no saved title yet; its future draft owner needs manga.create.\r\n\tif (!manga) return Session.hasPerm(\"manga.create\");\r\n\tconst current = manga as Manga & { can_mutate?: boolean; is_unavailable?: boolean };\r\n\tif (manga.deleted_at || current.can_mutate === false || current.is_unavailable) return false;\r\n\tif (Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm(\"manga.relation.edit\")) return true;\r\n\tif (manga.locked) return false;\r\n\tif (manga.state === State.Published) return Session.hasPerm(\"manga.relation.edit\");\r\n\treturn manga.relationships?.submitter?.id === actor.identity.id && Session.hasPerm(\"manga.create\");\r\n};"
  },
  {
    "path": "src/lib/stores/Session.ts",
    "label": "bootstrap Session depends on the authentication leaf instead of the API barrel",
    "sha256": "8baf2edac726343a1f1445c53afa7f9a5338c66fad41ffbbe6ac7285d5a25498",
    "count": 1,
    "before": "import API from \"$lib/API\";",
    "after": "import * as Auth from \"$lib/API/Auth\";"
  },
  {
    "path": "src/lib/stores/Session.ts",
    "label": "session initialization uses its authentication leaf",
    "sha256": "8baf2edac726343a1f1445c53afa7f9a5338c66fad41ffbbe6ac7285d5a25498",
    "count": 1,
    "before": "API.Auth.Check()",
    "after": "Auth.Check()"
  },
  {
    "path": "src/lib/API/Auth.ts",
    "label": "authentication leaf depends on HTTP without reentering the application barrel",
    "sha256": "098a14b177ea3b9dfa825fd7a86b0951e7454016185abe604ae79914127e5b49",
    "count": 1,
    "before": "import { HTTP } from \"$lib\";",
    "after": "import HTTP from \"$lib/HTTP\";"
  },
  {
    "path": "src/lib/stores/prefs.ts",
    "label": "bootstrap preferences read initialized constants outside the API Session cycle",
    "sha256": "cd441d7ca0bddd4ac0e2c8d60b40f073143b0c3b47084455dc2bb2c80acb64b6",
    "count": 1,
    "before": "import { Constants } from \"$lib\";",
    "after": "import * as Constants from \"$lib/constants\";"
  },
{
  "path": "src/lib/components/MangaSearchFilter.svelte",
  "label": "search UI imports field-specific query normalization",
  "sha256": "f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7",
  "count": 1,
  "before": "\timport type { SearchMangaRequest } from \"$lib/API/Manga\";",
  "after": "\timport type { SearchMangaRequest } from \"$lib/API/Manga\";\r\n\timport { parseMangaSearchQuery, serializeMangaSearchQuery, searchValidationError, unresolvedSearchIssues, validateMangaSearchQuery, type MangaFilterRequest, type SearchIssue } from \"$platform/manga-search-query\";\r\n\timport { toast } from \"svelte-sonner\";"
},
{
  "path": "src/lib/components/MangaSearchFilter.svelte",
  "label": "search initialization keeps a reviewable query issue state",
  "sha256": "f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7",
  "count": 1,
  "before": "\tlet search = $state<SearchMangaRequest>(",
  "after": "\tlet searchIssues = $state<SearchIssue[]>([]), searchPassthrough = new URLSearchParams(), explicitHasChapters = false, explicitEmptyRating = false;\r\n\tlet initialParsedSearch: MangaFilterRequest;\r\n\tlet search = $state<MangaFilterRequest>("
},
{
  "path": "src/lib/components/MangaSearchFilter.svelte",
  "label": "search URL values are normalized before typed field assignment",
  "sha256": "f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7",
  "count": 1,
  "before": "\t\t\tconst map: Record<string, any> = {};\r\n\t\t\tfor (const [key, value] of searchParams.entries()) {\r\n\t\t\t\tif (arrayFields.includes(key)) (map[key] ??= []).push(value);\r\n\t\t\t\telse map[key] = value;\r\n\t\t\t}\r\n\t\t\tfor (const [key, value] of Object.entries(map)) {\r\n\t\t\t\tinit[key as keyof SearchMangaRequest] = key === \"contentRating\" && Array.isArray(value) ? value.filter(v => v !== \"__none__\") : value;\r\n\t\t\t}\r\n\t\t\treturn init;",
  "after": "\t\t\tconst parsed = parseMangaSearchQuery(init, searchParams, isLibrary);\r\n\t\t\tsearchIssues = parsed.issues; searchPassthrough = parsed.passthrough; explicitHasChapters = parsed.explicitHasChapters; explicitEmptyRating = isLibrary && parsed.explicitEmptyRating;\r\n\t\t\tinitialParsedSearch = structuredClone(parsed.search);\r\n\t\t\treturn parsed.search;"
},
{
  "path": "src/lib/components/MangaSearchFilter.svelte",
  "label": "search Apply validates edits and preserves separate query passthrough",
  "sha256": "f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7",
  "count": 1,
  "before": "\t\tconst q = new URLSearchParams();\r\n\t\tfor (const [key, value] of Object.entries(search)) {\r\n\t\t\tif (value === undefined || value === null || (typeof value === \"string\" && !value.length)) {\r\n\t\t\t\tcontinue;\r\n\t\t\t}\r\n\r\n\t\t\tif (Array.isArray(value)) {\r\n\t\t\t\tvalue.forEach(v => q.append(key, v));\r\n\t\t\t} else {\r\n\t\t\t\tq.set(key, value);\r\n\t\t\t}\r\n\t\t}\r\n",
  "after": "\t\tconst unresolved = unresolvedSearchIssues(searchIssues, initialParsedSearch, $state.snapshot(search));\r\n\t\tconst q = serializeMangaSearchQuery($state.snapshot(search), searchPassthrough, explicitHasChapters, explicitEmptyRating);\r\n\t\tconst validation = searchValidationError(unresolved) ?? validateMangaSearchQuery(q, isLibrary);\r\n\t\tif (validation) { toast.error(validation.title, { description: validation.body }); return; }\r\n\t\tsearchIssues = [];"
},
{
  "path": "src/lib/components/MangaSearchFilter.svelte",
  "label": "explicit false chapter filter remains meaningful",
  "sha256": "f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7",
  "count": 1,
  "before": "if (!search.hasChapters) q.delete(\"hasChapters\");",
  "after": "if (!search.hasChapters && !explicitHasChapters) q.delete(\"hasChapters\");"
},
{
  "path": "src/lib/components/MangaSearchFilter.svelte",
  "label": "Reset explicitly clears rejected query state",
  "sha256": "f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7",
  "count": 1,
  "before": "\t\tsearch = structuredClone(defaultSearch);",
  "after": "\t\tsearchIssues = []; searchPassthrough = new URLSearchParams(); explicitHasChapters = false; explicitEmptyRating = false;\r\n\t\tsearch = structuredClone(defaultSearch);"
},
{
  "path": "src/lib/components/MangaSearch.svelte",
  "label": "search validation uses existing feed error surface before cache restoration",
  "sha256": "29ab147906ba70a2301b96e54c03eb4f454e0c67b43a9bfb8d72a7e2ea930da5",
  "count": 1,
  "before": "\timport { Constants } from \"$lib\";",
  "after": "\timport { Constants } from \"$lib\";\r\n\timport { validateMangaSearchQuery, effectiveMangaSearchReadQuery } from \"$platform/manga-search-query\";"
},
{
  "path": "src/lib/components/MangaSearch.svelte",
  "label": "malformed discovery links do not load replacement results",
  "sha256": "29ab147906ba70a2301b96e54c03eb4f454e0c67b43a9bfb8d72a7e2ea930da5",
  "count": 1,
  "before": "\tonMount(async () => {",
  "after": "\tonMount(async () => {\r\n\t\tif (window.location.pathname === \"/search\" || window.location.pathname === \"/library\") {\r\n\t\t\tconst input = new URLSearchParams(window.location.search), library = window.location.pathname === \"/library\";\r\n\t\t\tconst validation = validateMangaSearchQuery(input, library) ?? effectiveMangaSearchReadQuery(input, library, prefs, $prefs.items_per_page).error;\r\n\t\t\tif (validation) { error = validation; feed = undefined; isMounted = true; return; }\r\n\t\t}"
},
{
  "path": "src/lib/components/AppHeader.svelte",
  "label": "install prompt extensions are narrow runtime-guarded browser members",
  "sha256": "12cbd39322cb42b61df449e7c3b4d9ceb064f3ca67806d5c412689561e2065be",
  "count": 1,
  "before": "\tlet deferredPrompt = $state<Event>();",
  "after": "\timport { isInstallPromptEvent, isStandaloneNavigator, type InstallPromptEvent } from \"$platform/install-prompt\";\r\n\tlet deferredPrompt = $state<InstallPromptEvent>();"
},
{
  "path": "src/lib/components/AppHeader.svelte",
  "label": "Safari standalone is read only when present",
  "sha256": "12cbd39322cb42b61df449e7c3b4d9ceb064f3ca67806d5c412689561e2065be",
  "count": 1,
  "before": "window.navigator.standalone === true",
  "after": "isStandaloneNavigator(window.navigator)"
},
{
  "path": "src/lib/components/AppHeader.svelte",
  "label": "install state only retains callable browser prompt events",
  "sha256": "12cbd39322cb42b61df449e7c3b4d9ceb064f3ca67806d5c412689561e2065be",
  "count": 1,
  "before": "\t\tdeferredPrompt = e;",
  "after": "\t\tif (isInstallPromptEvent(e)) deferredPrompt = e;"
},
{
  "path": "src/lib/components/Lightbox.svelte",
  "label": "Lightbox types only its own exact exposed browser actions",
  "sha256": "0d3f2f488e7d73b8a5d0b84ab3f2c7ea3c0e60d11fa86abe559efe7333a17d76",
  "count": 1,
  "before": "\twindow._lightbox_ = {",
  "after": "\tdeclare global {\r\n\t\tinterface Window {\r\n\t\t\t_lightbox_: { open: typeof open; close: typeof close; prev: typeof prev; next: typeof next };\r\n\t\t}\r\n\t}\r\n\twindow._lightbox_ = {"
},
{
  "path": "src/lib/components/ReportDialog.svelte",
  "label": "optional HTTP reads enter their current acknowledgement failure path",
  "sha256": "12fc7e4392f038dc0cabd22a192208eae8c6e78db2adb06c5c1b730e8c2beca7",
  "count": 1,
  "before": "void API.Report.Reasons(type).then(res => {",
  "after": "void (async () => API.Report.Reasons(type))().then(res => {"
},
{
  "path": "src/routes/(app)/chapter/[id]/edit/+page.svelte",
  "label": "optional HTTP reads enter their current acknowledgement failure path",
  "sha256": "05d8e7dfb6e1e3d145f4a1b9f52d173423feeb11b41733d3a5e9595a25137012",
  "count": 1,
  "before": "void API.User.Get(actor).then(res => {",
  "after": "void (async () => API.User.Get(actor))().then(res => {"
},
{
  "path": "src/routes/(app)/title/[id]/upload/+page.svelte",
  "label": "optional HTTP reads enter their current acknowledgement failure path",
  "sha256": "0625ce874f8b332aec8e742919634676cf58e2b0a8dd78225dcc928425f607d0",
  "count": 1,
  "before": "void API.User.Get(actor).then(res => {",
  "after": "void (async () => API.User.Get(actor))().then(res => {"
},
{
  "path": "src/routes/(app)/search/+page.svelte",
  "label": "actual search read imports its canonical query admission seam",
  "sha256": "f60c7ad8b98c60c557a3d0902f8e2bd4331c40794b1a8a1cad3aab578440392f",
  "count": 1,
  "before": "\timport { StatusCodes } from \"http-status-codes\";",
  "after": "\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { effectiveMangaSearchReadQuery } from \"$platform/manga-search-query\";"
},
{
  "path": "src/routes/(app)/search/+page.svelte",
  "label": "actual search read normalizes explicit fields before preferences and limit defaults",
  "sha256": "f60c7ad8b98c60c557a3d0902f8e2bd4331c40794b1a8a1cad3aab578440392f",
  "count": 1,
  "before": "\t\tconst q = new URLSearchParams(window.location.search);\r\n\t\tprefs.applyMangaPreferences(q);\r\n\r\n\t\tif (!q.has(\"limit\")) q.set(\"limit\", $prefs.items_per_page);",
  "after": "\t\tconst admitted = effectiveMangaSearchReadQuery(new URLSearchParams(window.location.search), false, prefs, $prefs.items_per_page);\r\n\t\tif (admitted.error) throw admitted.error;\r\n\t\tconst q = admitted.query;"
},
{
  "path": "src/routes/(app)/library/+page.svelte",
  "label": "actual search read imports its canonical query admission seam",
  "sha256": "52caf35a0d725e0a5931814880b42a7e4cbc7a6db8f8afa53f51d1c1e9f03272",
  "count": 1,
  "before": "\timport { StatusCodes } from \"http-status-codes\";",
  "after": "\timport { StatusCodes } from \"http-status-codes\";\r\n\timport { effectiveMangaSearchReadQuery } from \"$platform/manga-search-query\";"
},
{
  "path": "src/routes/(app)/library/+page.svelte",
  "label": "actual search read normalizes explicit fields before preferences and limit defaults",
  "sha256": "52caf35a0d725e0a5931814880b42a7e4cbc7a6db8f8afa53f51d1c1e9f03272",
  "count": 1,
  "before": "\t\tconst q = new URLSearchParams(window.location.search);\r\n\r\n\t\tif (!q.has(\"limit\")) q.set(\"limit\", $prefs.items_per_page);",
  "after": "\t\tconst admitted = effectiveMangaSearchReadQuery(new URLSearchParams(window.location.search), true, prefs, $prefs.items_per_page);\r\n\t\tif (admitted.error) throw admitted.error;\r\n\t\tconst q = admitted.query;"
},
{
  "path": "src/lib/components/MangaSearchFilter.svelte",
  "label": "search Apply validates the final wire after sentinel and default-key mutations",
  "sha256": "f3628652c45bc03b00f0115e1a7ca135d5ccd990e1543bf9326e546c64bca4b7",
  "count": 1,
  "before": "\t\tconst target = window.location.pathname + q.toString();",
  "after": "\t\tconst finalValidation = validateMangaSearchQuery(q, isLibrary);\r\n\t\tif (finalValidation) { toast.error(finalValidation.title, { description: finalValidation.body }); return; }\r\n\t\tconst target = window.location.pathname + q.toString();"
},
  {
    "path": "src/routes/(app)/group/[id]/Group.svelte",
    "label": "group follow status follows current actor and target readiness",
    "sha256": "ece279c948c849fb298992cc883b3dbb4492e2d5748acdee430fed276ae3dfcc",
    "count": 1,
    "before": "\tlet isFollowed = $state(false);",
    "after": "\tconst followState = createActorFollowState({\r\n\t\towner: preferenceOwner, mutex,\r\n\t\ttarget: () => $group.id, lifetime: () => page.url.href,\r\n\t\tcanMutate: () => !isBlocked,\r\n\t\tread: target => API.Group.GetFollowStatus(target),\r\n\t\twrite: (target, followed, intent) => (followed ? API.Group.Follow : API.Group.Unfollow)(target, intent),\r\n\t\tonError: cause => { const error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; toast.error(error.title, { description: error.body }); }\r\n\t});\r\n\tlet isFollowed = $derived($followState.followed);\r\n\t$effect.pre(() => { $group.id; page.url.href; followState.selectTarget(); });"
  },
  {
    "path": "src/routes/(app)/group/[id]/Group.svelte",
    "label": "group mounted public chapter count is independent of actor readiness",
    "sha256": "ece279c948c849fb298992cc883b3dbb4492e2d5748acdee430fed276ae3dfcc",
    "count": 1,
    "before": "await Promise.all([getFollowStatus(), getChaptersCount()]);",
    "after": "await getChaptersCount();"
  },
  {
    "path": "src/routes/(app)/group/[id]/Group.svelte",
    "label": "group follow progress reflects current read and mutation only",
    "sha256": "ece279c948c849fb298992cc883b3dbb4492e2d5748acdee430fed276ae3dfcc",
    "count": 1,
    "before": "\tlet isUpdatingFollowStatus = $state(false);",
    "after": "\tlet isUpdatingFollowStatus = $derived($followState.loading || $followState.updating);"
  },
  {
    "path": "src/routes/(app)/group/[id]/Group.svelte",
    "label": "group follow control waits for known current status or offers read retry",
    "sha256": "ece279c948c849fb298992cc883b3dbb4492e2d5748acdee430fed276ae3dfcc",
    "count": 1,
    "before": "onclick={updateFollowStatus}\r\n\t\t\t\t\t\t\t\tdisabled={$mutex}",
    "after": "onclick={updateFollowStatus}\r\n\t\t\t\t\t\t\t\tdisabled={$mutex || $followState.loading || $followState.updating || (!$followState.ready && !$followState.error)}",
    "approved_follow_readiness_exception": "follow_current_status_disabled"
  },
  {
    "path": "src/routes/(app)/group/[id]/Group.svelte",
    "label": "group unknown follow status offers explicit read-only retry",
    "sha256": "ece279c948c849fb298992cc883b3dbb4492e2d5748acdee430fed276ae3dfcc",
    "count": 1,
    "before": "{#if isFollowed}\r\n\t\t\t\t\t\t\t\t\tUnfollow",
    "after": "{#if $followState.error}\r\n\t\t\t\t\t\t\t\t\tRetry follow status\r\n\t\t\t\t\t\t\t\t{:else if isFollowed}\r\n\t\t\t\t\t\t\t\t\tUnfollow",
    "approved_follow_readiness_exception": "follow_current_status_retry"
  },
  {
    "path": "src/routes/(app)/user/[id]/User.svelte",
    "label": "user follow status follows current actor and target readiness",
    "sha256": "f8232429c11dcc2dc08fb34f1c3fc31d262cc547b4becf485b884133ed6db0bd",
    "count": 1,
    "before": "\tlet isFollowed = $state(false);",
    "after": "\tconst followState = createActorFollowState({\r\n\t\towner: preferenceOwner, mutex,\r\n\t\ttarget: () => $user.id, lifetime: () => page.url.href,\r\n\t\tcanMutate: () => !isBlocked && $user.id !== $Session?.identity.id,\r\n\t\tread: target => API.User.GetFollowStatus(target),\r\n\t\twrite: (target, followed, intent) => (followed ? API.User.Follow : API.User.Unfollow)(target, intent),\r\n\t\tonError: cause => { const error = cause instanceof Constants.Error.Error ? cause : Constants.Error.Unknown; toast.error(error.title, { description: error.body }); }\r\n\t});\r\n\tlet isFollowed = $derived($followState.followed);\r\n\t$effect.pre(() => { $user.id; page.url.href; followState.selectTarget(); });"
  },
  {
    "path": "src/routes/(app)/user/[id]/User.svelte",
    "label": "user mounted public chapter count is independent of actor readiness",
    "sha256": "f8232429c11dcc2dc08fb34f1c3fc31d262cc547b4becf485b884133ed6db0bd",
    "count": 1,
    "before": "await Promise.all([getFollowStatus(), getChaptersCount()]);",
    "after": "await getChaptersCount();"
  },
  {
    "path": "src/routes/(app)/user/[id]/User.svelte",
    "label": "user follow progress reflects current read and mutation only",
    "sha256": "f8232429c11dcc2dc08fb34f1c3fc31d262cc547b4becf485b884133ed6db0bd",
    "count": 1,
    "before": "\tlet isUpdatingFollowStatus = $state(false);",
    "after": "\tlet isUpdatingFollowStatus = $derived($followState.loading || $followState.updating);"
  },
  {
    "path": "src/routes/(app)/user/[id]/User.svelte",
    "label": "user follow control waits for known current status or offers read retry",
    "sha256": "f8232429c11dcc2dc08fb34f1c3fc31d262cc547b4becf485b884133ed6db0bd",
    "count": 1,
    "before": "onclick={updateFollowStatus}\r\n\t\t\t\t\t\t\t\tdisabled={$mutex}",
    "after": "onclick={updateFollowStatus}\r\n\t\t\t\t\t\t\t\tdisabled={$mutex || $followState.loading || $followState.updating || (!$followState.ready && !$followState.error)}",
    "approved_follow_readiness_exception": "follow_current_status_disabled"
  },
  {
    "path": "src/routes/(app)/user/[id]/User.svelte",
    "label": "user unknown follow status offers explicit read-only retry",
    "sha256": "f8232429c11dcc2dc08fb34f1c3fc31d262cc547b4becf485b884133ed6db0bd",
    "count": 1,
    "before": "{#if isFollowed}\r\n\t\t\t\t\t\t\t\t\tUnfollow",
    "after": "{#if $followState.error}\r\n\t\t\t\t\t\t\t\t\tRetry follow status\r\n\t\t\t\t\t\t\t\t{:else if isFollowed}\r\n\t\t\t\t\t\t\t\t\tUnfollow",
    "approved_follow_readiness_exception": "follow_current_status_retry"
  },
  {
    "path": "src/routes/(app)/group/[id]/Group.svelte",
    "label": "group unknown follow state cannot expose a new Block action",
    "sha256": "ece279c948c849fb298992cc883b3dbb4492e2d5748acdee430fed276ae3dfcc",
    "count": 1,
    "before": "{#if !isFollowed}",
    "after": "{#if isBlocked || ($followState.ready && !isFollowed)}",
    "approved_follow_readiness_exception": "follow_current_status_block_guard"
  },
  {
    "path": "src/routes/(app)/user/[id]/User.svelte",
    "label": "user unknown follow state cannot expose a new Block action",
    "sha256": "f8232429c11dcc2dc08fb34f1c3fc31d262cc547b4becf485b884133ed6db0bd",
    "count": 1,
    "before": "{#if !isFollowed}",
    "after": "{#if isBlocked || ($followState.ready && !isFollowed)}",
    "approved_follow_readiness_exception": "follow_current_status_block_guard"
  },
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "file gestures capture mutable owner staging before taking their shared mutex",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "\tlet targetFile = $state<UploadSessionFile>();",
  "after": "\t// Parent disabled includes this same mutex. Admit the gesture before locking,\r\n\t// then retain exact owner/session authority across every asynchronous decode.\r\n\tconst assertFileSession = (session: UploadSession) => {\r\n\t\towner.assert();\r\n\t\tif (uploadSession !== session || session.can_mutate === false) throw new Constants.Error.Error(\"Upload changed\", \"Old file selection was discarded. No mutation was replayed.\");\r\n\t};\r\n\tconst fileSession = () => !disabled && !mutex.isLocked() && uploadSession && uploadSession.can_mutate !== false ? uploadSession : undefined;\r\n\tlet targetFile = $state<UploadSessionFile>();"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "replacement checks captured staging instead of rejecting its own mutex",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "const replaceFile = async (file: File) => {\r\n\t\tif (!targetFile || !uploadSession || uploadSession.can_mutate === false || disabled) return;",
  "after": "const replaceFile = async (file: File, session: UploadSession) => {\r\n\t\tassertFileSession(session);\r\n\t\tif (!targetFile) return;"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "replacement cannot apply a late page reconciliation to another session",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "target = await resolveUploadPage(uploadSession, target, owner); owner.assert(); pages[idx] = target;",
  "after": "target = await resolveUploadPage(session, target, owner); assertFileSession(session); pages[idx] = target;"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "replacement rechecks staging after image header read",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "const actual = header.length >= 8",
  "after": "assertFileSession(session);\r\n\t\tconst actual = header.length >= 8"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "replacement rechecks staging after image decoding",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "const decoded = await decodeUploadImage(file);\r\n\t\tif (decoded.width",
  "after": "const decoded = await decodeUploadImage(file); assertFileSession(session);\r\n\t\tif (decoded.width"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "replacement validates captured staging after authority read",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "const current = await owner.request<UploadSession>(\"/upload\", \"GET\");\r\n\t\tif (current?.status",
  "after": "const current = await owner.request<UploadSession>(\"/upload\", \"GET\"); assertFileSession(session);\r\n\t\tif (current?.status"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "replacement rechecks staging after digest before atomic mutation",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "const form = new FormData(); form.append(\"file\", file);",
  "after": "assertFileSession(session);\r\n\t\tconst form = new FormData(); form.append(\"file\", file);"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "replacement cannot apply an acknowledged receipt after staging changed",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "const result = await http.POST(); owner.assert();",
  "after": "const result = await http.POST(); assertFileSession(session);"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "replacement reconciliation remains bound to captured staging",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "const latest = await owner.request<UploadSession>(\"/upload\", \"GET\").catch(() => undefined); owner.assert(); const confirmed",
  "after": "const latest = await owner.request<UploadSession>(\"/upload\", \"GET\").catch(() => undefined); assertFileSession(session); const confirmed"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "file processing retains captured staging while its own mutex is locked",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "const processFiles = async (files: File[]) => {\r\n\t\tif (!uploadSession || uploadSession.can_mutate === false || disabled) return;",
  "after": "const processFiles = async (files: File[], session: UploadSession) => {\r\n\t\tassertFileSession(session);"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "file processing rechecks staging after decoding and splitting",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "const decoded = await decodeUploadImage(file);\r\n\t\t\tconst pieces = decoded.width > maxImageWidth || decoded.height > maxImageHeight ? await splitImage(decoded, file.name) : [file];",
  "after": "const decoded = await decodeUploadImage(file); assertFileSession(session);\r\n\t\t\tconst pieces = decoded.width > maxImageWidth || decoded.height > maxImageHeight ? await splitImage(decoded, file.name) : [file];\r\n\t\t\tassertFileSession(session);"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "file processing rechecks staging after archive extraction",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "const extracted = await extractArchive(file, remaining, maxSessionFiles - pages.length - processedFiles.length);",
  "after": "const extracted = await extractArchive(file, remaining, maxSessionFiles - pages.length - processedFiles.length); assertFileSession(session);"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "file processing rechecks staging before exposing candidates",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "const newFiles = processedFiles.map(createPage);",
  "after": "assertFileSession(session);\r\n\t\tconst newFiles = processedFiles.map(createPage);"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "each admitted chunk remains bound to the captured session",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "try { await uploadChunk(chunk, delta => { progress.loaded += delta; }); }",
  "after": "try { assertFileSession(session); await uploadChunk(chunk, delta => { progress.loaded += delta; }); assertFileSession(session); }"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "stale file failures cannot annotate replacement staging",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "if (owner.alive()) { markUnsentUploadPages(newFiles, uploadSession.id);",
  "after": "if (owner.alive() && uploadSession === session) { markUnsentUploadPages(newFiles, session.id);"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "input admission happens before the shared mutex is taken",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "\tconst onInputChange = (ev: Event) =>\r\n\t\tmutex.exec(async () => {",
  "after": "\tconst onInputChange = (ev: Event) => {\r\n\t\tconst session = fileSession();\r\n\t\tif (!session) { targetFile = undefined; (ev.target as HTMLInputElement).value = \"\"; return; }\r\n\t\treturn mutex.exec(async () => {"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "input add and replace retain the admitted session",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "if (targetFile) await replaceFile(target.files[0]);\r\n\t\t\t\t\telse await processFiles(Array.from(target.files));",
  "after": "if (targetFile) await replaceFile(target.files[0], session);\r\n\t\t\t\t\telse await processFiles(Array.from(target.files), session);"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "input admission closes before drop admission",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "\t\t});\r\n\r\n\tconst onDrop = (ev: DragEvent) => {",
  "after": "\t\t});\r\n\t};\r\n\r\n\tconst onDrop = (ev: DragEvent) => {"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "drop admission happens before the shared mutex is taken",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "\t\tev.preventDefault();\r\n\t\treturn mutex.exec(async () => {",
  "after": "\t\tev.preventDefault();\r\n\t\tconst session = fileSession();\r\n\t\tif (!session) return;\r\n\t\treturn mutex.exec(async () => {"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "folder and plain drops retain the admitted session",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "await processFiles(files.flat().filter(e => e instanceof File));",
  "after": "assertFileSession(session);\r\n\t\t\t\t\tawait processFiles(files.flat().filter(e => e instanceof File), session);"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "label": "plain drops retain the admitted session",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "before": "await processFiles(Array.from(ev.dataTransfer.files));",
  "after": "await processFiles(Array.from(ev.dataTransfer.files), session);"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "label": "batch results must retain the exact admitted staging object",
  "before": "if (uploadSession?.id !== session.id) throw",
  "after": "if (uploadSession !== session || session.can_mutate === false) throw"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "label": "local replacement failure cannot restore a row into changed staging",
  "before": "try { await uploadChunk([candidate]); revokeThumbnail(target); }\r\n\t\t\tcatch (error) { if (owner.alive())",
  "after": "try { await uploadChunk([candidate]); assertFileSession(session); revokeThumbnail(target); }\r\n\t\t\tcatch (error) { if (owner.alive() && uploadSession === session)"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "label": "batch owner retains exact admitted staging through authority reads and hashing",
  "before": "const result = await runUploadBatch(session, chunk, owner, onProgress); owner.assert();",
  "after": "const result = await runUploadBatch(session, chunk, { ...owner, assert: () => assertFileSession(session), alive: () => owner.alive() && uploadSession === session && session.can_mutate !== false }, onProgress); assertFileSession(session);"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "label": "obsolete file input cannot attach feedback to newer editor authority",
  "before": "\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t} finally {\r\n\t\t\t\ttargetFile = undefined;",
  "after": "\t\t\t} catch (e) {\r\n\t\t\t\tif (!owner.alive() || uploadSession !== session) return;\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t} finally {\r\n\t\t\t\ttargetFile = undefined;"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "label": "obsolete file drop cannot attach feedback to newer editor authority",
  "before": "\t\t\t} catch (e) {\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t}\r\n\t\t});\r\n\t};\r\n\r\n\tlet isDeletingPages",
  "after": "\t\t\t} catch (e) {\r\n\t\t\t\tif (!owner.alive() || uploadSession !== session) return;\r\n\t\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\t\telse {\r\n\t\t\t\t\tconsole.error(e);\r\n\t\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t\t}\r\n\t\t\t}\r\n\t\t});\r\n\t};\r\n\r\n\tlet isDeletingPages"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "label": "replacement authority compares the guarded captured staging ID",
  "before": "current.body?.id !== uploadSession.id || current.body.can_mutate === false",
  "after": "current.body?.id !== session.id || current.body.can_mutate === false"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "label": "atomic replacement targets the guarded captured staging ID",
  "before": "`${PUBLIC_API_URL}/upload/${uploadSession.id}/files/${target.id}/replace`",
  "after": "`${PUBLIC_API_URL}/upload/${session.id}/files/${target.id}/replace`"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "label": "replacement reconciliation compares the guarded captured staging ID",
  "before": "latest.body?.id === uploadSession.id && confirmed?.file_hash === sha",
  "after": "latest.body?.id === session.id && confirmed?.file_hash === sha"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "label": "replacement acknowledgement updates the guarded captured staging version",
  "before": "pages[idx] = replacement; pages = pages; uploadSession.version = version;",
  "after": "pages[idx] = replacement; pages = pages; session.version = version;"
},
{
  "path": "src/lib/components/upload/UploadChapterPages.svelte",
  "sha256": "b10b1a339cfd91dcb9c5af0ccc8e20f1dfce4e2cb2d6fc3c3b0286dc22904e66",
  "count": 1,
  "label": "upload candidates share reactive identity between queued mutations and rendered controls",
  "before": "\tconst createPage = (file: File) => {\r\n\t\treturn <UploadSessionFile>{\r\n\t\t\tid: crypto.randomUUID(),\r\n\t\t\toriginal_file_name: file.name,\r\n\r\n\t\t\tfile: file,\r\n\t\t\tthumbnail: URL.createObjectURL(file)\r\n\t\t};\r\n\t};",
  "after": "\tconst createPage = (file: File) => {\r\n\t\t// The upload queue and rendered array must share this exact reactive row.\r\n\t\t// Mutating a pre-proxy raw row after an awaited request is invisible.\r\n\t\tconst candidate = $state(<UploadSessionFile>{\r\n\t\t\tid: crypto.randomUUID(),\r\n\t\t\toriginal_file_name: file.name,\r\n\r\n\t\t\tfile: file,\r\n\t\t\tthumbnail: URL.createObjectURL(file)\r\n\t\t});\r\n\t\treturn candidate;\r\n\t};"
},
  {
    "path": "src/routes/+layout.svelte",
    "label": "one renderless library lifecycle across app and standalone reader",
    "sha256": "734f8e432add763b3557327eeb60927ca0edaa2d3c1e583d8079b552d00287b7",
    "count": 1,
    "before": "\timport { onMount } from \"svelte\";",
    "after": "\timport { onMount } from \"svelte\";\n\timport LibraryRuntime from \"$localLibrary/LibraryRuntime.svelte\";"
  },
  {
    "path": "src/routes/+layout.svelte",
    "label": "restore device state and save error reporting without management UI",
    "sha256": "734f8e432add763b3557327eeb60927ca0edaa2d3c1e583d8079b552d00287b7",
    "count": 1,
    "before": "<Lightbox.Provider>",
    "after": "<LibraryRuntime />\n\n<Lightbox.Provider>",
    "approved_ui_extension": "route_owned_library_identity"
  },
  {
    "path": "src/routes/(app)/library/+layout.svelte",
    "label": "library management action uses native Button",
    "sha256": "feb81ccc3e21e0663bd0fd3fe536fc775980fbdc66d2e53b390114f5523ad59d",
    "count": 1,
    "before": "\timport Protected from \"$lib/components/ui/Protected.svelte\";",
    "after": "\timport Protected from \"$lib/components/ui/Protected.svelte\";\r\n\timport { Button } from \"$lib/components/ui/button\";"
  },
  {
  "path": "src/routes/(app)/library/+layout.svelte",
  "label": "management stays reachable in account and failed-scope fallback",
  "sha256": "feb81ccc3e21e0663bd0fd3fe536fc775980fbdc66d2e53b390114f5523ad59d",
  "count": 1,
  "before": "<Protected>",
  "after": "<div class=\"mb-4 flex justify-end\">\r\n\t<Button variant=\"outline\" size=\"sm\" href=\"/settings/library\">Manage library</Button>\r\n</div>\r\n\r\n<Protected>",
  "approved_ui_extension": "route_owned_library_identity"
},
  {
    "path": "src/routes/(app)/title/Manga.svelte",
    "label": "title discussion owns its source-bound signed actions",
    "sha256": "960e709e68d2d0919580f8d38f058d45a5b4fcde66aed7cfe61ee699814920ff",
    "count": 1,
    "before": "\timport * as Comments from \"$lib/components/comments\";",
    "after": "\timport MangaDiscussion from \"$localLibrary/MangaDiscussion.svelte\";"
  },
  {
    "path": "src/routes/(app)/title/Manga.svelte",
    "label": "signed conversation remains available without native account thread",
    "sha256": "960e709e68d2d0919580f8d38f058d45a5b4fcde66aed7cfe61ee699814920ff",
    "count": 1,
    "before": "\t\t\tcomponent: Comments.Comments,\r\n\t\t\thidden: !isPublished || !manga.relationships.thread",
    "after": "\t\t\tcomponent: MangaDiscussion,\r\n\t\t\thidden: !isPublished"
  },
  {
    "path": "src/lib/components/reader/reader.svelte",
    "label": "reader discussion imports contextual signed actions",
    "sha256": "0b5ea1b1d3cb2ffbf818117e8f72d46218cd60b9b72e1539b864fb082e7ccf1d",
    "count": 1,
    "before": "\timport * as Comments from \"../comments\";",
    "after": "\timport * as Comments from \"../comments\";\r\n\timport ProtocolIdentityPanel from \"$localLibrary/ProtocolIdentityPanel.svelte\";"
  },
  {
    "path": "src/lib/components/reader/reader.svelte",
    "label": "signed comments belong below reading surface in discussion area",
    "sha256": "0b5ea1b1d3cb2ffbf818117e8f72d46218cd60b9b72e1539b864fb082e7ccf1d",
    "count": 1,
    "before": "\t\t<Comments.Comments class=\"m-auto max-w-300\" />",
    "after": "\t\t<div class=\"m-auto max-w-300 space-y-5\">\r\n\t\t\t{#if reader.chapter.relationships.thread}<Comments.Comments />{/if}\r\n\t\t\t<ProtocolIdentityPanel resourceKind=\"chapter\" resourceId={reader.chapter.id} />\r\n\t\t</div>",
    "approved_ui_extension": "route_owned_library_identity"
  },
  {
    "path": "src/routes/(app)/chapter/[id]/+page@.svelte",
    "label": "reader supports source discussion independently of native thread",
    "sha256": "89b6f7220a6e1638285b2b0f2ebbf8bad343fa74f3a786811a637359195f510a",
    "count": 1,
    "before": "\t\t<Comments.Provider thread={data.chapter.relationships.thread}>\r\n\t\t\t<Reader.Provider chapter={data.chapter}>\r\n\t\t\t\t<Reader.Reader />\r\n\t\t\t</Reader.Provider>\r\n\t\t</Comments.Provider>",
    "after": "\t\t{#snippet reading(chapter: Chapter)}\r\n\t\t\t<Reader.Provider {chapter}>\r\n\t\t\t\t<Reader.Reader />\r\n\t\t\t</Reader.Provider>\r\n\t\t{/snippet}\r\n\t\t{#if data.chapter.relationships.thread}\r\n\t\t\t<Comments.Provider thread={data.chapter.relationships.thread}>{@render reading(data.chapter)}</Comments.Provider>\r\n\t\t{:else}\r\n\t\t\t{@render reading(data.chapter)}\r\n\t\t{/if}",
    "approved_ui_extension": "route_owned_library_identity"
  },
  {
    "path": "src/routes/(app)/chapter/[id]/[page]/+page@.svelte",
    "label": "reader supports source discussion independently of native thread",
    "sha256": "89b6f7220a6e1638285b2b0f2ebbf8bad343fa74f3a786811a637359195f510a",
    "count": 1,
    "before": "\t\t<Comments.Provider thread={data.chapter.relationships.thread}>\r\n\t\t\t<Reader.Provider chapter={data.chapter}>\r\n\t\t\t\t<Reader.Reader />\r\n\t\t\t</Reader.Provider>\r\n\t\t</Comments.Provider>",
    "after": "\t\t{#snippet reading(chapter: Chapter)}\r\n\t\t\t<Reader.Provider {chapter}>\r\n\t\t\t\t<Reader.Reader />\r\n\t\t\t</Reader.Provider>\r\n\t\t{/snippet}\r\n\t\t{#if data.chapter.relationships.thread}\r\n\t\t\t<Comments.Provider thread={data.chapter.relationships.thread}>{@render reading(data.chapter)}</Comments.Provider>\r\n\t\t{:else}\r\n\t\t\t{@render reading(data.chapter)}\r\n\t\t{/if}",
    "approved_ui_extension": "route_owned_library_identity"
  },
  {
    "path": "src/lib/components/reader/reader-panel.svelte",
    "label": "reader native votes and thread link require native discussion",
    "sha256": "6fc6e2c1432d40d0d9c6069d41c751d9cc286ac4f26398d4750cab7e9e2d34fa",
    "count": 1,
    "before": "\t\t\t\t\t\t<div class=\"flex items-center gap-1\">\r\n\t\t\t\t\t\t\t<Comments.Votes />\r\n\t\t\t\t\t\t\t<a\r\n\t\t\t\t\t\t\t\tclass={buttonVariants({ class: \"flex-auto border-none\", variant: \"outline\" })}\r\n\t\t\t\t\t\t\t\thref=\"/forums/threads/{comments.thread.id}\"\r\n\t\t\t\t\t\t\t>\r\n\t\t\t\t\t\t\t\t<MessagesSquareIcon />\r\n\t\t\t\t\t\t\t\t{#if comments.stats?.replies}\r\n\t\t\t\t\t\t\t\t\t{comments.stats.replies.toLocaleString()}\r\n\t\t\t\t\t\t\t\t\t{pluralize(\"Comment\", comments.stats.replies)}\r\n\t\t\t\t\t\t\t\t{:else}\r\n\t\t\t\t\t\t\t\t\tNo comments yet\r\n\t\t\t\t\t\t\t\t{/if}\r\n\t\t\t\t\t\t\t</a>\r\n\t\t\t\t\t\t</div>",
    "after": "\t\t\t\t\t\t{#if reader.chapter.relationships.thread}\r\n\t\t\t\t\t\t<div class=\"flex items-center gap-1\">\r\n\t\t\t\t\t\t\t<Comments.Votes />\r\n\t\t\t\t\t\t\t<a\r\n\t\t\t\t\t\t\t\tclass={buttonVariants({ class: \"flex-auto border-none\", variant: \"outline\" })}\r\n\t\t\t\t\t\t\t\thref=\"/forums/threads/{comments.thread.id}\"\r\n\t\t\t\t\t\t\t>\r\n\t\t\t\t\t\t\t\t<MessagesSquareIcon />\r\n\t\t\t\t\t\t\t\t{#if comments.stats?.replies}\r\n\t\t\t\t\t\t\t\t\t{comments.stats.replies.toLocaleString()}\r\n\t\t\t\t\t\t\t\t\t{pluralize(\"Comment\", comments.stats.replies)}\r\n\t\t\t\t\t\t\t\t{:else}\r\n\t\t\t\t\t\t\t\t\tNo comments yet\r\n\t\t\t\t\t\t\t\t{/if}\r\n\t\t\t\t\t\t\t</a>\r\n\t\t\t\t\t\t</div>\r\n\t\t\t\t\t\t{/if}",
    "approved_ui_extension": "route_owned_library_identity"
  },
  {
    "path": "src/lib/components/reader/reader-last-page.svelte",
    "label": "both end-page native vote controls require native discussion",
    "sha256": "a95caf71008d455ea1d888cabe209e27273e3ddb3982ff6f7b8f2f0aac949b7e",
    "count": 2,
    "before": "<Comments.Votes />",
    "after": "{#if reader.chapter.relationships.thread}<Comments.Votes />{/if}",
    "approved_ui_extension": "route_owned_library_identity"
  },
  {
    "path": "src/lib/components/reader/reader-last-page.svelte",
    "label": "end-page native thread link guard 1",
    "sha256": "a95caf71008d455ea1d888cabe209e27273e3ddb3982ff6f7b8f2f0aac949b7e",
    "count": 1,
    "before": "<a\r\n\t\t\t\tclass={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"sm\" })}\r\n\t\t\t\thref=\"/forums/threads/{reader.chapter.relationships.thread.id}\"\r\n\t\t\t\tonclick={e => {\r\n\t\t\t\t\te.stopPropagation();\r\n\t\t\t\t\te.stopImmediatePropagation();\r\n\t\t\t\t}}\r\n\t\t\t>\r\n\t\t\t\t<MessagesSquareIcon class=\"shrink-0\" />\r\n\t\t\t\t{#if comments.stats?.replies}\r\n\t\t\t\t\t{comments.stats.replies.toLocaleString()}\r\n\t\t\t\t\t{pluralize(\"Comment\", comments.stats.replies)}\r\n\t\t\t\t{:else}\r\n\t\t\t\t\tGo to comments\r\n\t\t\t\t{/if}\r\n\t\t\t</a>",
    "after": "{#if reader.chapter.relationships.thread}<a\r\n\t\t\t\tclass={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"sm\" })}\r\n\t\t\t\thref=\"/forums/threads/{reader.chapter.relationships.thread.id}\"\r\n\t\t\t\tonclick={e => {\r\n\t\t\t\t\te.stopPropagation();\r\n\t\t\t\t\te.stopImmediatePropagation();\r\n\t\t\t\t}}\r\n\t\t\t>\r\n\t\t\t\t<MessagesSquareIcon class=\"shrink-0\" />\r\n\t\t\t\t{#if comments.stats?.replies}\r\n\t\t\t\t\t{comments.stats.replies.toLocaleString()}\r\n\t\t\t\t\t{pluralize(\"Comment\", comments.stats.replies)}\r\n\t\t\t\t{:else}\r\n\t\t\t\t\tGo to comments\r\n\t\t\t\t{/if}\r\n\t\t\t</a>{/if}",
    "approved_ui_extension": "route_owned_library_identity"
  },
  {
    "path": "src/lib/components/reader/reader-last-page.svelte",
    "label": "end-page native thread link guard 2",
    "sha256": "a95caf71008d455ea1d888cabe209e27273e3ddb3982ff6f7b8f2f0aac949b7e",
    "count": 1,
    "before": "<a\r\n\t\t\t\t\tclass={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"sm\" })}\r\n\t\t\t\t\thref=\"/forums/threads/{comments.thread.id}\"\r\n\t\t\t\t\tonclick={e => {\r\n\t\t\t\t\t\te.stopPropagation();\r\n\t\t\t\t\t\te.stopImmediatePropagation();\r\n\t\t\t\t\t}}\r\n\t\t\t\t>\r\n\t\t\t\t\t<MessagesSquareIcon class=\"shrink-0\" />\r\n\t\t\t\t\t{#if comments.stats?.replies}\r\n\t\t\t\t\t\t{comments.stats.replies}\r\n\t\t\t\t\t\t{pluralize(\"Comment\", comments.stats.replies)}\r\n\t\t\t\t\t{:else}\r\n\t\t\t\t\t\tGo to comments\r\n\t\t\t\t\t{/if}\r\n\t\t\t\t</a>",
    "after": "{#if reader.chapter.relationships.thread}<a\r\n\t\t\t\t\tclass={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"sm\" })}\r\n\t\t\t\t\thref=\"/forums/threads/{comments.thread.id}\"\r\n\t\t\t\t\tonclick={e => {\r\n\t\t\t\t\t\te.stopPropagation();\r\n\t\t\t\t\t\te.stopImmediatePropagation();\r\n\t\t\t\t\t}}\r\n\t\t\t\t>\r\n\t\t\t\t\t<MessagesSquareIcon class=\"shrink-0\" />\r\n\t\t\t\t\t{#if comments.stats?.replies}\r\n\t\t\t\t\t\t{comments.stats.replies}\r\n\t\t\t\t\t\t{pluralize(\"Comment\", comments.stats.replies)}\r\n\t\t\t\t\t{:else}\r\n\t\t\t\t\t\tGo to comments\r\n\t\t\t\t\t{/if}\r\n\t\t\t\t</a>{/if}",
    "approved_ui_extension": "route_owned_library_identity"
  },
  {
  "path": "src/lib/components/reader/reader-settings.svelte",
  "label": "reader settings links to anonymous library management",
  "sha256": "414271f285f269a15f9f171d41e11c96456cf498aa63805916728b7de6e54b90",
  "count": 1,
  "before": ">\r\n\t\t<Label for=\"skip_end_of_chapter_page\">Skip End of Chapter Page</Label>\r\n\t\t<Switch id=\"skip_end_of_chapter_page\" bind:checked={$skipEndOfChapterPage} />\r\n\t</div>\r\n</div>\r\n",
  "after": ">\r\n\t\t<Label for=\"skip_end_of_chapter_page\">Skip End of Chapter Page</Label>\r\n\t\t<Switch id=\"skip_end_of_chapter_page\" bind:checked={$skipEndOfChapterPage} />\r\n\t</div>\r\n</div>\r\n\r\n<Separator />\r\n<div class=\"space-y-2 p-2\">\r\n\t<Button class=\"w-full border-none\" variant=\"outline\" href=\"/settings/library\">Manage library</Button>\r\n</div>\r\n",
  "approved_ui_extension": "route_owned_library_identity"
},
  {
  "path": "src/routes/(app)/settings/+layout.svelte",
  "label": "anonymous Library and Public identity use native Settings tabs",
  "sha256": "59ebe00598167420b7b5ecea5f36214373f06e436e263f7aa7015f11b268574a",
  "count": 1,
  "before": "\t\tlibrary: {\r\n\t\t\tname: \"Library\",\r\n\t\t\ticon: LibraryIcon,\r\n\t\t\thidden: !$Session\r\n\t\t},",
  "after": "\t\tlibrary: {\r\n\t\t\tname: \"Library\",\r\n\t\t\ticon: LibraryIcon\r\n\t\t},\r\n\t\tidentity: {\r\n\t\t\tname: \"Public identity\",\r\n\t\t\ticon: SettingsIcon\r\n\t\t},"
},
  {
  "path": "src/routes/(app)/settings/+layout.svelte",
  "label": "native Settings tabs expose current page accessibly",
  "sha256": "59ebe00598167420b7b5ecea5f36214373f06e436e263f7aa7015f11b268574a",
  "count": 1,
  "before": "\t\t\t\t\t\thref={pathname}",
  "after": "\t\t\t\t\t\thref={pathname}\r\n\t\t\t\t\t\taria-current={page.url.pathname === pathname ? \"page\" : undefined}",
  "approved_ui_extension": "route_owned_library_identity"
},
  {
  "path": "src/routes/(app)/settings/library/+layout.svelte",
  "label": "library management does not inherit native account guard",
  "sha256": "feb81ccc3e21e0663bd0fd3fe536fc775980fbdc66d2e53b390114f5523ad59d",
  "count": 1,
  "before": "\timport Protected from \"$lib/components/ui/Protected.svelte\";",
  "after": ""
},
  {
  "path": "src/routes/(app)/settings/library/+layout.svelte",
  "label": "anonymous management renders independently of mode storage and scope",
  "sha256": "feb81ccc3e21e0663bd0fd3fe536fc775980fbdc66d2e53b390114f5523ad59d",
  "count": 1,
  "before": "<Protected>\r\n\t{@render children?.()}\r\n</Protected>",
  "after": "{@render children?.()}",
  "approved_ui_extension": "route_owned_library_identity"
},
  {
  "path": "src/routes/(app)/settings/library/+page.svelte",
  "label": "Library settings owns anonymous device management",
  "sha256": "7ca6181b34ecb147d3d079beed6f309575b459c04b988f46d3bea3a63dc3ed88",
  "count": 1,
  "before": "\timport { PUBLIC_API_URL, PUBLIC_SITE_NAME } from \"$env/static/public\";",
  "after": "\timport { PUBLIC_API_URL, PUBLIC_SITE_NAME } from \"$env/static/public\";\r\n\timport { Session } from \"$lib/stores\";\r\n\timport { localLibraryState } from \"$localLibrary/library\";\r\n\timport LocalLibraryPanel from \"$localLibrary/LocalLibraryPanel.svelte\";"
},
  {
  "path": "src/routes/(app)/settings/library/+page.svelte",
  "label": "native import export keeps explicit selected device or account destination",
  "sha256": "7ca6181b34ecb147d3d079beed6f309575b459c04b988f46d3bea3a63dc3ed88",
  "count": 1,
  "before": "<input class=\"hidden\" type=\"file\" bind:this={importRef} onchange={onImportChange} />",
  "after": "<LocalLibraryPanel />\r\n<Separator class=\"my-5\" />\r\n<section class=\"mb-5 space-y-1\">\r\n\t<h2 class=\"font-semibold\">CSV and MyAnimeList files</h2>\r\n\t<p class=\"text-muted-foreground text-sm\">{$localLibraryState.mode === \"device\" ? \"These imports and CSV exports use this device library.\" : \"These imports and CSV exports use your signed-in account library.\"}</p>\r\n\t{#if $localLibraryState.mode === \"account\" && !$Session}<p class=\"text-muted-foreground text-sm\">Sign in for account import and export, or choose Use device library above.</p>{/if}\r\n</section>\r\n<input class=\"hidden\" type=\"file\" bind:this={importRef} onchange={onImportChange} />",
  "approved_ui_extension": "route_owned_library_identity"
},
  {
  "path": "src/routes/(app)/settings/library/+page.svelte",
  "label": "account file actions require login without hiding device management",
  "sha256": "7ca6181b34ecb147d3d079beed6f309575b459c04b988f46d3bea3a63dc3ed88",
  "count": 3,
  "before": "disabled={$mutex}",
  "after": "disabled={$mutex || ($localLibraryState.mode === \"account\" && !$Session)}",
  "approved_ui_extension": "route_owned_library_identity"
},
  {
  "path": "src/routes/(app)/settings/+layout.svelte",
  "label": "native Settings tab navigation has a precise accessible name",
  "sha256": "59ebe00598167420b7b5ecea5f36214373f06e436e263f7aa7015f11b268574a",
  "count": 1,
  "before": "<nav class=\"text-muted-foreground/80 -mb-0.5 flex flex-nowrap overflow-auto whitespace-nowrap\">",
  "after": "<nav aria-label=\"Settings\" class=\"text-muted-foreground/80 -mb-0.5 flex flex-nowrap overflow-auto whitespace-nowrap\">",
  "approved_ui_extension": "route_owned_library_identity"
},
{
  "path": "src/routes/(app)/auth/register/+page.svelte",
  "label": "fresh registration imports presentation and safe local return only",
  "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
  "count": 1,
  "before": "\timport { createAuthFormOwner } from \"$platform/auth-form-owner\";",
  "after": "\timport { createAuthFormOwner } from \"$platform/auth-form-owner\";\r\n\timport AccountOnboarding from \"$platform/AccountOnboarding.svelte\";\r\n\timport { safeRegistrationReturnTo } from \"$platform/account-onboarding\";\r\n\timport { get } from \"svelte/store\";"
},
{
  "path": "src/routes/(app)/auth/register/+page.svelte",
  "label": "fresh registration keeps only in-memory welcome and current account ownership",
  "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
  "count": 1,
  "before": "\tlet isMounted = $state(false);",
  "after": "\tlet isMounted = $state(false);\r\n\tlet registrationWelcome = $state<{ accountId: string; returnTo: string; needsVerification: boolean; sessionRefreshed: boolean; continue: () => void; current: () => boolean }>();"
},
{
  "path": "src/routes/(app)/auth/register/+page.svelte",
  "label": "registration redirect is single-decoded same-origin safe navigation",
  "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
  "count": 1,
  "before": "\t\tconst redirect = decodeURIComponent(page.url.searchParams.get(\"redirect\") ?? \"/\");",
  "after": "\t\tconst redirect = safeRegistrationReturnTo(page.url.searchParams.get(\"redirect\"));"
},
{
  "path": "src/routes/(app)/auth/register/+page.svelte",
  "label": "registration refresh retains safe intended return without persisting a welcome trigger",
  "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
  "count": 1,
  "before": "\t\t\t\tq.delete(\"redirect\");",
  "after": "\t\t\t\tq.set(\"redirect\", safeRegistrationReturnTo(res.data.return_to || redirect));"
},
{
  "path": "src/routes/(app)/auth/register/+page.svelte",
  "label": "accepted registration is not submitted again from repeated controls",
  "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
  "count": 1,
  "before": "\t\tif (!flow || !ref) {",
  "after": "\t\tif (!flow || !ref || registrationWelcome) {"
},
{
  "path": "src/routes/(app)/auth/register/+page.svelte",
  "label": "fresh accepted registration explicitly finishes through native welcome",
  "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
  "count": 1,
  "before": "\t\t\tif (data?.session?.active) {\r\n\t\t\t\tawait Session.initialize(true);\r\n\t\t\t\tif (!authIntent.alive()) return;\r\n\t\t\t\tif (returnTo && !returnTo?.startsWith(window.location.origin)) {\r\n\t\t\t\t\twindow.location.href = returnTo;\r\n\t\t\t\t} else goto(returnTo || \"/\");",
  "after": "\t\t\tif (!(res instanceof AxiosError) && res.status === StatusCodes.OK && data?.session?.active) {\r\n\t\t\t\tconst accountId = data.session.identity?.id;\r\n\t\t\t\tif (!accountId) throw Constants.Error.Unknown;\r\n\t\t\t\tlet sessionRefreshed = false;\r\n\t\t\t\ttry { await Session.initialize(true); sessionRefreshed = true; }\r\n\t\t\t\tcatch { /* The accepted registration must not be replayed after a session-read failure. */ }\r\n\t\t\t\tif (!authIntent.alive()) return;\r\n\t\t\t\tconst destination = safeRegistrationReturnTo(returnTo);\r\n\t\t\t\tlet continuing = false;\r\n\t\t\t\tregistrationWelcome = {\r\n\t\t\t\t\taccountId, returnTo: destination, sessionRefreshed,\r\n\t\t\t\t\tneedsVerification: !!data.session.identity?.verifiable_addresses?.some(address => address.via === \"email\" && address.verified === false),\r\n\t\t\t\t\tcontinue: async () => {\r\n\t\t\t\t\t\tif (continuing || !sessionRefreshed || !authIntent.alive() || get(Session)?.identity?.id !== accountId) return;\r\n\t\t\t\t\t\tcontinuing = true;\r\n\t\t\t\t\t\ttry { await goto(destination); }\r\n\t\t\t\t\t\tcatch { if (authIntent.alive()) { continuing = false; toast.error(\"Could not continue\", { description: \"Please try again.\" }); } }\r\n\t\t\t\t\t},\r\n\t\t\t\t\tcurrent: () => sessionRefreshed && authIntent.alive() && get(Session)?.identity?.id === accountId\r\n\t\t\t\t};"
},
{
  "path": "src/routes/(app)/auth/register/+page.svelte",
  "label": "registration ignores queued repeated submit after the first accepted response",
  "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
  "count": 1,
  "before": "\t\tev.preventDefault();\r\n\t\treturn mutex.exec(async () => {",
  "after": "\t\tev.preventDefault();\r\n\t\tif (mutex.isLocked() || registrationWelcome) return;\r\n\t\treturn mutex.exec(async () => {"
},
{
  "path": "src/routes/(app)/auth/register/+page.svelte",
  "label": "fresh registration welcome replaces only the completed signup form",
  "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
  "count": 1,
  "before": "{#if isMounted}\r\n\t<Card.Root class=\"relative rounded\">",
  "after": "{#if registrationWelcome}\r\n\t<AccountOnboarding returnTo={registrationWelcome.returnTo} needsVerification={registrationWelcome.needsVerification} accountCurrent={registrationWelcome.sessionRefreshed && $Session?.identity?.id === registrationWelcome.accountId} oncontinue={registrationWelcome.continue} current={registrationWelcome.current} />\r\n{:else if isMounted}\r\n\t<Card.Root class=\"relative rounded\">",
  "approved_registration_onboarding_exception": "fresh_registration_welcome"
},
{
  "path": "src/routes/(app)/auth/login/+page.svelte",
  "label": "login imports single-decoded safe continuation",
  "sha256": "0f352782af49530504142a2e6dc79e621f9433a53e581710917b386bd279c8c5",
  "count": 1,
  "before": "\timport { createAuthFormOwner } from \"$platform/auth-form-owner\";",
  "after": "\timport { createAuthFormOwner } from \"$platform/auth-form-owner\";\r\n\timport { safeRegistrationReturnTo } from \"$platform/account-onboarding\";"
},
{
  "path": "src/routes/(app)/auth/login/+page.svelte",
  "label": "login preserves onboarding continuation without double decoding",
  "sha256": "0f352782af49530504142a2e6dc79e621f9433a53e581710917b386bd279c8c5",
  "count": 1,
  "before": "\t\tconst redirect = decodeURIComponent(page.url.searchParams.get(\"redirect\") ?? \"/\");",
  "after": "\t\tconst redirect = safeRegistrationReturnTo(page.url.searchParams.get(\"redirect\"));"
},
{
  "path": "src/routes/(app)/auth/verification/+page.svelte",
  "label": "verification imports single-decoded safe continuation",
  "sha256": "0a2ea92f2157a12e8dc56b813b5fc6ec7e909459fedab9447343b99aefcfe702",
  "count": 1,
  "before": "\timport { createAuthFormOwner } from \"$platform/auth-form-owner\";",
  "after": "\timport { createAuthFormOwner } from \"$platform/auth-form-owner\";\r\n\timport { safeRegistrationReturnTo } from \"$platform/account-onboarding\";"
},
{
  "path": "src/routes/(app)/auth/verification/+page.svelte",
  "label": "verification preserves onboarding continuation without double decoding",
  "sha256": "0a2ea92f2157a12e8dc56b813b5fc6ec7e909459fedab9447343b99aefcfe702",
  "count": 1,
  "before": "\t\tconst redirect = decodeURIComponent(page.url.searchParams.get(\"redirect\") ?? \"/\");",
  "after": "\t\tconst redirect = safeRegistrationReturnTo(page.url.searchParams.get(\"redirect\"));"
},
  {
  "path": "src/routes/(app)/title/MangaActions.svelte",
  "label": "all-read Continue Reading stays at newest completed aggregate chapter",
  "sha256": "a22bba60275a579d79c3850a90c98bef3db988774bf8081eb478b4ce012d058f",
  "count": 1,
  "before": "\t\t\t\t\t\t} else targetAggregatedChapter = aggregated.chapters[aggregated.chapters.length - 1];",
  "after": "\t\t\t\t\t\t} else targetAggregatedChapter = aggregated.chapters[0];"
},
  {
  "path": "src/lib/components/reader/reader-page-selector.svelte",
  "label": "reader ChevronsLeft accessible name",
  "sha256": "3de870d4eef414bc0b3d6e44fa0fb2b0e84f6bd3e635b48f21c597ad0e1694c3",
  "count": 1,
  "before": "<Button\r\n\t\tclass=\"border-none\"\r\n\t\tvariant=\"outline\"\r\n\t\tsize=\"icon\"\r\n\t\tonclick={() =>\r\n\t\t\treader.changePage($readerPrefs.direction === Constants.Reader.Direction.RTL ? reader.pages.length - 1 : 0)}\r\n\t\tdisabled={!reader.pages.length}\r\n\t>",
  "after": "<Button\r\n\t\tclass=\"border-none\"\r\n\t\tvariant=\"outline\"\r\n\t\tsize=\"icon\"\r\n\t\tonclick={() =>\r\n\t\t\treader.changePage($readerPrefs.direction === Constants.Reader.Direction.RTL ? reader.pages.length - 1 : 0)}\r\n\t\tdisabled={!reader.pages.length}\r\n\t\taria-label={$readerPrefs.direction === Constants.Reader.Direction.RTL ? \"Last page\" : \"First page\"}\r\n\t>",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-page-selector.svelte",
  "label": "reader ChevronLeft accessible name",
  "sha256": "3de870d4eef414bc0b3d6e44fa0fb2b0e84f6bd3e635b48f21c597ad0e1694c3",
  "count": 1,
  "before": "<Button\r\n\t\tclass=\"border-none\"\r\n\t\tvariant=\"outline\"\r\n\t\tsize=\"icon\"\r\n\t\tonclick={() =>\r\n\t\t\t$readerPrefs.direction === Constants.Reader.Direction.RTL ? reader.gotoNextPage() : reader.gotoPreviousPage()}\r\n\t\tdisabled={!reader.pages.length}\r\n\t>",
  "after": "<Button\r\n\t\tclass=\"border-none\"\r\n\t\tvariant=\"outline\"\r\n\t\tsize=\"icon\"\r\n\t\tonclick={() =>\r\n\t\t\t$readerPrefs.direction === Constants.Reader.Direction.RTL ? reader.gotoNextPage() : reader.gotoPreviousPage()}\r\n\t\tdisabled={!reader.pages.length}\r\n\t\taria-label={$readerPrefs.direction === Constants.Reader.Direction.RTL ? \"Next page\" : \"Previous page\"}\r\n\t>",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-page-selector.svelte",
  "label": "reader ChevronRight accessible name",
  "sha256": "3de870d4eef414bc0b3d6e44fa0fb2b0e84f6bd3e635b48f21c597ad0e1694c3",
  "count": 1,
  "before": "<Button\r\n\t\tclass=\"border-none\"\r\n\t\tvariant=\"outline\"\r\n\t\tsize=\"icon\"\r\n\t\tonclick={() =>\r\n\t\t\t$readerPrefs.direction === Constants.Reader.Direction.RTL ? reader.gotoPreviousPage() : reader.gotoNextPage()}\r\n\t\tdisabled={!reader.pages.length}\r\n\t>",
  "after": "<Button\r\n\t\tclass=\"border-none\"\r\n\t\tvariant=\"outline\"\r\n\t\tsize=\"icon\"\r\n\t\tonclick={() =>\r\n\t\t\t$readerPrefs.direction === Constants.Reader.Direction.RTL ? reader.gotoPreviousPage() : reader.gotoNextPage()}\r\n\t\tdisabled={!reader.pages.length}\r\n\t\taria-label={$readerPrefs.direction === Constants.Reader.Direction.RTL ? \"Previous page\" : \"Next page\"}\r\n\t>",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-page-selector.svelte",
  "label": "reader ChevronsRight accessible name",
  "sha256": "3de870d4eef414bc0b3d6e44fa0fb2b0e84f6bd3e635b48f21c597ad0e1694c3",
  "count": 1,
  "before": "<Button\r\n\t\tclass=\"border-none\"\r\n\t\tvariant=\"outline\"\r\n\t\tsize=\"icon\"\r\n\t\tonclick={() =>\r\n\t\t\treader.changePage($readerPrefs.direction === Constants.Reader.Direction.RTL ? 0 : reader.pages.length - 1)}\r\n\t\tdisabled={!reader.pages.length}\r\n\t>",
  "after": "<Button\r\n\t\tclass=\"border-none\"\r\n\t\tvariant=\"outline\"\r\n\t\tsize=\"icon\"\r\n\t\tonclick={() =>\r\n\t\t\treader.changePage($readerPrefs.direction === Constants.Reader.Direction.RTL ? 0 : reader.pages.length - 1)}\r\n\t\tdisabled={!reader.pages.length}\r\n\t\taria-label={$readerPrefs.direction === Constants.Reader.Direction.RTL ? \"First page\" : \"Last page\"}\r\n\t>",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-page-selector.svelte",
  "label": "reader page selector accessible name",
  "sha256": "3de870d4eef414bc0b3d6e44fa0fb2b0e84f6bd3e635b48f21c597ad0e1694c3",
  "count": 1,
  "before": "<Combobox.Trigger class={buttonVariants({ class: \"w-0 flex-auto border-none\", variant: \"outline\" })}>",
  "after": "<Combobox.Trigger class={buttonVariants({ class: \"w-0 flex-auto border-none\", variant: \"outline\" })} aria-label=\"Select page\">",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-chapter-selector.svelte",
  "label": "reader ChevronsLeft accessible name",
  "sha256": "93e883cf939c5a381bb14e2c285f3a11d68ae22740b7e02eea1baafc97a93702",
  "count": 1,
  "before": "<a\r\n\t\tclass={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"icon\" })}\r\n\t\tclass:pointer-events-none={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !lastChapter : !firstChapter}\r\n\t\tclass:opacity-50={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !lastChapter : !firstChapter}\r\n\t\thref={$readerPrefs.direction === Constants.Reader.Direction.RTL\r\n\t\t\t? lastChapter\r\n\t\t\t\t? `/chapter/${lastChapter.id}/1`\r\n\t\t\t\t: undefined\r\n\t\t\t: firstChapter\r\n\t\t\t\t? `/chapter/${firstChapter.id}/1`\r\n\t\t\t\t: undefined}\r\n\t>",
  "after": "<a\r\n\t\tclass={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"icon\" })}\r\n\t\tclass:pointer-events-none={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !lastChapter : !firstChapter}\r\n\t\tclass:opacity-50={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !lastChapter : !firstChapter}\r\n\t\thref={$readerPrefs.direction === Constants.Reader.Direction.RTL\r\n\t\t\t? lastChapter\r\n\t\t\t\t? `/chapter/${lastChapter.id}/1`\r\n\t\t\t\t: undefined\r\n\t\t\t: firstChapter\r\n\t\t\t\t? `/chapter/${firstChapter.id}/1`\r\n\t\t\t\t: undefined}\r\n\t\taria-label={$readerPrefs.direction === Constants.Reader.Direction.RTL ? \"Last chapter\" : \"First chapter\"}\r\n\t>",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-chapter-selector.svelte",
  "label": "reader ChevronLeft accessible name",
  "sha256": "93e883cf939c5a381bb14e2c285f3a11d68ae22740b7e02eea1baafc97a93702",
  "count": 1,
  "before": "<a\r\n\t\tclass={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"icon\" })}\r\n\t\tclass:pointer-events-none={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !nextChapter : !prevChapter}\r\n\t\tclass:opacity-50={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !nextChapter : !prevChapter}\r\n\t\thref={$readerPrefs.direction === Constants.Reader.Direction.RTL\r\n\t\t\t? nextChapter\r\n\t\t\t\t? `/chapter/${nextChapter.id}/1`\r\n\t\t\t\t: undefined\r\n\t\t\t: prevChapter\r\n\t\t\t\t? `/chapter/${prevChapter.id}/1`\r\n\t\t\t\t: undefined}\r\n\t>",
  "after": "<a\r\n\t\tclass={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"icon\" })}\r\n\t\tclass:pointer-events-none={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !nextChapter : !prevChapter}\r\n\t\tclass:opacity-50={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !nextChapter : !prevChapter}\r\n\t\thref={$readerPrefs.direction === Constants.Reader.Direction.RTL\r\n\t\t\t? nextChapter\r\n\t\t\t\t? `/chapter/${nextChapter.id}/1`\r\n\t\t\t\t: undefined\r\n\t\t\t: prevChapter\r\n\t\t\t\t? `/chapter/${prevChapter.id}/1`\r\n\t\t\t\t: undefined}\r\n\t\taria-label={$readerPrefs.direction === Constants.Reader.Direction.RTL ? \"Next chapter\" : \"Previous chapter\"}\r\n\t>",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-chapter-selector.svelte",
  "label": "reader ChevronRight accessible name",
  "sha256": "93e883cf939c5a381bb14e2c285f3a11d68ae22740b7e02eea1baafc97a93702",
  "count": 1,
  "before": "<a\r\n\t\tclass={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"icon\" })}\r\n\t\tclass:pointer-events-none={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !prevChapter : !nextChapter}\r\n\t\tclass:opacity-50={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !prevChapter : !nextChapter}\r\n\t\thref={$readerPrefs.direction === Constants.Reader.Direction.RTL\r\n\t\t\t? prevChapter\r\n\t\t\t\t? `/chapter/${prevChapter.id}/1`\r\n\t\t\t\t: undefined\r\n\t\t\t: nextChapter\r\n\t\t\t\t? `/chapter/${nextChapter.id}/1`\r\n\t\t\t\t: undefined}\r\n\t>",
  "after": "<a\r\n\t\tclass={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"icon\" })}\r\n\t\tclass:pointer-events-none={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !prevChapter : !nextChapter}\r\n\t\tclass:opacity-50={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !prevChapter : !nextChapter}\r\n\t\thref={$readerPrefs.direction === Constants.Reader.Direction.RTL\r\n\t\t\t? prevChapter\r\n\t\t\t\t? `/chapter/${prevChapter.id}/1`\r\n\t\t\t\t: undefined\r\n\t\t\t: nextChapter\r\n\t\t\t\t? `/chapter/${nextChapter.id}/1`\r\n\t\t\t\t: undefined}\r\n\t\taria-label={$readerPrefs.direction === Constants.Reader.Direction.RTL ? \"Previous chapter\" : \"Next chapter\"}\r\n\t>",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-chapter-selector.svelte",
  "label": "reader ChevronsRight accessible name",
  "sha256": "93e883cf939c5a381bb14e2c285f3a11d68ae22740b7e02eea1baafc97a93702",
  "count": 1,
  "before": "<a\r\n\t\tclass={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"icon\" })}\r\n\t\tclass:pointer-events-none={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !firstChapter : !lastChapter}\r\n\t\tclass:opacity-50={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !firstChapter : !lastChapter}\r\n\t\thref={$readerPrefs.direction === Constants.Reader.Direction.RTL\r\n\t\t\t? firstChapter\r\n\t\t\t\t? `/chapter/${firstChapter.id}/1`\r\n\t\t\t\t: undefined\r\n\t\t\t: lastChapter\r\n\t\t\t\t? `/chapter/${lastChapter.id}/1`\r\n\t\t\t\t: undefined}\r\n\t>",
  "after": "<a\r\n\t\tclass={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"icon\" })}\r\n\t\tclass:pointer-events-none={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !firstChapter : !lastChapter}\r\n\t\tclass:opacity-50={$readerPrefs.direction === Constants.Reader.Direction.RTL ? !firstChapter : !lastChapter}\r\n\t\thref={$readerPrefs.direction === Constants.Reader.Direction.RTL\r\n\t\t\t? firstChapter\r\n\t\t\t\t? `/chapter/${firstChapter.id}/1`\r\n\t\t\t\t: undefined\r\n\t\t\t: lastChapter\r\n\t\t\t\t? `/chapter/${lastChapter.id}/1`\r\n\t\t\t\t: undefined}\r\n\t\taria-label={$readerPrefs.direction === Constants.Reader.Direction.RTL ? \"First chapter\" : \"Last chapter\"}\r\n\t>",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-chapter-selector.svelte",
  "label": "reader chapter selector accessible name",
  "sha256": "93e883cf939c5a381bb14e2c285f3a11d68ae22740b7e02eea1baafc97a93702",
  "count": 1,
  "before": "<Combobox.Trigger class={buttonVariants({ class: \"w-0 flex-auto border-none\", variant: \"outline\" })}>",
  "after": "<Combobox.Trigger class={buttonVariants({ class: \"w-0 flex-auto border-none\", variant: \"outline\" })} aria-label=\"Select chapter\">",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-pagination.svelte",
  "label": "reader HomeIcon accessible name",
  "sha256": "c600e78d8754207a5a4b791458c36ef7e3bdcd7902e53d55eb81f702f764d36b",
  "count": 1,
  "before": "<a class={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"icon-lg\" })} href=\"/\">",
  "after": "<a class={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"icon-lg\" })} href=\"/\" aria-label=\"Home\">",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-pagination.svelte",
  "label": "reader Undo2 accessible name",
  "sha256": "c600e78d8754207a5a4b791458c36ef7e3bdcd7902e53d55eb81f702f764d36b",
  "count": 1,
  "before": "<a\r\n\t\t\t\t\tclass={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"icon-lg\" })}\r\n\t\t\t\t\thref=\"/title/{reader.chapter.relationships.manga.id}{Utils.makeSlug(\r\n\t\t\t\t\t\treader.chapter.relationships.manga.title\r\n\t\t\t\t\t)}\"\r\n\t\t\t\t>",
  "after": "<a\r\n\t\t\t\t\tclass={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"icon-lg\" })}\r\n\t\t\t\t\thref=\"/title/{reader.chapter.relationships.manga.id}{Utils.makeSlug(\r\n\t\t\t\t\t\treader.chapter.relationships.manga.title\r\n\t\t\t\t\t)}\"\r\n\t\t\t\t\taria-label=\"Back to title\"\r\n\t\t\t\t>",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-pagination.svelte",
  "label": "reader SettingsIcon accessible name",
  "sha256": "c600e78d8754207a5a4b791458c36ef7e3bdcd7902e53d55eb81f702f764d36b",
  "count": 1,
  "before": "<Button class=\"border-none\" variant=\"outline\" size=\"icon-lg\" onclick={sidebar.toggle}>",
  "after": "<Button class=\"border-none\" variant=\"outline\" size=\"icon-lg\" onclick={sidebar.toggle} aria-label=\"Reader settings\">",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-pagination.svelte",
  "label": "reader ChevronsLeft accessible name",
  "sha256": "c600e78d8754207a5a4b791458c36ef7e3bdcd7902e53d55eb81f702f764d36b",
  "count": 1,
  "before": "<Button\r\n\t\t\t\t\tclass=\"border-none\"\r\n\t\t\t\t\tvariant=\"outline\"\r\n\t\t\t\t\tsize=\"icon-lg\"\r\n\t\t\t\t\tonclick={() =>\r\n\t\t\t\t\t\treader.changePage($readerPrefs.direction === Constants.Reader.Direction.RTL ? reader.pages.length - 1 : 0)}\r\n\t\t\t\t\tdisabled={!reader.pages.length}\r\n\t\t\t\t>",
  "after": "<Button\r\n\t\t\t\t\tclass=\"border-none\"\r\n\t\t\t\t\tvariant=\"outline\"\r\n\t\t\t\t\tsize=\"icon-lg\"\r\n\t\t\t\t\tonclick={() =>\r\n\t\t\t\t\t\treader.changePage($readerPrefs.direction === Constants.Reader.Direction.RTL ? reader.pages.length - 1 : 0)}\r\n\t\t\t\t\tdisabled={!reader.pages.length}\r\n\t\t\t\t\taria-label={$readerPrefs.direction === Constants.Reader.Direction.RTL ? \"Last page\" : \"First page\"}\r\n\t\t\t\t>",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-pagination.svelte",
  "label": "reader ChevronsRight accessible name",
  "sha256": "c600e78d8754207a5a4b791458c36ef7e3bdcd7902e53d55eb81f702f764d36b",
  "count": 1,
  "before": "<Button\r\n\t\t\t\t\tclass=\"border-none\"\r\n\t\t\t\t\tvariant=\"outline\"\r\n\t\t\t\t\tsize=\"icon-lg\"\r\n\t\t\t\t\tonclick={() =>\r\n\t\t\t\t\t\treader.changePage($readerPrefs.direction === Constants.Reader.Direction.RTL ? 0 : reader.pages.length - 1)}\r\n\t\t\t\t\tdisabled={!reader.pages.length}\r\n\t\t\t\t>",
  "after": "<Button\r\n\t\t\t\t\tclass=\"border-none\"\r\n\t\t\t\t\tvariant=\"outline\"\r\n\t\t\t\t\tsize=\"icon-lg\"\r\n\t\t\t\t\tonclick={() =>\r\n\t\t\t\t\t\treader.changePage($readerPrefs.direction === Constants.Reader.Direction.RTL ? 0 : reader.pages.length - 1)}\r\n\t\t\t\t\tdisabled={!reader.pages.length}\r\n\t\t\t\t\taria-label={$readerPrefs.direction === Constants.Reader.Direction.RTL ? \"First page\" : \"Last page\"}\r\n\t\t\t\t>",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-pagination.svelte",
  "label": "reader pagination chapter selector accessible name",
  "sha256": "c600e78d8754207a5a4b791458c36ef7e3bdcd7902e53d55eb81f702f764d36b",
  "count": 1,
  "before": "\t\t\t\t<Combobox.Trigger\r\n",
  "after": "\t\t\t\t<Combobox.Trigger\r\n\t\t\t\t\taria-label=\"Select chapter\"\r\n",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-panel.svelte",
  "label": "reader HomeIcon accessible name",
  "sha256": "6fc6e2c1432d40d0d9c6069d41c751d9cc286ac4f26398d4750cab7e9e2d34fa",
  "count": 1,
  "before": "<a class={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"icon\" })} href=\"/\">",
  "after": "<a class={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"icon\" })} href=\"/\" aria-label=\"Home\">",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-panel.svelte",
  "label": "reader BookIcon accessible name",
  "sha256": "6fc6e2c1432d40d0d9c6069d41c751d9cc286ac4f26398d4750cab7e9e2d34fa",
  "count": 1,
  "before": "<a\r\n\t\t\t\t\t\t\tclass={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"icon\" })}\r\n\t\t\t\t\t\t\thref=\"/title/{reader.chapter.relationships.manga.id}{Utils.makeSlug(\r\n\t\t\t\t\t\t\t\treader.chapter.relationships.manga.title\r\n\t\t\t\t\t\t\t)}\"\r\n\t\t\t\t\t\t>",
  "after": "<a\r\n\t\t\t\t\t\t\tclass={buttonVariants({ class: \"border-none\", variant: \"outline\", size: \"icon\" })}\r\n\t\t\t\t\t\t\thref=\"/title/{reader.chapter.relationships.manga.id}{Utils.makeSlug(\r\n\t\t\t\t\t\t\t\treader.chapter.relationships.manga.title\r\n\t\t\t\t\t\t\t)}\"\r\n\t\t\t\t\t\t\taria-label=\"Back to title\"\r\n\t\t\t\t\t\t>",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-panel.svelte",
  "label": "reader XIcon accessible name",
  "sha256": "6fc6e2c1432d40d0d9c6069d41c751d9cc286ac4f26398d4750cab7e9e2d34fa",
  "count": 1,
  "before": "<Button class=\"border-none\" type=\"button\" variant=\"outline\" size=\"icon\" onclick={sidebar.toggle}>",
  "after": "<Button class=\"border-none\" type=\"button\" variant=\"outline\" size=\"icon\" onclick={sidebar.toggle} aria-label=\"Close reader settings\">",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/comments/comments-votes.svelte",
  "label": "reader discussion vote 1 accessible name",
  "sha256": "5d7b67c964c3d38c42b7fc6c064be82dfef0c351258a5ff6f576fd702a88a14c",
  "count": 1,
  "before": "\t<Button\r\n\t\tclass={cn(\"text-muted-foreground border-0\", {\r\n\t\t\t\"!border-accent text-accent-foreground border-b-2\": comments.voteState === 1",
  "after": "\t<Button\r\n\t\taria-label=\"Upvote chapter or discussion\"\r\n\t\tclass={cn(\"text-muted-foreground border-0\", {\r\n\t\t\t\"!border-accent text-accent-foreground border-b-2\": comments.voteState === 1",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/comments/comments-votes.svelte",
  "label": "reader discussion vote -1 accessible name",
  "sha256": "5d7b67c964c3d38c42b7fc6c064be82dfef0c351258a5ff6f576fd702a88a14c",
  "count": 1,
  "before": "\t<Button\r\n\t\tclass={cn(\"text-muted-foreground border-0\", {\r\n\t\t\t\"!border-accent text-accent-foreground border-b-2\": comments.voteState === -1",
  "after": "\t<Button\r\n\t\taria-label=\"Downvote chapter or discussion\"\r\n\t\tclass={cn(\"text-muted-foreground border-0\", {\r\n\t\t\t\"!border-accent text-accent-foreground border-b-2\": comments.voteState === -1",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-page-image.svelte",
  "label": "reader page image exposes its page number",
  "sha256": "528b0e5787cc24aa6cd1863cf34ea49b84e360a2a3322d1025de5b2c266b03cf",
  "count": 1,
  "before": "\t<img\r\n\t\tclass=\"object-contain\"",
  "after": "\t<img\r\n\t\talt={`Page ${page.index + 1}`}\r\n\t\tclass=\"object-contain\"",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-settings.svelte",
  "label": "reader narrow-phone direction LTR retains nonoverlapping controls",
  "sha256": "414271f285f269a15f9f171d41e11c96456cf498aa63805916728b7de6e54b90",
  "count": 1,
  "before": "class={cn(\"w-0 flex-auto border-none\", {\r\n\t\t\t\t\"text-accent-foreground\": $readerPrefs.direction === Constants.Reader.Direction.LTR\r\n\t\t\t})}",
  "after": "class={cn(\"w-0 flex-auto border-none max-[360px]:gap-1 max-[360px]:px-1 max-[360px]:text-xs max-[360px]:[&>svg]:size-4\", {\r\n\t\t\t\t\"text-accent-foreground\": $readerPrefs.direction === Constants.Reader.Direction.LTR\r\n\t\t\t})}",
  "approved_reader_control_exception": "reader_narrow_direction_fit"
},
  {
  "path": "src/lib/components/reader/reader-settings.svelte",
  "label": "reader narrow-phone direction RTL retains nonoverlapping controls",
  "sha256": "414271f285f269a15f9f171d41e11c96456cf498aa63805916728b7de6e54b90",
  "count": 1,
  "before": "class={cn(\"w-0 flex-auto border-none\", {\r\n\t\t\t\t\"text-accent-foreground\": $readerPrefs.direction === Constants.Reader.Direction.RTL\r\n\t\t\t})}",
  "after": "class={cn(\"w-0 flex-auto border-none max-[360px]:gap-1 max-[360px]:px-1 max-[360px]:text-xs max-[360px]:[&>svg]:size-4\", {\r\n\t\t\t\t\"text-accent-foreground\": $readerPrefs.direction === Constants.Reader.Direction.RTL\r\n\t\t\t})}",
  "approved_reader_control_exception": "reader_narrow_direction_fit"
},
  {
  "path": "src/lib/components/reader/reader-page-image.svelte",
  "label": "reader Stretch Width expands its actual image wrapper",
  "sha256": "528b0e5787cc24aa6cd1863cf34ea49b84e360a2a3322d1025de5b2c266b03cf",
  "count": 1,
  "before": "<div class=\"group relative\" bind:this={$ref}>",
  "after": "<div class=\"group relative\" class:w-full={$readerPrefs.scale === Constants.Reader.Scale.StretchWidth} bind:this={$ref}>",
  "approved_reader_control_exception": "reader_stretch_width_fit"
},
  {
  "path": "src/lib/components/reader/reader-provider.svelte",
  "label": "reader malformed raw page always resolves a finite bounded image index",
  "sha256": "c12a1d141b258f7461a150511387488193b55f6825f9f4c3cd86fbc962feb3fb",
  "count": 1,
  "before": "\t\t\treader.currentPage =\r\n\t\t\t\treader.pages[\r\n\t\t\t\t\tMath.min(Math.max(Number((page.state as any).page ?? page.params.page ?? 1), 1), reader.pages.length) - 1\r\n\t\t\t\t];",
  "after": "\t\t\tconst requestedPage = Number((page.state as any).page ?? page.params.page ?? 1);\r\n\t\t\tconst safePage = Number.isFinite(requestedPage) ? Math.trunc(requestedPage) : 1;\r\n\t\t\treader.currentPage = reader.pages[Math.min(Math.max(safePage, 1), reader.pages.length) - 1];"
},
  {
  "path": "src/lib/components/reader/context.svelte.ts",
  "label": "reader selecting current last page leaves end-of-chapter card",
  "sha256": "7ee3da2eada7042e8cd7d2d2b464dfe03cd71a7bcd51f861e0b25b7eb5c02c4d",
  "count": 1,
  "before": "\t\t\t\tidx === this.currentPage.index ||",
  "after": "\t\t\t\t(idx === this.currentPage.index && !this.isViewingLastPage) ||"
},
  {
  "path": "src/lib/components/reader/reader-last-page.svelte",
  "label": "mobile long-strip Home accessible name",
  "sha256": "a95caf71008d455ea1d888cabe209e27273e3ddb3982ff6f7b8f2f0aac949b7e",
  "count": 1,
  "before": "<a\r\n\t\t\t\tclass={buttonVariants({\r\n\t\t\t\t\tclass: \"border-none\",\r\n\t\t\t\t\tvariant: \"outline\",\r\n\t\t\t\t\tsize: isMobile.current ? \"icon\" : \"sm\"\r\n\t\t\t\t})}\r\n\t\t\t\thref=\"/\"\r\n\t\t\t\tonclick={e => {\r\n\t\t\t\t\te.stopPropagation();\r\n\t\t\t\t\te.stopImmediatePropagation();\r\n\t\t\t\t}}\r\n\t\t\t>",
  "after": "<a\r\n\t\t\t\tclass={buttonVariants({\r\n\t\t\t\t\tclass: \"border-none\",\r\n\t\t\t\t\tvariant: \"outline\",\r\n\t\t\t\t\tsize: isMobile.current ? \"icon\" : \"sm\"\r\n\t\t\t\t})}\r\n\t\t\t\thref=\"/\"\r\n\t\t\t\tonclick={e => {\r\n\t\t\t\t\te.stopPropagation();\r\n\t\t\t\t\te.stopImmediatePropagation();\r\n\t\t\t\t}}\r\n\t\t\t\taria-label=\"Home\"\r\n\t\t\t>",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
  {
  "path": "src/lib/components/reader/reader-last-page.svelte",
  "label": "mobile long-strip Back to title accessible name",
  "sha256": "a95caf71008d455ea1d888cabe209e27273e3ddb3982ff6f7b8f2f0aac949b7e",
  "count": 1,
  "before": "<a\r\n\t\t\t\tclass={buttonVariants({\r\n\t\t\t\t\tclass: \"border-none\",\r\n\t\t\t\t\tvariant: \"outline\",\r\n\t\t\t\t\tsize: isMobile.current ? \"icon\" : \"sm\"\r\n\t\t\t\t})}\r\n\t\t\t\thref=\"/title/{reader.chapter.relationships.manga.id}{Utils.makeSlug(reader.chapter.relationships.manga.title)}\"\r\n\t\t\t\tonclick={e => {\r\n\t\t\t\t\te.stopPropagation();\r\n\t\t\t\t\te.stopImmediatePropagation();\r\n\t\t\t\t}}\r\n\t\t\t>",
  "after": "<a\r\n\t\t\t\tclass={buttonVariants({\r\n\t\t\t\t\tclass: \"border-none\",\r\n\t\t\t\t\tvariant: \"outline\",\r\n\t\t\t\t\tsize: isMobile.current ? \"icon\" : \"sm\"\r\n\t\t\t\t})}\r\n\t\t\t\thref=\"/title/{reader.chapter.relationships.manga.id}{Utils.makeSlug(reader.chapter.relationships.manga.title)}\"\r\n\t\t\t\tonclick={e => {\r\n\t\t\t\t\te.stopPropagation();\r\n\t\t\t\t\te.stopImmediatePropagation();\r\n\t\t\t\t}}\r\n\t\t\t\taria-label=\"Back to title\"\r\n\t\t\t>",
  "approved_reader_control_exception": "reader_accessible_control_name"
},
{
  "path": "src/lib/components/upload/UploadChapter.svelte",
  "label": "recovered upload begin acknowledges only returned server staging baseline",
  "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
  "count": 1,
  "before": "\t\t\t\t\tuploadSession = res.body;",
  "after": "\t\t\t\t\tuploadSession = res.body;\r\n\t\t\t\t\tlastSavedUpload = signature(snapshot(res.body.draft ?? {}, res.body.relationships.files ?? []));"
},
{
  "path": "src/lib/components/upload/UploadChapter.svelte",
  "label": "recovered upload autosave waits for a valid nonempty language",
  "sha256": "91e44bf44322f2d67fa1990c1f9843f98c34ebc795fb27d73e09215b2191d9a3",
  "count": 1,
  "before": "if (resumeDisposed || !uploadSession || uploadSession.can_mutate === false) return;",
  "after": "if (resumeDisposed || !uploadSession || uploadSession.can_mutate === false || typeof draft.language !== \"string\" || !draft.language.trim()) return;"
},
{
  "path": "src/lib/components/upload/UploadChapterEdit.svelte",
  "label": "recovered upload begin acknowledges only returned server staging baseline",
  "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
  "count": 1,
  "before": "\t\t\t\t\tuploadSession = res.body;",
  "after": "\t\t\t\t\tuploadSession = res.body;\r\n\t\t\t\t\tlastSavedUpload = signature(snapshot(res.body.draft ?? {}, res.body.relationships.files ?? []));"
},
{
  "path": "src/lib/components/upload/UploadChapterEdit.svelte",
  "label": "recovered upload autosave waits for a valid nonempty language",
  "sha256": "4d0c4b3619d8c21486eaf5bcfaa94d73ae7ec5b8e90a08dd5abcaf7ea658258c",
  "count": 1,
  "before": "if (resumeDisposed || !uploadSession || uploadSession.can_mutate === false) return;",
  "after": "if (resumeDisposed || !uploadSession || uploadSession.can_mutate === false || typeof chapter.language !== \"string\" || !chapter.language.trim()) return;"
},
{
  "path": "src/lib/components/reader/context.svelte.ts",
  "label": "reader missing current chapter has no accidental minus-one navigation index",
  "sha256": "7ee3da2eada7042e8cd7d2d2b464dfe03cd71a7bcd51f861e0b25b7eb5c02c4d",
  "count": 1,
  "before": "\treadonly chapterIndex = $derived.by(() => this.aggregated?.chapters.findIndex(e => this.chapter.id in e.entries));",
  "after": "\treadonly chapterIndex = $derived.by(() => {\r\n\t\tconst index = this.aggregated?.chapters.findIndex(e => this.chapter.id in e.entries);\r\n\t\treturn index !== undefined && index >= 0 ? index : undefined;\r\n\t});"
},
{
  "path": "src/lib/components/reader/context.svelte.ts",
  "label": "reader chapter selection rejects noninteger indices before aggregate access",
  "sha256": "7ee3da2eada7042e8cd7d2d2b464dfe03cd71a7bcd51f861e0b25b7eb5c02c4d",
  "count": 1,
  "before": "\t\tif (!this.aggregated?.chapters || idx < 0 || idx >= this.aggregated.chapters.length) {",
  "after": "\t\tif (!Number.isSafeInteger(idx) || !this.aggregated?.chapters || idx < 0 || idx >= this.aggregated.chapters.length) {"
},
{
  "path": "src/lib/components/reader/context.svelte.ts",
  "label": "reader empty aggregate entries do not crash chapter navigation",
  "sha256": "7ee3da2eada7042e8cd7d2d2b464dfe03cd71a7bcd51f861e0b25b7eb5c02c4d",
  "count": 1,
  "before": "\t\tconst entries = Object.entries(aggregate.entries);\r\n\t\tif (entries.length > 1) {",
  "after": "\t\tconst entries = Object.entries(aggregate.entries);\r\n\t\tif (!entries.length) return;\r\n\t\tif (entries.length > 1) {"
},
{
  "path": "src/lib/components/reader/context.svelte.ts",
  "label": "reader page selection rejects noninteger indices before changing current page",
  "sha256": "7ee3da2eada7042e8cd7d2d2b464dfe03cd71a7bcd51f861e0b25b7eb5c02c4d",
  "count": 1,
  "before": "\t\t\t\t!this.currentPage ||\r\n\t\t\t\t(idx === this.currentPage.index && !this.isViewingLastPage) ||",
  "after": "\t\t\t\t!this.currentPage ||\r\n\t\t\t\t!Number.isSafeInteger(idx) ||\r\n\t\t\t\t(idx === this.currentPage.index && !this.isViewingLastPage) ||"
},
{
  "path": "src/routes/(app)/user/[id]/uploads/+page.svelte",
  "label": "user upload sort buttons preserve pathname and delimit encoded query",
  "sha256": "28ffd1d4de746e4db28b466a0b1e2d5e85d77b8210aa0bb8f31845b7116b5cc4",
  "count": 2,
  "before": "const target = window.location.pathname + q.toString();",
  "after": "const target = window.location.pathname + (q.size ? \"?\" + q.toString().replace(/^\\?/, \"\") : \"\");",
  "approved_event_handler_exception": true
},
{
  "path": "src/lib/components/reader/reader-pages.svelte",
  "label": "reader page history preserves query/hash and yields to newer page or navigation",
  "sha256": "b9afbee090ec3e6d6b17b77718add83310833ba7ea8ac610228e420d021a5012",
  "count": 1,
  "before": "<script lang=\"ts\">\r\n\timport { replaceState } from \"$app/navigation\";\r\n\timport { page } from \"$app/state\";\r\n\timport { onMount, tick } from \"svelte\";\r\n\timport { useReader } from \"./context.svelte\";\r\n\timport ReaderPagesDouble from \"./reader-pages-double.svelte\";\r\n\timport ReaderPagesSingle from \"./reader-pages-single.svelte\";\r\n\r\n\tconst reader = useReader();\r\n\tconst readerPrefs = reader.prefs;\r\n\tlet isMounted = $state(false);\r\n\r\n\tonMount(async () => {\r\n\t\tawait tick();\r\n\t\tisMounted = true;\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tif (isMounted && reader.currentPage) {\r\n\t\t\tconst q = new URLSearchParams(window.location.search);\r\n\t\t\tconst path = `/chapter/${reader.chapter.id}/${reader.currentPage.index + 1}${q.toString()}${window.location.hash}`;\r\n\t\t\ttick().then(() => {\r\n\t\t\t\treplaceState(path, {\r\n\t\t\t\t\t...page.state,\r\n\t\t\t\t\tpage: reader.currentPage!.index + 1\r\n\t\t\t\t});\r\n\t\t\t});\r\n\t\t}\r\n\t});\r\n</script>",
  "after": "<script lang=\"ts\">\r\n\timport { replaceState } from \"$app/navigation\";\r\n\timport { page } from \"$app/state\";\r\n\timport { onMount, tick } from \"svelte\";\r\n\timport { createSafeReadOwner } from \"$platform/safe-read-owner\";\r\n\timport { useReader } from \"./context.svelte\";\r\n\timport ReaderPagesDouble from \"./reader-pages-double.svelte\";\r\n\timport ReaderPagesSingle from \"./reader-pages-single.svelte\";\r\n\r\n\tconst reader = useReader();\r\n\tconst readerPrefs = reader.prefs;\r\n\tconst historyOwner = createSafeReadOwner();\r\n\tlet isMounted = $state(false);\r\n\r\n\tonMount(() => {\r\n\t\tconst intent = historyOwner.capture();\r\n\t\tvoid tick().then(() => { if (intent.alive()) isMounted = true; });\r\n\t});\r\n\r\n\t$effect(() => {\r\n\t\tconst selected = reader.currentPage;\r\n\t\tif (!isMounted || !selected) return;\r\n\t\tconst chapterId = reader.chapter.id, pageNumber = selected.index + 1;\r\n\t\tconst intent = historyOwner.capture(() => reader.chapter.id === chapterId && reader.currentPage === selected);\r\n\t\tconst target = new URL(window.location.href);\r\n\t\ttarget.pathname = `/chapter/${chapterId}/${pageNumber}`;\r\n\t\tvoid tick().then(() => {\r\n\t\t\tif (!intent.alive()) return;\r\n\t\t\treplaceState(target.pathname + target.search + target.hash, { ...page.state, page: pageNumber });\r\n\t\t});\r\n\t});\r\n</script>"
},
{
  "path": "src/lib/components/reader/context.svelte.ts",
  "label": "reader edition preference compares actual current group records",
  "sha256": "7ee3da2eada7042e8cd7d2d2b464dfe03cd71a7bcd51f861e0b25b7eb5c02c4d",
  "count": 1,
  "before": "\t\t\tconst currentGroups = this.chapter.relationships.groups?.map(e => e.name) ?? [\"No Group\"];",
  "after": "\t\t\tconst currentGroups = this.chapter.relationships.groups ?? [];"
},
{
  "path": "src/lib/components/reader/context.svelte.ts",
  "label": "reader edition preference resolves group identity before presentation name",
  "sha256": "7ee3da2eada7042e8cd7d2d2b464dfe03cd71a7bcd51f861e0b25b7eb5c02c4d",
  "count": 1,
  "before": "\t\t\t\tconst groups = entry.groups.map(idx => this.aggregated?.groups[idx].name);",
  "after": "\t\t\t\tconst groups = entry.groups.map(idx => this.aggregated?.groups[idx]);"
},
{
  "path": "src/lib/components/reader/context.svelte.ts",
  "label": "reader edition preference keeps name fallback only for missing legacy group IDs",
  "sha256": "7ee3da2eada7042e8cd7d2d2b464dfe03cd71a7bcd51f861e0b25b7eb5c02c4d",
  "count": 1,
  "before": "\t\t\t\tconst score = groups.filter(e => currentGroups.includes(e ?? \"\")).length;",
  "after": "\t\t\t\tconst score = groups.filter(group => group && currentGroups.some(current => group.id && current.id ? group.id === current.id : group.name === current.name)).length;"
},
{
  "path": "src/lib/components/reader/reader-queue.svelte",
  "label": "reader disposed queue cannot enqueue pages after a deferred initial tick",
  "sha256": "a40d89e88715e94cf4270dd7cfa9f4ef43ea1da33711cf12fda3ed06876dce03",
  "count": 1,
  "before": "\t\t\tif (!reader.pages.length || !reader.currentPage || reader.pages.every(e => e.isLoaded)) {",
  "after": "\t\t\tif (disposed || !reader.pages.length || !reader.currentPage || reader.pages.every(e => e.isLoaded)) {"
},
{
  "path": "src/lib/components/reader/reader-queue.svelte",
  "label": "reader disposed queue cannot publish deferred queue changes",
  "sha256": "a40d89e88715e94cf4270dd7cfa9f4ef43ea1da33711cf12fda3ed06876dce03",
  "count": 1,
  "before": "\t\t\tif (queue.length !== clone.length || queue.some((e, i) => e !== clone[i])) {",
  "after": "\t\t\tif (disposed) return;\r\n\t\t\tif (queue.length !== clone.length || queue.some((e, i) => e !== clone[i])) {"
},
{
  "path": "src/lib/components/reader/reader-queue.svelte",
  "label": "reader disposed queue cannot acquire pages after mutex admission",
  "sha256": "a40d89e88715e94cf4270dd7cfa9f4ef43ea1da33711cf12fda3ed06876dce03",
  "count": 1,
  "before": "\t\t\tif (!reader.pages.length || !queue.length) {",
  "after": "\t\t\tif (disposed || !reader.pages.length || !queue.length) {"
},
{
  "path": "src/lib/components/reader/reader-queue.svelte",
  "label": "reader disposed queue cannot start another parallel page",
  "sha256": "a40d89e88715e94cf4270dd7cfa9f4ef43ea1da33711cf12fda3ed06876dce03",
  "count": 1,
  "before": "\t\t\tfor (let i = 0; i < queue.length && parallel < $readerPrefs.parallel; i++) {",
  "after": "\t\t\tfor (let i = 0; !disposed && i < queue.length && parallel < $readerPrefs.parallel; i++) {"
},
{
  "path": "src/lib/components/reader/reader-queue.svelte",
  "label": "reader final tick rechecks exact live page before loading claim",
  "sha256": "a40d89e88715e94cf4270dd7cfa9f4ef43ea1da33711cf12fda3ed06876dce03",
  "count": 1,
  "before": "\t\t\t\t\tif (page.isLoading || page.isLoaded || page.isFailed) {",
  "after": "\t\t\t\t\tif (disposed || !page || reader.pages[page.index] !== page || page.isLoading || page.isLoaded || page.isFailed) {"
},
{
  "path": "src/lib/components/reader/reader-queue.svelte",
  "label": "reader queue tracks its own page claims across replacement",
  "sha256": "a40d89e88715e94cf4270dd7cfa9f4ef43ea1da33711cf12fda3ed06876dce03",
  "count": 1,
  "before": "\t\t\t\t\tpage.isLoading = true;",
  "after": "\t\t\t\t\tpage.isLoading = true;\r\n\t\t\t\t\townedPages.add(page);"
},
{
  "path": "src/lib/components/reader/reader-queue.svelte",
  "label": "reader queue releases only its still owned page claim",
  "sha256": "a40d89e88715e94cf4270dd7cfa9f4ef43ea1da33711cf12fda3ed06876dce03",
  "count": 1,
  "before": "\t\t\t\t\t\t\tpage.isLoading = false;",
  "after": "\t\t\t\t\t\t\tif (!ownedPages.delete(page)) return;\r\n\t\t\t\t\t\t\tpage.isLoading = false;\r\n\t\t\t\t\t\t\tif (disposed) return;"
},
{
  "path": "src/lib/components/reader/reader-queue.svelte",
  "label": "reader replacement releases old claims synchronously before next queue",
  "sha256": "a40d89e88715e94cf4270dd7cfa9f4ef43ea1da33711cf12fda3ed06876dce03",
  "count": 1,
  "before": "\tconst temporaryUrls = new Set<string>();\r\n\tlet disposed = false;\r\n\tonDestroy(() => {\r\n\t\tdisposed = true;",
  "after": "\tconst temporaryUrls = new Set<string>();\r\n\tconst ownedPages = new Set<ReaderPage>();\r\n\tlet disposed = false;\r\n\tonDestroy(() => {\r\n\t\tdisposed = true;\r\n\t\tfor (const page of ownedPages) page.isLoading = false;\r\n\t\townedPages.clear();"
}
];
const hash = value => createHash('sha256').update(value).digest('hex');
export function createFrontendOverlays(frontendRoot) {
  const byPath = new Map();
  for (const patch of patches) {
    const absolute = resolve(frontendRoot, patch.path);
    if (hash(readFileSync(absolute)) !== patch.sha256) throw new Error(`Overlay source changed: ${patch.path}`);
    const entries = byPath.get(absolute) ?? [];
    entries.push(patch); byPath.set(absolute, entries);
  }
  return {
    name: 'weebdex-reviewed-script-overlays', enforce: 'pre',
    transform: { order: 'pre', handler(code, id) {
      if (id.includes('?')) return null;
      const entries = byPath.get(resolve(id)); if (!entries) return null;
      if (hash(code) !== entries[0].sha256) throw new Error(`Overlay input differs from preserved source: ${id}`);
      let result = code;
      for (const patch of entries) {
        if (result.split(patch.before).length - 1 !== patch.count) throw new Error(`Overlay anchor changed: ${patch.label}`);
        result = result.split(patch.before).join(patch.after);
      }
      return { code: result, map: null };
    } }
  };
}
export const frontendOverlayManifest = patches.map(({ before, after, ...metadata }) => metadata);

// Exact approved event-handler JavaScript expressions, never general markup.
export const frontendApprovedEventHandlerExceptions = patches.filter(patch => patch.approved_event_handler_exception).map(({ path, sha256, count, before, after }) => ({ path, sha256, count, before, after }));

export const frontendApprovedPanelExceptions = patches.filter(patch => patch.approved_ui_extension).map(({ path, sha256, count, before, after, approved_ui_extension }) => ({ path, sha256, count, before, after, approved_ui_extension }));

export const frontendApprovedTitleContextExceptions = patches.filter(patch => patch.approved_title_context_exception).map(({ path, sha256, count, before, after, approved_title_context_exception }) => ({ path, sha256, count, before, after, approved_title_context_exception }));

export const frontendApprovedUploadRecoveryExceptions = patches.filter(patch => patch.approved_upload_recovery_exception).map(({ path, sha256, count, before, after, approved_upload_recovery_exception }) => ({ path, sha256, count, before, after, approved_upload_recovery_exception }));

export const frontendApprovedRecoveryBoundaryExceptions = patches.filter(patch => patch.approved_recovery_extension).map(({ path, sha256, count, before, after, approved_recovery_extension }) => ({ path, sha256, count, before, after, approved_recovery_extension }));

export const frontendApprovedDiscussionContextExceptions = patches.filter(patch => patch.approved_discussion_context_exception).map(({ path, sha256, count, before, after, approved_discussion_context_exception }) => ({ path, sha256, count, before, after, approved_discussion_context_exception }));

export const frontendApprovedShareControlExceptions = patches.filter(patch => patch.approved_share_control_exception).map(({ path, sha256, count, before, after, approved_share_control_exception }) => ({ path, sha256, count, before, after, approved_share_control_exception }));

export const frontendApprovedClientNameExceptions = patches.filter(patch => patch.approved_client_name_exception).map(({ path, sha256, count, before, after, approved_client_name_exception }) => ({ path, sha256, count, before, after, approved_client_name_exception }));

export const frontendApprovedSearchCallbackExceptions = patches.filter(patch => patch.approved_search_callback_exception).map(({ path, sha256, count, before, after, approved_search_callback_exception }) => ({ path, sha256, count, before, after, approved_search_callback_exception }));

export const frontendApprovedAuthFlowKeyExceptions = patches.filter(patch => patch.approved_auth_flow_key_exception).map(({ path, sha256, count, before, after, approved_auth_flow_key_exception }) => ({ path, sha256, count, before, after, approved_auth_flow_key_exception }));

export const frontendApprovedCreateRecoveryExceptions = patches.filter(patch => patch.approved_create_recovery_exception).map(({ path, sha256, count, before, after, approved_create_recovery_exception }) => ({ path, sha256, count, before, after, approved_create_recovery_exception }));

export const frontendApprovedFollowReadinessExceptions = patches.filter(patch => patch.approved_follow_readiness_exception).map(({ path, sha256, count, before, after, approved_follow_readiness_exception }) => ({ path, sha256, count, before, after, approved_follow_readiness_exception }));

export const frontendApprovedRegistrationOnboardingExceptions = patches.filter(patch => patch.approved_registration_onboarding_exception).map(({ path, sha256, count, before, after, approved_registration_onboarding_exception }) => ({ path, sha256, count, before, after, approved_registration_onboarding_exception }));
export const frontendApprovedReaderControlExceptions = patches.filter(patch => patch.approved_reader_control_exception).map(({ path, sha256, count, before, after, approved_reader_control_exception }) => ({ path, sha256, count, before, after, approved_reader_control_exception }));
