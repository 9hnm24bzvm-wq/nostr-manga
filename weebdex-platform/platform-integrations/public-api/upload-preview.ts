// A component-scoped lease for an already acknowledged remote upload's preview.
// The injected loader must authenticate, hash-check and decode the signed GET
// response before returning its original Blob. Preview failure never mutates the
// acknowledged remote file, queues a mutation, or retries the GET automatically.
export interface UploadPreviewContext {
  readonly sessionId: string;
  readonly fileId: string;
  readonly fileHash: string;
  readonly fileSize: number;
  readonly mimeType: 'image/png' | 'image/jpeg' | 'image/gif' | 'image/webp';
  readonly owner: object;
}
export interface UploadLocalPreview {
  readonly url: string;
  // A transferred URL must have exactly one owner: this lease. The caller must
  // relinquish its own cleanup and must not transfer a URL used by another live
  // lease. Borrowed thumbnails remain their creator's cleanup responsibility.
  readonly ownership: 'borrowed' | 'transferred';
}
export interface UploadPreviewState {
  readonly key: string | null;
  readonly url: string | null;
  readonly status: 'idle' | 'loading' | 'ready' | 'error';
  readonly error: Error | null;
}
export interface UploadPreviewOptions {
  readonly load: (context: Readonly<UploadPreviewContext>) => Promise<Blob>;
  // Include the live document/session/file and signing-owner lifetime here.
  readonly isCurrent: (context: Readonly<UploadPreviewContext>) => boolean;
  readonly onChange: (state: Readonly<UploadPreviewState>) => void;
  readonly url?: { createObjectURL(blob: Blob): string; revokeObjectURL(url: string): void };
  readonly isBlob?: (value: unknown) => value is Blob;
}

// This module-private registry prevents an accidental second live lease from
// taking cleanup ownership of the first lease's URL. It never replaces URL APIs.
const previewUrlOwners = new Map<string, { owner: symbol; release: () => void }>();

// Route native page cleanup to the one live lease that owns this URL. False
// leaves cleanup to the native/local creator. This does not authenticate a URL,
// change remote state, or replace the component's required dispose() on unmount.
export function revokeUploadPreviewUrl(url: string): boolean {
  const registration = previewUrlOwners.get(url);
  if (!registration) return false;
  registration.release();
  return true;
}

export function uploadPreviewKey(context: UploadPreviewContext): string {
  if (!context || typeof context.sessionId !== 'string' || !context.sessionId || context.sessionId.length > 128 ||
      typeof context.fileId !== 'string' || !context.fileId || context.fileId.length > 128 ||
      typeof context.fileHash !== 'string' || !/^[0-9a-f]{64}$/.test(context.fileHash) || !Number.isSafeInteger(context.fileSize) || context.fileSize < 1 || context.fileSize > 25 * 1024 * 1024 ||
      !['image/png', 'image/jpeg', 'image/gif', 'image/webp'].includes(context.mimeType) || !context.owner || typeof context.owner !== 'object')
    throw Error('An exact acknowledged upload file and owner are required for preview');
  return JSON.stringify([context.sessionId, context.fileId, context.fileHash, context.fileSize, context.mimeType]);
}

