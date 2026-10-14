// Successful event receipts must describe the exact action. Unknown JSON and
// malformed boolean flags never become an accepted/projected UI result.
export type PublicActionReceipt = { id: string; projected?: boolean; superseded?: boolean };
export function isPublicActionReceipt(value: unknown): value is PublicActionReceipt {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    && 'id' in value && typeof value.id === 'string' && /^[0-9a-f]{64}$/.test(value.id)
    && (!('projected' in value) || typeof value.projected === 'boolean')
    && (!('superseded' in value) || typeof value.superseded === 'boolean');
}
