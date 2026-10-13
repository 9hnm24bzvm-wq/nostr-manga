// Discard only acknowledged/explicitly local resources, preserving retained shared URLs.
export function releaseDiscardedThumbnails(discarded: UploadSessionFile[], retained: UploadSessionFile[]) {
  const kept = new Set(retained.map(page => page.thumbnail));
  for (const thumbnail of new Set(discarded.map(page => page.thumbnail))) if (thumbnail?.startsWith('blob:') && !kept.has(thumbnail)) URL.revokeObjectURL(thumbnail);
}
