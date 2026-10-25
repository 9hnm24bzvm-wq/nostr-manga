// Test-harness envelope equality only. Nostr signature/id validation is a
// separate prerequisite; this projection never treats an equal digest as proof.
import { createHash } from 'node:crypto';
const fields = Object.freeze(['id', 'pubkey', 'created_at', 'kind', 'tags', 'content', 'sig']);
const denseArray = value => Array.isArray(value) && Reflect.ownKeys(value).length === value.length + 1 &&
  Array.from({ length: value.length }, (_, index) => Object.hasOwn(value, index) &&
    Object.hasOwn(Object.getOwnPropertyDescriptor(value, index), 'value')).every(Boolean);
export function signedEventEnvelope(event) {
  if (!event || typeof event !== 'object' || Array.isArray(event) ||
      Reflect.ownKeys(event).length !== fields.length || fields.some(key => !Object.hasOwn(event, key)) ||
      fields.some(key => !Object.hasOwn(Object.getOwnPropertyDescriptor(event, key), 'value')) ||
      typeof event.id !== 'string' || !/^[0-9a-f]{64}$/.test(event.id) ||
      typeof event.pubkey !== 'string' || !/^[0-9a-f]{64}$/.test(event.pubkey) ||
      typeof event.sig !== 'string' || !/^[0-9a-f]{128}$/.test(event.sig) ||
      !Number.isSafeInteger(event.created_at) || event.created_at < 0 || !Number.isSafeInteger(event.kind) || event.kind < 0 || event.kind > 65535 ||
      typeof event.content !== 'string' || !denseArray(event.tags) ||
      Array.prototype.some.call(event.tags, tag => !denseArray(tag) || Array.prototype.some.call(tag, value => typeof value !== 'string'))) {
    throw Error('Signed event envelope is not the closed seven-field contract');
  }
  // Preserve all fields, tag order, tag-entry order and content bytes. Object
  // member insertion order changes under Rust serde_json::to_value serialization.
  // Copy validated tag entries so inherited toJSON/iterator behavior cannot
  // change the equality projection of an otherwise closed event envelope.
  const tags = Array.from({ length: event.tags.length }, (_, index) =>
    Array.from({ length: event.tags[index].length }, (_, item) => event.tags[index][item]));
  return [event.id, event.pubkey, event.created_at, event.kind, tags, event.content, event.sig];
}
export const signedEventDigest = event => createHash('sha256').update(JSON.stringify(signedEventEnvelope(event))).digest('hex');
