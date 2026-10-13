// Additive reconstruction. No original announcement implementation was supplied.
import { PUBLIC_API_URL } from "$env/static/public";
import { authenticatedMutation, type MutationIntent } from "$platform/authenticated-mutation";
import { readThreadContext } from "$platform/thread-read";
import { HTTPError } from "$lib/HTTP";
export type Announcement = {
	id: string; title: string; body: string; pinned: boolean; published: boolean;
	version: number; created_at: string; updated_at: string; published_at?: string | null;
	relationships: { author?: { id: string; name: string; version: number } };
};
export type AnnouncementFeed = { total: number; limit: number; page: number; data: Announcement[] };
export type AnnouncementInput = { title: string; body: string; pinned: boolean; published: boolean; version?: number };
function url(id?: string, query?: Record<string, string | number | boolean>) {
	const base = `${PUBLIC_API_URL.replace(/\/$/, "")}/announcement${id ? `/${encodeURIComponent(id)}` : ""}`;
	const params = new URLSearchParams();
	for (const [key, value] of Object.entries(query ?? {})) params.set(key, String(value));
	// Also supports initialization before the retained layout's '?' prefix patch.
	const serialized = params.toString().replace(/^\?/, "");
	return base + (serialized ? `?${serialized}` : "");
}
async function request<T>(target: string, method: "GET" | "POST" | "PUT" | "DELETE" = "GET", body?: unknown, intent?: MutationIntent): Promise<T> {
	const base = PUBLIC_API_URL.replace(/\/$/, "");
	if (!target.startsWith(base + "/announcement")) throw Error("Unexpected announcement route");
	const route = target.slice(base.length);
	try {
		const result = method === "GET" ? await readThreadContext<T>(route) : await authenticatedMutation<T>(route, method, body, intent);
		if (!result || result.status < 200 || result.status >= 300) {
			const failure = result?.body as any;
			throw Error(typeof failure?.error === "string" ? failure.error : `Announcement request failed (${result?.status ?? "no response"})`);
		}
		return result.body as T;
	} catch (failure) { if (failure instanceof HTTPError) throw Error(failure.error); throw failure; }
}
export const announcementAPI = {
	list: (query: Record<string, string | number | boolean> = {}) => request<AnnouncementFeed>(url(undefined, query)),
	get: (id: string) => request<Announcement>(url(id)),
	create: (body: AnnouncementInput, intent?: MutationIntent) => request<Announcement>(url(), "POST", body, intent),
	update: (id: string, body: AnnouncementInput & { version: number }, intent?: MutationIntent) => request<Announcement>(url(id), "PUT", body, intent),
	remove: (id: string, version: number, intent?: MutationIntent) => request<void>(url(id), "DELETE", { version }, intent)
};
