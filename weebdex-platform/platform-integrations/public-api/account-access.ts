// Presentation-only classification of the server's exact account prerequisites.
// A generic 403 is never evidence that enrolling or linking will grant access.
import { PublicApiContractError } from './validation';
export type AccountAccessReason = 'join' | 'link' | 'inactive' | 'permission' | 'reconnect' | 'unavailable';
export class AccountAccessError extends PublicApiContractError {
  readonly accessReason: AccountAccessReason;
  constructor(reason: AccountAccessReason, message: string, status: number) {
    super(message, status); this.name = 'AccountAccessError'; this.accessReason = reason;
  }
}
export function accountAccessError(status: number, body: unknown, action = 'use this feature'): AccountAccessError {
  // Only a complete, unambiguous API error identifies a recoverable prerequisite.
  const envelope = body && typeof body === 'object' && !Array.isArray(body) ? body as Record<string, unknown> : {};
  const errors = envelope.result === 'error' && Array.isArray(envelope.errors) ? envelope.errors : [];
  const row = errors.length === 1 && errors[0] && typeof errors[0] === 'object' ? errors[0] as Record<string, unknown> : {};
  const detail = row.status === status && typeof row.detail === 'string' ? row.detail : '';
  if (status === 403 && detail === 'The Nostr principal is not admitted on this instance') return new AccountAccessError('join', 'Join this site with your public identity before continuing. Joining is a separate step from connecting your recovery key.', status);
  if (status === 403 && detail === 'An explicit native account binding is required') return new AccountAccessError('link', 'This identity is not linked to a site account. Ask the site team to link it, or use an identity that is already linked. Joining again or signing in by email will not link it automatically.', status);
  if (status === 403 && detail === 'This bound account is not active and verified') return new AccountAccessError('inactive', 'This identity’s account is not active and verified. If it is your email account, check your email verification; otherwise ask the site team for help. Creating another key will not restore its access.', status);
  if (status === 401) return new AccountAccessError('reconnect', 'Your identity could not be confirmed. Reconnect it, then try again.', status);
  if (status === 403) return new AccountAccessError('permission', `This account is not allowed to ${action}. Ask the site team about access, or use a different authorized identity. Joining again will not change these permissions.`, status);
  return new AccountAccessError('unavailable', 'The site could not check your account right now. Try again in a moment; no account change was made.', status);
}
export function accountAccessReason(cause: unknown): AccountAccessReason | undefined {
  return cause instanceof AccountAccessError ? cause.accessReason : undefined;
}
