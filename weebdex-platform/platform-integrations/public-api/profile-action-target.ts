import { publicUuid, uuidPattern } from './validation';
type LoadedProfile = { id: string; public_profile_readonly?: boolean; public_profile_route?: { requestedId: string; canonicalId: string }; public_api?: { resource?: { id: string; type: string } } };
// Only the catalogue loader can establish a legacy route binding, after checking
// the server's durable alias and loaded user resource. An arbitrary invalid route
// must continue to fail the existing action validators rather than select a user.
export function profileActionRequestedId(user: LoadedProfile, requestedId: string): string {
 if (uuidPattern.test(requestedId)) return publicUuid(requestedId);
 const binding = user.public_profile_route;
 if (user.public_profile_readonly === true && binding?.requestedId === requestedId && binding.canonicalId === user.id && user.public_api?.resource?.type === 'user' && user.public_api.resource.id === user.id)
  return publicUuid(user.id);
 return requestedId;
}