export function createUploadPreviewLease(options: UploadPreviewOptions) {
  const urls = options.url ?? URL;
  const isBlob = options.isBlob ?? ((value: unknown): value is Blob => value instanceof Blob);
  const owned = new Set<string>();
  const ownership = Symbol('upload preview lease');
  let generation = 0, disposed = false;
  let active: Readonly<UploadPreviewContext> | null = null;
  let displayed: { url: string; owned: boolean } | null = null;
  let state: Readonly<UploadPreviewState> = Object.freeze({ key: null, url: null, status: 'idle', error: null });
  let pending: Promise<void> = Promise.resolve();

  const own = (url: string) => {
    const registration = previewUrlOwners.get(url);
    if (registration && registration.owner !== ownership) throw Error('This Blob URL already belongs to another live upload preview lease');
    previewUrlOwners.set(url, { owner: ownership, release: () => release(url) });
    owned.add(url);
  };
  const release = (url: string) => {
    if (!owned.delete(url)) return;
    if (previewUrlOwners.get(url)?.owner === ownership) previewUrlOwners.delete(url);
    urls.revokeObjectURL(url);
  };
  const current = (context: Readonly<UploadPreviewContext>, version: number) => {
    if (disposed || generation !== version || active !== context) return false;
    try { return options.isCurrent(context); } catch { return false; }
  };
  const publish = (next: UploadPreviewState) => { state = Object.freeze(next); options.onChange(state); };
  const abandon = (context: Readonly<UploadPreviewContext>, version: number) => {
    if (active !== context || generation !== version) return;
    generation++;
    active = null;
    displayed = null;
    for (const url of [...owned]) release(url);
    state = Object.freeze({ key: null, url: null, status: 'idle', error: null });
  };
  const replace = (next: { url: string; owned: boolean } | null, nextState: UploadPreviewState) => {
    const before = displayed;
    displayed = next;
    try { publish(nextState); } finally { if (before?.owned && before.url !== next?.url) release(before.url); }
  };

  async function load(context: Readonly<UploadPreviewContext>, key: string, version: number): Promise<void> {
    try {
      if (!current(context, version)) { abandon(context, version); return; }
      const blob = await options.load(context);
      if (!current(context, version)) { abandon(context, version); return; } // Late Blobs allocate no URL.
      if (!isBlob(blob) || blob.size !== context.fileSize || blob.type !== context.mimeType)
        throw Error('The signed upload preview is not the expected original image Blob');
      const url = urls.createObjectURL(blob);
      own(url);
      if (!current(context, version)) { release(url); abandon(context, version); return; }
      replace({ url, owned: true }, { key, url, status: 'ready', error: null });
    } catch (error) {
      if (!current(context, version)) { abandon(context, version); return; }
      publish({ key, url: displayed?.url ?? null, status: 'error', error: error instanceof Error ? error : Error('The signed upload preview could not be loaded') });
    }
  }

  function update(context: UploadPreviewContext | null, localPreview?: UploadLocalPreview): Promise<void> {
    if (disposed) return Promise.resolve();
    const key = context === null ? null : uploadPreviewKey(context);
    if (context && active && state.key === key && active.owner === context.owner && current(active, generation)) return pending;
    if (localPreview && (typeof localPreview.url !== 'string' || !localPreview.url.startsWith('blob:') || !['borrowed', 'transferred'].includes(localPreview.ownership)))
      throw Error('A local preview must explicitly identify its Blob URL ownership');
    const next = context && localPreview ? { url: localPreview.url, owned: localPreview.ownership === 'transferred' || owned.has(localPreview.url) } : null;
    if (next?.owned) own(next.url);
    const version = ++generation;
    active = context === null ? null : Object.freeze({ ...context });
    if (!active) {
      replace(null, { key: null, url: null, status: 'idle', error: null });
      pending = Promise.resolve();
      return pending;
    }
    if (!current(active, version)) {
      const before = displayed;
      displayed = null;
      if (before?.owned) release(before.url);
      if (next?.owned) release(next.url);
      state = Object.freeze({ key, url: null, status: 'idle', error: null });
      pending = Promise.resolve();
      return pending;
    }
    const captured = active;
    // Defer invocation so synchronous replacement/disposal can prevent even a
    // signed preview read. This is one read, with no implicit replay on error.
    const operation = Promise.resolve().then(() => load(captured, uploadPreviewKey(captured), version));
    pending = operation;
    replace(next, { key, url: next?.url ?? null, status: 'loading', error: null });
    return operation;
  }

  function dispose(): void {
    if (disposed) return;
    disposed = true;
    generation++;
    active = null;
    displayed = null;
    for (const url of [...owned]) release(url);
    state = Object.freeze({ key: null, url: null, status: 'idle', error: null });
  }

  // Cleanup ownership only; this accessor makes no content/authentication claim.
  return { update, dispose, getState: () => state, owns: (url: string) => owned.has(url) };
}
