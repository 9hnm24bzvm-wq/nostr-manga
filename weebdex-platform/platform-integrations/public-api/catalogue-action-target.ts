import { publicUuid, uuidPattern } from './validation';
export type CatalogueActionKind = 'manga' | 'group';
type LoadedActionResource = { id?: unknown; public_api?: { resource?: { id?: unknown; type?: unknown }; action_route?: { kind?: unknown; requestedId?: unknown; canonicalId?: unknown } } };
// Only the detail loader records a route after verifying both its durable alias
// and the typed resource returned for that UUID. A raw server attribute, another
// route, stale resource, or cross-kind binding never normalizes an action target.
export function catalogueActionRequestedId(view: unknown, kind: CatalogueActionKind, requestedId: string): string {
 const loaded = view && typeof view === 'object' ? view as LoadedActionResource : undefined;
 const binding = loaded?.public_api?.action_route, resource = loaded?.public_api?.resource;
 if (loaded && (kind === 'manga' || kind === 'group') && binding?.kind === kind && binding.requestedId === requestedId && binding.canonicalId === loaded?.id && resource?.id === loaded?.id && resource?.type === (kind === 'manga' ? 'manga' : 'scanlation_group'))
  return publicUuid(loaded.id);
 return uuidPattern.test(requestedId) ? publicUuid(requestedId) : requestedId;
}
