// Private, versioned staging metadata. No signed command/key material is exposed.
import { PUBLIC_API_URL, PUBLIC_UPLOAD_URL } from "$env/static/public";
import API from "$lib/API";
import HTTP from "$lib/HTTP";
import { Error as UserVisibleError } from "$lib/constants/Error";
type Owner = { actor: string; assert: () => void };
export type UploadSnapshot = { draft: ChapterDraft; page_order: string[] };
export const snapshot = (draft: ChapterDraft, files: UploadSessionFile[]): UploadSnapshot => ({
	draft: Object.fromEntries(["volume", "chapter", "title", "language"].flatMap(key => {
		const value = draft[key as keyof ChapterDraft];
		return typeof value === "string" ? [[key, value]] : [];
	})), page_order: files.map(file => file.id)
});
export const signature = (value: UploadSnapshot) => JSON.stringify(value);
const sameDraft = (a?: ChapterDraft | null, b?: ChapterDraft | null) => signature(snapshot(a ?? {}, [])) === signature(snapshot(b ?? {}, []));
export function resumeError(status?: number, body?: unknown): UserVisibleError {
	const detail = typeof body === "object" && body && "error" in body && typeof body.error === "string" ? body.error : undefined;
	return new UserVisibleError(status === 409 ? "Upload session changed" : "Upload could not be saved",
		detail ?? (status === 409 ? "Reload the current upload before retrying. Its staged files and any prepared publication were kept." : "The server did not confirm this operation. Your staged upload has not been discarded."));
}
export async function currentUpload(owner?: Owner): Promise<UploadSession | undefined> {
	owner?.assert();
	const http = owner ? new HTTP<UploadSession>(`${PUBLIC_API_URL}/upload`) : undefined;
	if (http) { http.setWithCredentials(true); http.setRequestHeader("X-WeebDex-Expected-Account", owner!.actor); }
	const result = await (http ? http.GET() : API.UploadSession.Get()); owner?.assert();
	if (result?.status === 204) return undefined;
	if (result?.status !== 200 || !result.body) throw resumeError(result?.status, result?.body);
	return result.body;
}
export function prepareResume(session: UploadSession): UploadSession {
	const positions = new Map((session.page_order ?? []).map((id, index) => [id, index]));
	session.relationships.files = (session.relationships.files ?? []).map(file => ({ ...file, thumbnail: `${PUBLIC_UPLOAD_URL}/${encodeURIComponent(file.file_name)}` }))
		.sort((a, b) => (positions.get(a.id) ?? Number.MAX_SAFE_INTEGER) - (positions.get(b.id) ?? Number.MAX_SAFE_INTEGER));
	return session;
}
export function resumeNotice(session: UploadSession): string {
	if (session.is_committed) return "The saved publication receipt was restored. Reader navigation is available only after local materialization.";
	if (session.can_mutate === false) return session.can_retry_commit === false
		? "Your saved publication is retained, but this instance cannot retry it with its current authorization/configuration. No staged work was discarded."
		: "Your immutable publication was restored. Editing and cancellation are locked; Upload retries only its exact saved metadata and page order.";
	return "Your staged files, chapter metadata and page order were restored. Changes are saved privately on this instance.";
}
export async function saveResume(session: UploadSession, value: UploadSnapshot, owner?: Owner): Promise<UploadSession> {
	owner?.assert(); const fresh = await currentUpload(owner); owner?.assert();
	if (!fresh || fresh.id !== session.id || fresh.can_mutate === false) throw resumeError(409);
	// File operations update version independently. Accept that version refresh,
	// but never silently overwrite another tab's acknowledged form/order changes.
	if (!sameDraft(fresh.draft, session.draft) && !sameDraft(fresh.draft, value.draft)) throw resumeError(409);
	const known = new Set(session.page_order ?? []);
	const previousOrder = (session.page_order ?? []).filter(id => (fresh.page_order ?? []).includes(id));
	const remoteOrder = (fresh.page_order ?? []).filter(id => known.has(id));
	if (JSON.stringify(previousOrder) !== JSON.stringify(remoteOrder) && JSON.stringify(fresh.page_order ?? []) !== JSON.stringify(value.page_order)) throw resumeError(409);
	const fileIds = new Set((fresh.relationships.files ?? []).map(file => file.id));
	if (value.page_order.length !== fileIds.size || new Set(value.page_order).size !== fileIds.size || value.page_order.some(id => !fileIds.has(id))) throw resumeError(409);
	const http = new HTTP<UploadSession>(`${PUBLIC_API_URL}/upload/${encodeURIComponent(session.id)}`, { version: fresh.version, ...value });
	http.setWithCredentials(true); if (owner) http.setRequestHeader("X-WeebDex-Expected-Account", owner.actor); owner?.assert();
	const result = await http.PATCH(); owner?.assert();
	if (result?.status !== 200 || !result.body) throw resumeError(result?.status, result?.body);
	return result.body;
}
export function acknowledge(session: UploadSession, saved: UploadSession) {
	for (const key of ["version", "draft", "page_order", "can_mutate", "can_cancel", "can_retry_commit", "publication_status", "state", "is_expired", "expires_at"] as const)
		Object.assign(session, { [key]: saved[key] });
}
export function immutableCommit(session: UploadSession, value: UploadSnapshot): UploadSnapshot {
	if (session.can_retry_commit === false || session.is_committed) throw new UserVisibleError("Publication cannot be retried", resumeNotice(session));
	if (session.can_mutate !== false) return value;
	if (!session.draft || !session.page_order?.length) throw new UserVisibleError("Saved publication is incomplete", "No replacement request was sent. Reload its durable receipt before retrying.");
	return snapshot(session.draft, session.page_order.map(id => ({ id } as UploadSessionFile)));
}
export async function cancelResume(session: UploadSession, owner?: Owner) {
	if (session.can_cancel === false) throw new UserVisibleError("Upload cannot be cancelled", resumeNotice(session));
	owner?.assert();
	const http = owner ? new HTTP(`${PUBLIC_API_URL}/upload/${encodeURIComponent(session.id)}`) : undefined;
	if (http) { http.setWithCredentials(true); http.setRequestHeader("X-WeebDex-Expected-Account", owner!.actor); }
	const result = await (http ? http.DELETE() : API.UploadSession.Delete(session.id)); owner?.assert();
	if (result?.status !== 204) throw resumeError(result?.status, result?.body);
}
