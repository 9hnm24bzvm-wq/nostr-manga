// One original image, serialized once. The caller owns body and must pass these
// exact, unmodified bytes to both its request proof and its transport. Rebuilding
// a FormData payload afterward would sign a different multipart boundary.
export const MAX_UPLOAD_IMAGE_BYTES = 25 * 1024 * 1024;
export type UploadImageMime = 'image/png' | 'image/jpeg' | 'image/gif' | 'image/webp';
export interface UploadImageFile {
  readonly name: string;
  readonly type: string;
  readonly size: number;
  arrayBuffer(): Promise<ArrayBuffer>;
}
export interface SerializedUploadMultipart {
  readonly body: Uint8Array;
  // The exact file slice already copied into body, for matching its signed receipt.
  readonly fileBytes: Uint8Array;
  readonly contentType: string;
  readonly boundary: string;
  readonly file: Readonly<{ name: string; type: UploadImageMime; size: number }>;
}
const encoder = new TextEncoder();
const mimes = new Set<string>(['image/png', 'image/jpeg', 'image/gif', 'image/webp']);
const startsWith = (bytes: Uint8Array, signature: readonly number[], offset = 0) =>
  bytes.length >= offset + signature.length && signature.every((byte, index) => bytes[offset + index] === byte);

function matchesImageSignature(bytes: Uint8Array, type: UploadImageMime): boolean {
  // This is a format guard, not an image decoder. Admission and the signed
  // preview loader still have to decode/validate the actual image separately.
  if (type === 'image/png') return startsWith(bytes, [137, 80, 78, 71, 13, 10, 26, 10]);
  if (type === 'image/jpeg') return startsWith(bytes, [255, 216, 255]);
  if (type === 'image/gif') return startsWith(bytes, [71, 73, 70, 56, 55, 97]) || startsWith(bytes, [71, 73, 70, 56, 57, 97]);
  return startsWith(bytes, [82, 73, 70, 70]) && startsWith(bytes, [87, 69, 66, 80], 8);
}

function randomBoundary(): string {
  const bytes = new Uint8Array(24);
  globalThis.crypto.getRandomValues(bytes);
  return 'weebdex-upload-' + [...bytes].map(byte => byte.toString(16).padStart(2, '0')).join('');
}

export async function serializeUploadMultipart(
  file: UploadImageFile,
  options: { boundary?: string } = {}
): Promise<SerializedUploadMultipart> {
  return serializeImage(file, options);
}

// Replacement signs one canonical version field followed by exactly one file.
export async function serializeUploadReplacementMultipart(
  file: UploadImageFile,
  expectedVersion: number,
  options: { boundary?: string } = {}
): Promise<SerializedUploadMultipart> {
  if (!Number.isSafeInteger(expectedVersion) || expectedVersion < 1 || expectedVersion > 2147483647)
    throw Error('A canonical positive upload session version is required');
  return serializeImage(file, options, expectedVersion);
}

async function serializeImage(
  file: UploadImageFile,
  options: { boundary?: string },
  expectedVersion?: number
): Promise<SerializedUploadMultipart> {
  if (!file || typeof file.arrayBuffer !== 'function') throw Error('An original image file is required');
  const { name, type, size } = file;
  if (typeof name !== 'string' || !name.trim() || name.length > 255 || encoder.encode(name).byteLength > 255 || /[\u0000-\u001f\u007f"\\/]/.test(name))
    throw Error('The original image filename is empty, too long, or unsafe for a multipart header');
  if (typeof type !== 'string' || !mimes.has(type)) throw Error('A canonical PNG, JPEG, GIF or WebP image MIME type is required');
  if (!Number.isSafeInteger(size) || size < 1 || size > MAX_UPLOAD_IMAGE_BYTES) throw Error('The original image must contain 1 to 25 MiB of bytes');
  const boundary = options.boundary === undefined ? randomBoundary() : options.boundary;
  if (typeof boundary !== 'string' || !/^[A-Za-z0-9_-]{16,70}$/.test(boundary)) throw Error('The multipart boundary must contain 16 to 70 canonical characters');
  const buffer = await file.arrayBuffer();
  if (!(buffer instanceof ArrayBuffer) || buffer.byteLength !== size) throw Error('The original image byte length does not match its file metadata');
  const bytes = new Uint8Array(buffer);
  if (!matchesImageSignature(bytes, type as UploadImageMime)) throw Error('The original image signature does not match its MIME type');
  const delimiter = encoder.encode('--' + boundary);
  for (let index = 0; index <= bytes.length - delimiter.length; index++) {
    if (bytes[index] === delimiter[0] && delimiter.every((byte, offset) => bytes[index + offset] === byte))
      throw Error('The multipart boundary collides with the original image bytes');
  }
  const version = expectedVersion === undefined ? '' : `--${boundary}\r\nContent-Disposition: form-data; name="expectedVersion"\r\n\r\n${expectedVersion}\r\n`;
  const head = encoder.encode(`${version}--${boundary}\r\nContent-Disposition: form-data; name="file"; filename="${name}"\r\nContent-Type: ${type}\r\n\r\n`);
  const tail = encoder.encode(`\r\n--${boundary}--\r\n`);
  const body = new Uint8Array(head.byteLength + bytes.byteLength + tail.byteLength);
  body.set(head);
  body.set(bytes, head.byteLength);
  body.set(tail, head.byteLength + bytes.byteLength);
  return Object.freeze({ body, fileBytes: body.subarray(head.byteLength, head.byteLength + bytes.byteLength), contentType: `multipart/form-data; boundary=${boundary}`, boundary, file: Object.freeze({ name, type: type as UploadImageMime, size }) });
}
