type DiscussionResource = { id?: unknown; relationships?: { thread?: { id?: unknown; relationships?: { stats?: { replies?: unknown } } } | null } };
const resourceId = (value: unknown): value is string => typeof value === 'string' && /^(?:[a-zA-Z0-9]{10}|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$/i.test(value);
export function feedCommentsDestination(chapter: DiscussionResource | undefined, manga: DiscussionResource | undefined) {
  const thread = chapter?.relationships?.thread;
  if (typeof thread?.id === 'string' && /^[a-zA-Z0-9_-]{1,128}$/.test(thread.id)) {
    const replies = thread.relationships?.stats?.replies;
    return { href: '/forums/threads/' + encodeURIComponent(thread.id), label: 'Comments', title: 'Go to comments', replies: Number.isSafeInteger(replies) && Number(replies) > 0 ? Number(replies) : undefined };
  }
  // A known parent gives a real title-comments route, not an invented chapter
  // thread or a fabricated reply count. Missing hydration is not absence.
  if (resourceId(manga?.id)) return { href: '/title/' + manga.id + '?tab=comments', label: 'Title comments', title: 'Open comments on this title', replies: undefined };
  return undefined;
}
