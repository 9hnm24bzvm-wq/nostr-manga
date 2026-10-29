// Presentation-only recovery: never submits a registration or retains a password.
import type { RegistrationFlow } from '@ory/client';
export type RegistrationDetails = { email: string; username: string };
export function registrationDetails(form?: HTMLFormElement): RegistrationDetails {
 const value = (name: string) => {
  const input = form?.elements.namedItem(name);
  return input instanceof HTMLInputElement ? input.value : '';
 };
 return { email: value('email'), username: value('username') };
}
export function clearRegistrationPassword(form?: HTMLFormElement) {
 const input = form?.elements.namedItem('password');
 if (input instanceof HTMLInputElement) input.value = '';
}
export function retainRegistrationDetails(flow: RegistrationFlow, details: Partial<RegistrationDetails> = {}): RegistrationFlow {
 return { ...flow, ui: { ...flow.ui, nodes: flow.ui.nodes.map(node => {
  const attributes = node.attributes;
  if (attributes.node_type !== 'input') return node;
  if (attributes.name === 'password') return { ...node, attributes: { ...attributes, value: '' } };
  const key = attributes.name === 'traits.email' ? 'email' : attributes.name === 'traits.username' ? 'username' : undefined;
  if (key && attributes.type !== 'hidden' && !attributes.value && typeof details[key] === 'string')
   return { ...node, attributes: { ...attributes, value: details[key] } };
  return node;
 }) } };
}
export function registrationError(data: unknown, status?: number): { expired: boolean; message: string } {
 const error = data && typeof data === 'object' && 'error' in data ? data.error : undefined;
 const detail = error && typeof error === 'object' ? error as {id?: unknown;message?: unknown;reason?: unknown} : undefined;
 const expired = status === 410 && detail?.id === 'self_service_flow_expired';
 const message = typeof detail?.message === 'string' && detail.message ? detail.message : typeof detail?.reason === 'string' && detail.reason ? detail.reason : '';
 return { expired, message: expired ? 'This signup form expired. Get a fresh form, then re-enter your password.' : message };
}
// The retained auth layout intentionally remounts on a new flow ID. Transfer only
// nonsecret fields once to that exact replacement, never to another route/flow.
let handoffTimer: ReturnType<typeof setTimeout> | undefined;
let handoff: { flowId: string; href: string; until: number; details: RegistrationDetails } | undefined;
export function rememberRegistrationDetails(flowId: string, href: string, details: RegistrationDetails) {
 if (handoffTimer !== undefined) clearTimeout(handoffTimer);
 handoff = { flowId, href, until: Date.now() + 30_000, details: { email: details.email, username: details.username } };
 handoffTimer = setTimeout(() => { handoff = undefined; handoffTimer = undefined; }, 30_000);
}
export function takeRegistrationDetails(flowId: string, href: string): RegistrationDetails | undefined {
 const saved = handoff; handoff = undefined;
 if (handoffTimer !== undefined) clearTimeout(handoffTimer);
 handoffTimer = undefined;
 if (saved?.flowId === flowId && saved.href === href && saved.until >= Date.now()) return saved.details;
}
