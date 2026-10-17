// Configured capability support is separate from a signed service proof and
// account admission. None of those alone proves snapshot acceptance.
import { record } from '../public-api/validation';
export function assertLibraryRelayProfile(input: unknown, relay: { origin: string; serviceKey: string }) {
  const wrapper = record(input), profile = record(wrapper.manga), features = record(profile.features), schemas = record(profile.schemas), kinds = record(profile.event_kinds);
  if (profile.namespace_version !== 1 || profile.base_url !== relay.origin || profile.service_pubkey !== relay.serviceKey || features.public_libraries !== true || kinds.library !== 11999 || !Array.isArray(profile.api_versions) || !profile.api_versions.includes(1) || !Array.isArray(schemas.library_root) || !schemas.library_root.includes(1) || !Array.isArray(schemas.library_chunk) || !schemas.library_chunk.includes(1)) throw Error('This verified instance does not advertise the supported original public-library protocol');
}
export function createLibraryRelayVerifier(deps: { fetch: typeof fetch } = { fetch: (...args) => fetch(...args) }) {
  return async (input: { origin: string; serviceKey: string }, assertCurrent = () => {}) => {
    const relay = { ...input }; assertCurrent();
    if (new URL(relay.origin).origin !== relay.origin || !['http:', 'https:'].includes(new URL(relay.origin).protocol) || !/^[0-9a-f]{64}$/.test(relay.serviceKey)) throw Error('Invalid library relay scope');
    const response = await deps.fetch(relay.origin + '/', { credentials: 'omit', redirect: 'error', cache: 'no-store', signal: AbortSignal.timeout(15000), headers: { accept: 'application/nostr+json' } }); assertCurrent();
    if (!response.ok || !['application/json', 'application/nostr+json'].includes(response.headers.get('content-type')?.split(';', 1)[0].trim().toLowerCase() ?? '')) throw Error('Library relay support could not be verified');
    const reader = response.body?.getReader(); if (!reader) throw Error('Library relay capability response unavailable'); const chunks: Uint8Array[] = []; let size = 0;
    try { for (;;) { const next = await reader.read(); assertCurrent(); if (next.done) break; if (!(next.value instanceof Uint8Array) || next.value.length > 65536 - size) throw Error('Library relay capabilities exceed response budget'); chunks.push(next.value); size += next.value.length; } } catch (error) { await reader.cancel().catch(() => {}); throw error; } finally { reader.releaseLock(); }
    const bytes = new Uint8Array(size); let offset = 0; for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
    assertLibraryRelayProfile(JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)), relay); assertCurrent();
  };
}
export const verifyLibraryRelay = createLibraryRelayVerifier();
