// NEW fixture-only HTTP boundary. No application state is created or replaced here.
type LoadInput = { fetch: typeof fetch; url: URL; params: Record<string, string> };
export async function fixtureRouteData(input: LoadInput, kind: 'title' | 'chapter') {
  const query = new URLSearchParams({ kind, id: input.params.id ?? '', page: input.params.page ?? '', search: input.url.search });
  // The retained initializer intentionally prefixes URLSearchParams.toString with ?.
  const encoded = query.toString().replace(/^\?/, '');
  const response = await input.fetch(`/__badge_fixture/route?${encoded}`, { credentials: 'omit', cache: 'no-store', redirect: 'error' });
  if (!response.ok) throw new Error(`Fixture route HTTP ${response.status}`);
  const text = await response.text();
  if (new TextEncoder().encode(text).byteLength > 262144) throw new Error('Fixture route data exceeds 256 KiB');
  const data = JSON.parse(text);
  if (!data || typeof data !== 'object' || Array.isArray(data)) throw new Error('Invalid fixture route data');
  if (kind === 'title' && (!data.manga || data.manga.id !== input.params.id)) throw new Error('Fixture title identity mismatch');
  return data;
}
