import { currentUpload, saveResume, cancelResume, reconcileUploadReceipt, snapshot, isRemoteUploadFile, type PublicUploadOwner, type PublicSession } from './upload-client';

// Thin orchestration over the existing owned transport. No native cookie API,
// page conversion, replacement emulation, automatic cancellation or POST replay.
export function createChapterPagesController(owner: PublicUploadOwner, current: () => Chapter,
  assertCurrent: () => void, api = { currentUpload, saveResume, cancelResume, reconcileUploadReceipt }) {
  const check = () => { owner.assert(); assertCurrent(); if (current().id !== owner.id) throw Error('The page editor target changed.'); };
  function exact(session: PublicSession) {
    check();
    if (session.chapter_id !== current().id || session.manga_id !== current().relationships.manga.id || session.edit_version === undefined) throw Error('This upload belongs to another target or has no verified local edit version. Open its original editor.');
    return session;
  }
  async function eligibility(version: number) {
    check(); const access = await owner.chapterAccess(version); check();
    if (!access.canEditPages) throw Error(access.pageSource === 'protocol' ? 'Original signed protocol pages are immutable.' : 'Current local page-edit permission could not be verified.');
    if (owner.chapterPending()) throw Error('Reconcile the unconfirmed chapter command before editing pages.');
  }
  function acknowledged(session: PublicSession) {
    exact(session);
    const files = session.relationships.files ?? [];
    if (!session.can_mutate || !session.can_retry_commit || !files.length || !files.every(isRemoteUploadFile)) throw Error('Every page must have an acknowledged original-image receipt before saving. Staging was kept.');
    return snapshot(session.draft ?? current(), files);
  }
  async function save(session: PublicSession) {
    const value = acknowledged(session); await eligibility(session.edit_version!);
    const saved = await api.saveResume(session, value, owner); check(); return exact(saved);
  }
  return {
    exact,
    async inspect() { check(); const session = await api.currentUpload(owner); check(); return session; },
    async begin(replace = false) {
      check(); const version = current().version; await eligibility(version);
      const response = await owner.request<PublicSession>(`/upload/begin/${owner.id}`, 'POST', { version, replace }); check();
      if (response.status !== 200 || !response.body) throw Error(`Page-edit admission was refused or unconfirmed (HTTP ${response.status}). Existing staging was kept.`);
      return exact(response.body);
    },
    save,
    async commit(session: PublicSession, termsAccepted: boolean) {
      if (termsAccepted !== true) throw Error('Accept the upload terms before saving pages.');
      const saved = await save(session); check();
      const response = await owner.request<Chapter>(`/upload/${saved.id}/commit`, 'POST', { ...snapshot(saved.draft ?? {}, saved.relationships.files ?? []), termsAccepted }); check();
      if (!response.accepted) throw Error(`Page changes were not confirmed (HTTP ${response.status}). Staging was kept.`);
      return response;
    },
    async cancel(session: PublicSession) { exact(session); await api.cancelResume(session, owner); check(); },
    async reconcile() {
      check();
      if (owner.chapterBeginPending()) { const session = await owner.reconcileChapterBegin(); check(); return exact(session); }
      const id = owner.pendingCommitSession();
      if (!id) { const session = await api.currentUpload(owner); check(); return session; }
      const result = await api.reconcileUploadReceipt(owner, id); check();
      if (!('id' in result)) throw Error('This recovery belongs to a scheduled upload, not this local page edit.');
      return exact(result);
    }
  };
}
