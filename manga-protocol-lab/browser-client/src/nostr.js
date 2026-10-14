import { schnorr } from "@noble/curves/secp256k1.js";
import { hash, unhex, integer, b64, unb64 } from "./codec.js";
const text = new TextEncoder(),
  decoder = new TextDecoder("utf-8", { fatal: true });
/** @typedef {import("./types.js").SignedEvent} SignedEvent */
/** @typedef {import("./types.js").EventTemplate} EventTemplate */
/** @typedef {import("./types.js").Signer} Signer */
/** @typedef {import("./types.js").Relay} Relay */
/** @typedef {import("./types.js").JsonObject} JsonObject */

/** @param {unknown} value @returns {value is JsonObject} */
function isObject(value) {
  return (
    value !== null && (typeof value === "object" || typeof value === "function")
  );
}
/** @param {unknown} value @returns {value is string[][]} */
function isTags(value) {
  if (!Array.isArray(value)) return false;
  // JSON serialization reads indexed values, not a signer-supplied iterator.
  for (let index = 0; index < value.length; index++) {
    /** @type {unknown} */
    const tag = value[index];
    if (!Array.isArray(tag)) return false;
    for (let entry = 0; entry < tag.length; entry++)
      if (typeof tag[entry] !== "string") return false;
  }
  return true;
}

export const now = () => Math.floor(Date.now() / 1000);
/** @param {unknown} e @returns {e is SignedEvent} */
export function verifyEvent(e) {
  try {
    if (!isObject(e)) return false;
    unhex(e.pubkey);
    unhex(e.id);
    unhex(e.sig, 64);
    integer(e.created_at);
    integer(e.kind, 65535);
    if (
      typeof e.content !== "string" ||
      !isTags(e.tags) ||
      text.encode(JSON.stringify(e)).length > 16384
    )
      return false;
    const id = hash(
      text.encode(
        JSON.stringify([0, e.pubkey, e.created_at, e.kind, e.tags, e.content]),
      ),
    );
    return (
      id === e.id &&
      schnorr.verify(unhex(e.sig, 64), unhex(id), unhex(e.pubkey))
    );
  } catch {
    return false;
  }
}
// NIP07 cannot cancel an extension prompt; abandon overdue results locally.
/**
 * @template T
 * @param {() => T | PromiseLike<T>} operation
 * @returns {Promise<T>}
 */
export async function signerRequest(operation) {
  const deadline = performance.now() + 60000;
  const timeout = () =>
    Error(
      "Signer request timed out. Retry explicitly; late approval will not be used.",
    );
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let timer;
  /** @type {Promise<never>} */
  const overdue = new Promise((_, reject) => {
    timer = setTimeout(() => reject(timeout()), 60000);
  });
  try {
    const result = await Promise.race([
      Promise.resolve().then(operation),
      overdue,
    ]);
    if (performance.now() >= deadline) throw timeout();
    return result;
  } finally {
    clearTimeout(timer);
  }
}
/**
 * @param {Signer} signer
 * @param {string} owner
 * @param {EventTemplate} event
 * @returns {Promise<SignedEvent>}
 */
export async function signExact(signer, owner, event) {
  unhex(owner);
  if (event.pubkey !== undefined && event.pubkey !== owner)
    throw Error("Requested event identity differs from selected owner");
  const template = structuredClone({ ...event, pubkey: owner });
  const e = await signerRequest(() =>
    signer.signEvent(structuredClone(template)),
  );
  /** @type {Array<"pubkey" | "created_at" | "kind" | "content">} */
  const fields = ["pubkey", "created_at", "kind", "content"];
  if (
    !verifyEvent(e) ||
    fields.some((k) => e[k] !== template[k]) ||
    JSON.stringify(e.tags) !== JSON.stringify(template.tags)
  )
    throw Error(
      "Signer changed the requested event or returned an invalid signature",
    );
  return e;
}
/** @param {SignedEvent} e @param {string} key @returns {string} */
export function one(e, key) {
  const tags = e.tags.filter((t) => t[0] === key);
  if (tags.length !== 1 || tags[0].length !== 2)
    throw Error("Invalid proof tag: " + key);
  return tags[0][1];
}
/** @param {string} value @returns {string} */
export function origin(value) {
  const u = new URL(value);
  const local = ["localhost", "127.0.0.1", "[::1]"].includes(u.hostname);
  if (
    u.username ||
    u.password ||
    u.search ||
    u.hash ||
    u.pathname !== "/" ||
    (u.protocol !== "https:" && !(local && u.protocol === "http:"))
  )
    throw Error("Use an HTTPS origin, or loopback HTTP for development");
  return u.origin;
}
/**
 * @param {Response} response
 * @param {number} [limit]
 * @returns {Promise<Uint8Array<ArrayBuffer>>}
 */
