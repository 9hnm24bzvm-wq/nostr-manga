// Public presentation only. A profile is never a site session or write authority.
import type { ViewProvenance } from './types';
import { publicUuid, resource, record, PublicApiContractError } from './validation';
export interface PublicUserView {
  id: string;
  name: string;
  version?: number;
  roles: string[];
  relationships: Record<string, never>;
  public_api: ViewProvenance;
  public_profile_readonly: true;
  public_lists_available?: true;
  public_account_follow_available?: true;
}
declare global { interface User { public_profile_readonly?: true; public_lists_available?: true; public_account_follow_available?: true; } }
export function normalizePublicUser(input: unknown): PublicUserView {
  const row = resource(input, 'user'), attrs = row.attributes;
  if (typeof attrs.username !== 'string' || !attrs.username || attrs.username.length > 256 || !Array.isArray(attrs.roles) || attrs.roles.length > 100 || attrs.roles.some(role => typeof role !== 'string')) throw new PublicApiContractError('Invalid public user presentation');
  const output: PublicUserView = { id: publicUuid(row.id), name: attrs.username, roles: [...attrs.roles] as string[], relationships: {}, public_api: { resource: row }, public_profile_readonly: true };
  if (attrs.version !== undefined) {
    if (!Number.isSafeInteger(attrs.version) || Number(attrs.version) < 1) throw new PublicApiContractError('Invalid public user revision');
    output.version = Number(attrs.version);
  }
  // Some protocol-publisher profiles have no mutable revision. Preserve omission.
  const provenance = record(row).nostr;
  if (provenance !== undefined) {const source=record(provenance, 'public user provenance');if(source.identity==='native_public_user')output.public_account_follow_available=true;if(source.identity==='protocol_publisher'&&source.known===true&&typeof source.pubkey==='string'&&/^[0-9a-f]{64}$/.test(source.pubkey))output.public_lists_available=true;}
  return output;
}
