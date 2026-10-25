// Public presentation only. A profile is never a site session or write authority.
import type { ViewProvenance } from './types';
import { publicUuid, resource, record, PublicApiContractError } from './validation';
export interface PublicUserView {
  id: string;
  name: string;
  description?: string; discord?: string; twitter?: string; website?: string;
  version?: number;
  roles: string[];
  account_state?: "disabled";
  relationships: { thread?: Record<string, unknown> | null };
  public_api: ViewProvenance;
  public_profile_readonly: true;
  public_lists_available?: true;
  public_account_follow_available?: true;
}
declare global { interface User { public_profile_readonly?: true; public_lists_available?: true; public_account_follow_available?: true; } }
export function normalizePublicUser(input: unknown): PublicUserView {
  const row = resource(input, 'user'), attrs = row.attributes;
  if (typeof attrs.username !== 'string' || !attrs.username || attrs.username.length > 256 || !Array.isArray(attrs.roles) || attrs.roles.length > 128 || attrs.roles.some(role => typeof role !== 'string' || !role || new TextEncoder().encode(role).byteLength > 128)) throw new PublicApiContractError('Invalid public user presentation');
  const output: PublicUserView = { id: publicUuid(row.id), name: attrs.username, roles: [...attrs.roles] as string[], relationships: {}, public_api: { resource: row }, public_profile_readonly: true };
  if (attrs.version !== undefined) {
    if (!Number.isSafeInteger(attrs.version) || Number(attrs.version) < 1) throw new PublicApiContractError('Invalid public user revision');
    output.version = Number(attrs.version);
  }
  // Some protocol-publisher profiles have no mutable revision. Preserve omission.
  const provenance = record(row).nostr;
  if (provenance !== undefined) {const source=record(provenance, 'public user provenance');if(source.identity==='native_public_user'){if(attrs.accountState!==undefined&&attrs.accountState!=='disabled')throw new PublicApiContractError('Invalid native account state');if(attrs.accountState==='disabled'){if(['description','discord','twitter','website'].some(field=>attrs[field]!==undefined)||attrs.roles.length)throw new PublicApiContractError('Disabled account disclosed profile fields');output.account_state='disabled';output.name+=' (disabled)';}else output.public_account_follow_available=true;for(const field of ['description','discord','twitter','website'] as const)if(attrs[field]!==undefined){if(typeof attrs[field]!=='string'||new TextEncoder().encode(attrs[field] as string).byteLength>(field==='description'?20000:2048))throw new PublicApiContractError('Invalid native public profile field');output[field]=attrs[field] as string;}}if(source.identity==='protocol_publisher'&&source.known===true&&typeof source.pubkey==='string'&&/^[0-9a-f]{64}$/.test(source.pubkey))output.public_lists_available=true;}
  return output;
}
