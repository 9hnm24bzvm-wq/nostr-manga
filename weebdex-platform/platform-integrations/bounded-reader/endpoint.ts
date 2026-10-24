import type { ReaderEndpoint } from './client';
export function readerEndpoint(publicMode: boolean, configuredNativeApi: string, pageHref: string): ReaderEndpoint {
  const page = new URL(pageHref);
  if (publicMode) return { origin: page.origin, base: '/api/v1', profile: 'public-retained-v1' };
  // This is the build-configured native API, never a route/query/user-selected URL.
  // Same-origin credentials exactly retain XHR GET behavior, including omission
  // of cross-origin cookies. No credentials:include or redirect widening.
  const endpoint = new URL(configuredNativeApi || '/', page);
  if (!['http:', 'https:'].includes(endpoint.protocol) || endpoint.username || endpoint.password || endpoint.search || endpoint.hash || /[\\?#]/.test(configuredNativeApi) || !endpoint.pathname.startsWith('/') || endpoint.pathname.includes('//')) throw Error('Invalid configured native reader API endpoint');
  return { origin: endpoint.origin, base: endpoint.pathname.replace(/\/$/, ''), profile: 'native-aggregate-v1' };
}

export function identityChanged(endpoint: ReaderEndpoint, verified: { origin: string; catalogNamespace: string; serviceKey: string | null } | undefined, observed: { url: string; scope: { origin: string; catalogNamespace: string | null; serviceKey: string | null } } | undefined): boolean {
  if (!verified || !observed) return false;
  const capabilityUrl = new URL((endpoint.profile === 'public-retained-v1' ? '/api' : endpoint.base) + '/protocol/capabilities', endpoint.origin).href;
  if (observed.url !== capabilityUrl) return false; // A different configured catalogue is not this source.
  return observed.scope.origin !== verified.origin || observed.scope.catalogNamespace !== verified.catalogNamespace || observed.scope.serviceKey !== verified.serviceKey;
}
