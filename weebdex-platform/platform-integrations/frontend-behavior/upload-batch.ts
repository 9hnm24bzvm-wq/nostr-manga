// One owned upload batch, with truthful terminal placeholders and read-only recovery.
// A transport failure is never evidence that a non-idempotent POST was rejected.
import { createContext } from 'svelte';
import { Error as VisibleError } from '$lib/constants/Error';
import HTTP from '$lib/HTTP';
import { PUBLIC_API_URL, PUBLIC_UPLOAD_URL } from '$env/static/public';
import { currentUpload } from './upload-resume';
import type { targetOwner } from './target-owner';
type Owner = ReturnType<typeof targetOwner>;
export const [getUploadOwner, setUploadOwner] = createContext<Owner>();
declare global {
  interface UploadSessionFile {
    upload_recovery?: { state: 'unsent' | 'rejected' | 'unknown'; session: string; before_ids: string[]; hash?: string };
  }
}
const unknownText = 'Outcome unknown. This page may already be staged. Replace/Remove first checks current owner staging; if no exact receipt is available, reload or explicitly cancel the staged upload. No POST was replayed.';
const sha256 = async (file: File) => [...new Uint8Array(await crypto.subtle.digest('SHA-256', await file.arrayBuffer()))].map(byte => byte.toString(16).padStart(2, '0')).join('');
const remotePage = (value: UploadSessionFile, local?: UploadSessionFile): UploadSessionFile => ({ ...value, file: undefined, error: undefined, upload_recovery: undefined, thumbnail: local?.thumbnail ?? `${PUBLIC_UPLOAD_URL}/${encodeURIComponent(value.file_name)}` });
const validRemote = (row: UploadSessionFile) => row && /^[A-Za-z0-9]{10}$/.test(row.id) && /^[0-9a-f]{64}$/.test(row.file_hash) && typeof row.file_name === 'string' && !!row.file_name && /^\d+$/.test(row.file_size);
function matchCurrent(page: UploadSessionFile, fresh: UploadSession) {
  const recovery = page.upload_recovery;
  if (!recovery || recovery.state !== 'unknown') return page;
  if (fresh.id !== recovery.session) throw new VisibleError('Upload changed', 'The original staging session is unavailable. No page was removed or resent.');
  const matches = (fresh.relationships.files ?? []).filter(row => validRemote(row) && !recovery.before_ids.includes(row.id) && row.file_hash === recovery.hash && row.original_file_name === page.original_file_name);
  if (matches.length !== 1) throw new VisibleError('Page outcome is still unknown', unknownText);
  return remotePage(matches[0], page);
}
export async function resolveUploadPage(session: UploadSession, page: UploadSessionFile, owner: Owner) {
  owner.assert();
  if (page.upload_recovery?.state !== 'unknown') return page;
  const fresh = await currentUpload(owner); owner.assert();
  if (!fresh || fresh.id !== session.id) throw new VisibleError('Page outcome is still unknown', unknownText);
  return matchCurrent(page, fresh);
}
export function markUnsentUploadPages(pages: UploadSessionFile[], session: string) {
  for (const page of pages) if (page.file && !page.error) {
    page.error = 'Not sent because an earlier batch failed. Remove this local placeholder or select a replacement to try it explicitly.';
    page.upload_recovery = { state: 'unsent', session, before_ids: [] };
  }
}
export async function runUploadBatch(session: UploadSession, chunk: UploadSessionFile[], owner: Owner, onProgress?: (delta: number) => void): Promise<UploadSessionFile[]> {
  owner.assert();
  const sessionId = session.id;
  if (chunk.some(page => page.upload_recovery?.state === 'unknown')) throw new VisibleError('Page cannot be replayed', unknownText);
  for (const page of chunk) page.upload_recovery = { state: 'unsent', session: sessionId, before_ids: [] };
  let sent = false;
  try {
    const before = await currentUpload(owner); owner.assert();
    if (!before || before.id !== sessionId || before.can_mutate === false) throw new VisibleError('Upload unavailable', 'Current mutable owner staging was not confirmed. No files were sent.');
    const beforeIds = (before.relationships.files ?? []).map(row => row.id);
    for (const page of chunk) {
      if (!page.file || page.upload_recovery?.state === 'unknown') throw new VisibleError('Page cannot be replayed', unknownText);
      const hash = await sha256(page.file); owner.assert();
      page.upload_recovery = { state: 'unknown', session: sessionId, before_ids: beforeIds, hash };
    }
    const form = new FormData(); for (const page of chunk) form.append('file', page.file!);
    // Request progress is owned by this single non-replayed POST.
    const http = new HTTP<UploadSessionFile[]>(`${PUBLIC_API_URL.replace(/\/$/, '')}/upload/${encodeURIComponent(sessionId)}`, form);
    http.setWithCredentials(true); http.setRequestHeader('X-WeebDex-Expected-Account', owner.actor);
    if (onProgress) { let loaded = 0; http.addUploadListener('progress', event => { if (owner.alive() && event.lengthComputable) { const delta = Math.max(0, event.loaded - loaded); loaded = event.loaded; onProgress(delta); } }); }
    owner.assert(); sent = true;
    const response = await http.POST(); owner.assert();
    if (response?.status !== 200 || !Array.isArray(response.body) || response.body.length !== chunk.length) throw new VisibleError('Upload was not confirmed', unknownText);
    return response.body.map((row, index) => {
      const local = chunk[index];
      if (typeof row.error === 'string') return { ...local, error: row.error, upload_recovery: { state: 'rejected', session: sessionId, before_ids: beforeIds } };
      if (!validRemote(row)) throw new VisibleError('Upload receipt is invalid', unknownText);
      return remotePage(row, local);
    });
  } catch (error) {
    if (owner.alive()) {
      // Reads can confirm exact newly staged bytes, but absence cannot prove a
      // disconnected POST will never finish. Unknown placeholders stay fenced.
      let fresh: UploadSession | undefined;
      if (sent) try { fresh = await currentUpload(owner); } catch { /* Keep uncertainty. */ }
      owner.assert();
      for (const page of chunk) {
        if (sent && fresh?.id === sessionId) {
          try { const confirmed = matchCurrent(page, fresh); if (confirmed !== page) { Object.assign(page, confirmed); continue; } } catch { /* Exact receipt absent/ambiguous. */ }
        }
        page.error = sent ? unknownText : 'Not sent. Current owner staging could not be confirmed; this local placeholder can be removed or replaced explicitly.';
        if (!sent) page.upload_recovery = { state: 'unsent', session: sessionId, before_ids: [] };
      }
      if (sent && chunk.every(page => !page.file && !page.error)) return chunk;
    }
    throw error;
  }
}
