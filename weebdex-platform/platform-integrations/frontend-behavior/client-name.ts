import { HTTPError } from '$lib/HTTP';
// Rust str::trim uses Unicode White_Space, unlike ECMAScript String.trim.
export const normalizedClientName = (name: string) => name.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, '');
export function validClientName(value: string) {
  const name = normalizedClientName(value), length = [...name].length;
  return length >= 1 && length <= 64 && new TextEncoder().encode(name).length <= 256 && !/[\u0000-\u001f\u007f-\u009f\ud800-\udfff]/u.test(name);
}
export function requireClientName(value: string) {
  if (!validClientName(value)) throw new HTTPError(400, 'Client name must contain 1 to 64 printable Unicode characters after trimming whitespace.', 'Invalid client name');
  return normalizedClientName(value);
}
