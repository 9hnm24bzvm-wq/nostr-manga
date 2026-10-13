import { sha256 } from "../vendor/hashes/sha2.js";
export const families = ["chapters", "series", "lists", "members"];
export const hex = (b) =>
  Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
export function unhex(s, n = 32) {
  if (typeof s !== "string" || !new RegExp(`^[0-9a-f]{${n * 2}}$`).test(s))
    throw Error("Invalid identifier");
  return Uint8Array.from(s.match(/../g), (x) => parseInt(x, 16));
}
export const hash = (b) => hex(sha256(b));
const utf8 = new TextEncoder(),
  text = new TextDecoder("utf-8", { fatal: true });
export function integer(x, max = Number.MAX_SAFE_INTEGER) {
  if (!Number.isSafeInteger(x) || x < 0 || x > max)
    throw Error("Integer outside client bounds");
  return x;
}
export function compare(a, b) {
  for (let i = 0; i < Math.min(a.length, b.length); i++)
    if (a[i] !== b[i]) return a[i] - b[i];
  return a.length - b.length;
}
export function encode(v) {
  const out = [];
  function head(m, n) {
    integer(n);
    if (n < 24) out.push(m | n);
    else if (n <= 255) out.push(m | 24, n);
    else if (n <= 65535) out.push(m | 25, n >> 8, n & 255);
    else if (n <= 0xffffffff)
      out.push(
        m | 26,
        (n >>> 24) & 255,
        (n >>> 16) & 255,
        (n >>> 8) & 255,
        n & 255,
      );
    else {
      out.push(m | 27);
      let z = BigInt(n);
      for (let i = 7; i >= 0; i--)
        out.push(Number((z >> BigInt(8 * i)) & 255n));
    }
  }
  function add(b) {
    for (const x of b) out.push(x);
  }
  function write(x, depth = 0) {
    if (depth > 20) throw Error("CBOR depth");
    if (x === null) out.push(246);
    else if (x === true || x === false) out.push(x ? 245 : 244);
    else if (typeof x === "number") head(0, x);
    else if (x instanceof Uint8Array) {
      head(64, x.length);
      add(x);
    } else if (typeof x === "string") {
      const b = utf8.encode(x);
      head(96, b.length);
      add(b);
    } else if (Array.isArray(x)) {
      head(128, x.length);
      for (const y of x) write(y, depth + 1);
    } else if (x && typeof x === "object") {
      const pairs = Object.entries(x)
        .map(([k, v]) => [encode(k), v])
        .sort((a, b) => compare(a[0], b[0]));
      head(160, pairs.length);
      for (const [k, v] of pairs) {
        add(k);
        write(v, depth + 1);
      }
    } else throw Error("Unsupported CBOR value");
  }
  write(v);
  return Uint8Array.from(out);
}
export function decode(bytes, max = 1000000) {
  if (!(bytes instanceof Uint8Array) || bytes.length > max)
    throw Error("CBOR byte limit");
  let p = 0,
    nodes = 0;
  function byte() {
    if (p >= bytes.length) throw Error("Truncated CBOR");
    return bytes[p++];
  }
  function read(depth = 0) {
    if (depth > 20 || ++nodes > 50000) throw Error("CBOR complexity");
    const tag = byte(),
      major = tag >> 5,
      ai = tag & 31;
    if (major === 7) {
      if (tag === 244) return false;
      if (tag === 245) return true;
      if (tag === 246) return null;
      throw Error("Unsupported CBOR simple value");
    }
    let n = ai;
    if (ai >= 24) {
      const count = { 24: 1, 25: 2, 26: 4, 27: 8 }[ai];
      if (!count) throw Error("Indefinite CBOR");
      let z = 0n;
      for (let i = 0; i < count; i++) z = z * 256n + BigInt(byte());
      if (z > BigInt(Number.MAX_SAFE_INTEGER))
        throw Error("Integer outside client bounds");
      n = Number(z);
    }
    if (major === 0) return n;
    if (major === 2 || major === 3) {
      if (n > bytes.length - p) throw Error("Truncated CBOR bytes");
      const b = bytes.slice(p, p + n);
      p += n;
      return major === 2 ? b : text.decode(b);
    }
    if (major === 4) {
      if (n > 50000 - nodes) throw Error("CBOR array budget");
      return Array.from({ length: n }, () => read(depth + 1));
    }
    if (major === 5) {
      if (n > 25000) throw Error("CBOR map budget");
      const o = Object.create(null);
      for (let i = 0; i < n; i++) {
        const k = read(depth + 1);
        if (typeof k !== "string" || Object.hasOwn(o, k))
          throw Error("Invalid map key");
        o[k] = read(depth + 1);
      }
      return o;
    }
    throw Error("Unsupported CBOR type");
  }
  const v = read();
  if (p !== bytes.length || compare(encode(v), bytes) !== 0)
    throw Error("Noncanonical CBOR");
  return v;
}
export function normalize(f, r) {
  if (!families.includes(f) || !r || typeof r !== "object")
    throw Error("Invalid library record");
  const updated_at = integer(r.updated_at ?? Math.floor(Date.now() / 1000));
  let v;
  if (f === "chapters") {
    unhex(r.id);
    if (typeof r.completed !== "boolean")
      throw Error("Completion must be boolean");
    v = {
      id: r.id,
      completed: r.completed,
      page: r.page == null ? null : integer(r.page, 0xffffffff),
      updated_at,
    };
  }
  if (f === "series") {
    unhex(r.id);
    if (
      ![
        "reading",
        "on_hold",
        "plan_to_read",
        "dropped",
        "re_reading",
        "completed",
      ].includes(r.status)
    )
      throw Error("Invalid reading status");
    v = { id: r.id, status: r.status, updated_at };
  }
  if (f === "lists") {
    unhex(r.id, 16);
    if (
      typeof r.name !== "string" ||
      !r.name.length ||
      [...r.name].length > 128 ||
      utf8.encode(r.name).length > 512 ||
      text.decode(utf8.encode(r.name)) !== r.name ||
      /[\p{Cc}]/u.test(r.name)
    )
      throw Error("Invalid list name");
    v = { id: r.id, name: r.name, updated_at };
  }
  if (f === "members") {
    unhex(r.list, 16);
    unhex(r.series);
    v = {
      list: r.list,
      series: r.series,
      position: integer(r.position),
      updated_at,
    };
  }
  return v;
}
export const recordKey = (f, r) => (f === "members" ? r.list + r.series : r.id);
export function tuple(f, r) {
  r = normalize(f, r);
  if (f === "chapters") return [unhex(r.id), r.completed, r.page, r.updated_at];
  if (f === "series") return [unhex(r.id), r.status, r.updated_at];
  if (f === "lists") return [unhex(r.id, 16), r.name, r.updated_at];
  return [unhex(r.list, 16), unhex(r.series), r.position, r.updated_at];
}
function fromTuple(f, t) {
  if (
    !Array.isArray(t) ||
    t.length !== { chapters: 4, series: 3, lists: 3, members: 4 }[f]
  )
    throw Error("Invalid tuple");
  if (
    !(t[0] instanceof Uint8Array) ||
    t[0].length !== (["lists", "members"].includes(f) ? 16 : 32) ||
    (f === "members" && (!(t[1] instanceof Uint8Array) || t[1].length !== 32))
  )
    throw Error("Invalid tuple identifier bytes");
  integer(t.at(-1));
  const id = hex(t[0]);
  return normalize(
    f,
    f === "chapters"
      ? { id, completed: t[1], page: t[2], updated_at: t[3] }
      : f === "series"
        ? { id, status: t[1], updated_at: t[2] }
        : f === "lists"
          ? { id, name: t[1], updated_at: t[2] }
          : { list: id, series: hex(t[1]), position: t[2], updated_at: t[3] },
  );
}
export function bucket(key, n) {
  const b = sha256(unhex(key, key.length / 2));
  return (
    ((b[0] * 0x1000000 + (b[1] << 16) + (b[2] << 8) + b[3]) >>> 0) & (n - 1)
  );
}
function fields(o, names) {
  if (
    !o ||
    Array.isArray(o) ||
    Object.keys(o).sort().join("|") !== [...names].sort().join("|")
  )
    throw Error("Object fields mismatch");
}
export function buildGraph(records) {
  const chunks = [],
    roots = [];
  let total = 0,
    totalBytes = 0,
    totalBuckets = 0;
  for (const family of families) {
    const rows = (records[family] ?? [])
      .map((r) => normalize(family, r))
      .sort((a, b) =>
        recordKey(family, a) < recordKey(family, b)
          ? -1
          : recordKey(family, a) > recordKey(family, b)
            ? 1
            : 0,
      );
    total += rows.length;
    if (total > 2000000) throw Error("Library record budget");
    for (let i = 1; i < rows.length; i++)
      if (recordKey(family, rows[i - 1]) === recordKey(family, rows[i]))
        throw Error("Duplicate record");
    const hashes = rows.map((r) => {
      const b = sha256(
        unhex(
          recordKey(family, r),
          family === "members" ? 48 : family === "lists" ? 16 : 32,
        ),
      );
      return (b[0] * 0x1000000 + (b[1] << 16) + (b[2] << 8) + b[3]) >>> 0;
    });
    let n = 1,
      parts,
      encodedParts;
    for (;;) {
      parts = Array.from({ length: n }, () => []);
      rows.forEach((r, i) => parts[hashes[i] & (n - 1)].push(r));
      if (parts.every((x) => x.length <= 4096)) {
        encodedParts = parts.map((entries) =>
          entries.length
            ? encode({
                type: "library-chunk",
                version: 1,
                family,
                entries: entries.map((record) => tuple(family, record)),
              })
            : null,
        );
        if (
          encodedParts.every((data) => data === null || data.length <= 1000000)
        )
          break;
      }
      n *= 2;
      if (n > 2048) throw Error("Family bucket limit");
    }
    const descriptors = [];
    for (const [index, entries] of parts.entries()) {
      if (!entries.length) {
        descriptors.push(null);
        continue;
      }
      const data = encodedParts[index];
      const h = hash(data);
      totalBytes += data.length;
      chunks.push({ type: "chunk", hash: h, data });
      descriptors.push([unhex(h), data.length, entries.length]);
    }
    totalBuckets += n;
    roots.push({ family, chunks: descriptors });
  }
  if (totalBuckets > 4096 || totalBytes > 256 * 1024 * 1024)
    throw Error("Library object budget");
  const data = encode({ type: "library-root", version: 1, families: roots });
  if (data.length > 262144) throw Error("Root byte budget");
  const root = { type: "root", hash: hash(data), data };
  return { root, objects: [...chunks, root], records: total };
}
export function rootDescriptors(bytes) {
  const r = decode(bytes, 262144);
  fields(r, ["type", "version", "families"]);
  if (
    r.type !== "library-root" ||
    r.version !== 1 ||
    !Array.isArray(r.families) ||
    r.families.length !== 4
  )
    throw Error("Invalid library root");
  let buckets = 0,
    count = 0,
    size = 0;
  const refs = [];
  r.families.forEach((f, i) => {
    fields(f, ["family", "chunks"]);
    if (f.family !== families[i] || !Array.isArray(f.chunks))
      throw Error("Family order");
    const n = f.chunks.length;
    if (!n || n > 2048 || n & (n - 1)) throw Error("Bucket count");
    buckets += n;
    f.chunks.forEach((d, index) => {
      if (d === null) return;
      if (
        !Array.isArray(d) ||
        d.length !== 3 ||
        !(d[0] instanceof Uint8Array) ||
        d[0].length !== 32
      )
        throw Error("Invalid descriptor");
      integer(d[1], 1000000);
      integer(d[2], 4096);
      if (!d[1] || !d[2]) throw Error("Empty descriptor");
      count += d[2];
      size += d[1];
      refs.push({
        hash: hex(d[0]),
        size: d[1],
        records: d[2],
        family: f.family,
        buckets: n,
        index,
      });
    });
  });
  if (buckets > 4096 || count > 2000000 || size > 256 * 1024 * 1024)
    throw Error("Root aggregate limit");
  return refs;
}
export function readGraph(root, objects) {
  const records = Object.fromEntries(families.map((f) => [f, []]));
  for (const d of rootDescriptors(root)) {
    const b = objects.get(d.hash);
    if (!b || b.length !== d.size || hash(b) !== d.hash)
      throw Error("Missing/corrupt library object");
    const c = decode(b);
    fields(c, ["type", "version", "family", "entries"]);
    if (
      c.type !== "library-chunk" ||
      c.version !== 1 ||
      c.family !== d.family ||
      !Array.isArray(c.entries) ||
      c.entries.length !== d.records
    )
      throw Error("Chunk contract mismatch");
    let prev = "";
    for (const t of c.entries) {
      const r = fromTuple(d.family, t),
        key = recordKey(d.family, r);
      if (key <= prev || bucket(key, d.buckets) !== d.index)
        throw Error("Record ordering/bucket mismatch");
      prev = key;
      records[d.family].push(r);
    }
  }
  return records;
}
export function b64(b) {
  let s = "";
  for (let i = 0; i < b.length; i += 8192)
    s += String.fromCharCode(...b.subarray(i, i + 8192));
  return btoa(s);
}
export function unb64(s) {
  if (typeof s !== "string" || s.length > 1400000) throw Error("Base64 limit");
  return Uint8Array.from(atob(s), (c) => c.charCodeAt(0));
}
