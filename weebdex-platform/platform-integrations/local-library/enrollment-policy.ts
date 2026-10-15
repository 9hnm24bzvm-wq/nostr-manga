// RECONSTRUCTED after the 2026-10-15 environment reset; revalidate before release.
// Read-only instance enrollment discovery. Never select a signer or send a key.
import { capabilities, assertAllowedRelay, type Relay } from './publication';
import { proveService, bytesResponse } from './protocol/nostr.js';

export async function enrollmentPolicy(relay: Relay, assertCurrent: () => void = () => {}): Promise<{ enabled: boolean }> {
  assertCurrent();
  const policy = await capabilities(); assertCurrent();
  if (!policy.available) throw Error('Instance enrollment policy is unavailable');
  const canonical = assertAllowedRelay(relay.origin, policy);
  await proveService(canonical, relay.serviceKey); assertCurrent();
  const response = await fetch(canonical + '/enrollment', {
    method: 'GET', credentials: 'omit', redirect: 'error', cache: 'no-store',
    signal: AbortSignal.timeout(15000),
  });
  assertCurrent();
  if (!response.ok) throw Error('Instance enrollment policy is unavailable');
  const bytes = await bytesResponse(response, 4096); assertCurrent();
  const value: unknown = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw Error('Invalid instance enrollment policy');
  const document = value as Record<string, unknown>;
  if (typeof document.enabled !== 'boolean' || document.mode !== (document.enabled ? 'self_registration' : 'operator_only') || document.endpoint !== '/enrollment' || document.scope !== 'personal_account_only') throw Error('Invalid instance enrollment policy');
  assertCurrent();
  return { enabled: document.enabled };
}
