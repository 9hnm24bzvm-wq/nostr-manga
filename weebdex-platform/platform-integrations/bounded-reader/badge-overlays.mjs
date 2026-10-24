import fs from 'node:fs';
import { createHash } from 'node:crypto';
export const badgePatches = JSON.parse(fs.readFileSync(new URL('./badge-anchors.json', import.meta.url), 'utf8'));
const approved = new Map([
  ['src/routes/(app)/title/MangaChapterListSection.svelte', ['visible badge uncertainty and bounded retry']],
  ['src/lib/components/ChapterListEntry.svelte', ['unknown is not unread border', 'unknown disables read mutation', 'read status action title reflects unknown', 'unknown read status is not unread icon']]
]);
export function assertBadgeExceptionScope() {
  const markup = badgePatches.filter(patch => patch.markup);
  if (markup.length !== 5 || markup.some(patch => patch.count !== 1 || !approved.get(patch.path)?.includes(patch.label)) || [...approved].some(([path, labels]) => labels.some(label => markup.filter(patch => patch.path === path && patch.label === label).length !== 1))) throw Error('Unreviewed visible badge markup extension');
}
export function applyBadgeOverlays(input, relative, original) {
  let code = input;
  for (const patch of badgePatches.filter(patch => patch.path === relative)) {
    if (createHash('sha256').update(original).digest('hex') !== patch.sha256 || code.split(patch.before).length - 1 !== patch.count) throw Error('Visible badge source or exact anchor changed: ' + patch.label);
    code = code.split(patch.before).join(patch.after);
  }
  return code;
}
export function reverseBadgeOverlays(output, relative) {
  let code = output;
  for (const patch of badgePatches.filter(patch => patch.path === relative).reverse()) {
    if (code.split(patch.after).length - 1 !== patch.count) throw Error('Visible badge reverse anchor changed: ' + patch.label);
    code = code.split(patch.after).join(patch.before);
  }
  return code;
}