export async function bytesResponse(response, limit = 65536) {
  const reader = response.body?.getReader();
  if (!reader) return new Uint8Array();
  /** @type {Uint8Array[]} */
  const chunks = [];
  let size = 0;
  try {
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      size += value.length;
      if (size > limit) throw Error("Relay response exceeds budget");
      chunks.push(value);
    }
  } catch (e) {
    await reader.cancel();
    throw e;
  }
  const out = new Uint8Array(size);
  let pos = 0;
  for (const b of chunks) {
    out.set(b, pos);
    pos += b.length;
  }
  return out;
}
/**
 * @param {string} endpoint
 * @param {string} [expectedKey]
 * @returns {Promise<Relay>}
 */
export async function proveService(endpoint, expectedKey) {
  endpoint = origin(endpoint);
  if (expectedKey) unhex(expectedKey);
  const nonce = hash(text.encode(crypto.randomUUID())),
    url = `${endpoint}/identity?nonce=${encodeURIComponent(nonce)}`;
  const r = await fetch(url, {
    credentials: "omit",
    redirect: "error",
    cache: "no-store",
    signal: AbortSignal.timeout(15000),
  });
  if (!r.ok) throw Error("Service identity request failed");
  /** @type {unknown} */
  const e = JSON.parse(decoder.decode(await bytesResponse(r)));
  if (
    !verifyEvent(e) ||
    e.kind !== 20005 ||
    Math.abs(e.created_at - now()) > 60 ||
    one(e, "nonce") !== nonce ||
    one(e, "endpoint") !== endpoint ||
    (expectedKey && e.pubkey !== expectedKey)
  )
    throw Error("Service identity mismatch");
  return { origin: endpoint, serviceKey: e.pubkey };
}
/**
 * @param {Relay} relay
 * @param {Signer} signer
 * @param {string} owner
 * @param {string} path
 * @param {string} [method]
 * @param {unknown} [value]
 * @param {number} [limit]
 * @param {() => void | Promise<void>} [beforeSend]
 * @returns {Promise<{status: number, data: unknown}>}
 */
export async function signedRequest(
  relay,
  signer,
  owner,
  path,
  method = "POST",
  value = {},
  limit = 65536,
  beforeSend = async () => {},
) {
  const url = origin(relay.origin) + path;
  const bytes =
    method === "GET" ? new Uint8Array() : text.encode(JSON.stringify(value));
  const tags = [
    ["u", url],
    ["method", method],
    ["nonce", crypto.randomUUID()],
  ];
  if (method !== "GET") tags.push(["payload", hash(bytes)]);
  const e = await signExact(signer, owner, {
    kind: 27235,
    created_at: now(),
    content: "",
    tags,
  });
  /** @type {Record<string, string>} */
  const headers = {
    authorization: "Nostr " + b64(text.encode(JSON.stringify(e))),
  };
  if (method !== "GET") headers["content-type"] = "application/json";
  await beforeSend();
  const r = await fetch(url, {
    method,
    headers,
    body: method === "GET" ? undefined : bytes,
    credentials: "omit",
    redirect: "error",
    cache: "no-store",
    signal: AbortSignal.timeout(60000),
  });
  const body = await bytesResponse(r, limit);
  const proof = r.headers.get("x-manga-response-proof");
  if (!proof?.startsWith("Nostr ") || proof.length > 16384)
    throw Error("Missing signed relay response (check CORS exposure)");
  /** @type {unknown} */
  const p = JSON.parse(decoder.decode(unb64(proof.slice(6))));
  if (
    !verifyEvent(p) ||
    p.pubkey !== relay.serviceKey ||
    p.kind !== 20006 ||
    Math.abs(p.created_at - now()) > 60
  )
    throw Error("Invalid relay response proof");
  for (const [k, v] of Object.entries({
    protocol: "manga-federation-response-v1",
    u: url,
    method,
    request: e.id,
    status: String(r.status),
    payload: hash(body),
  }))
    if (one(p, k) !== v) throw Error("Relay response scope mismatch");
  /** @type {unknown} */
  const data = JSON.parse(decoder.decode(body));
  return { status: r.status, data };
}
/**
 * @param {Relay} relay
 * @param {Signer} signer
 * @param {string} owner
 * @param {string} hashValue
 * @param {number} max
 * @returns {Promise<Uint8Array<ArrayBuffer>>}
 */
export async function publicObject(relay, signer, owner, hashValue, max) {
  unhex(hashValue);
  const url = origin(relay.origin) + "/library/blobs/" + hashValue;
  const e = await signExact(signer, owner, {
    kind: 27235,
    created_at: now(),
    content: "",
    tags: [
      ["u", url],
      ["method", "GET"],
      ["nonce", crypto.randomUUID()],
    ],
  });
  const r = await fetch(url, {
    headers: { authorization: "Nostr " + b64(text.encode(JSON.stringify(e))) },
    credentials: "omit",
    redirect: "error",
    cache: "no-store",
    signal: AbortSignal.timeout(60000),
  });
  const bytes = await bytesResponse(r, max);
  if (r.status !== 200 || hash(bytes) !== hashValue)
    throw Error("Library object unavailable or corrupt");
  return bytes;
}
