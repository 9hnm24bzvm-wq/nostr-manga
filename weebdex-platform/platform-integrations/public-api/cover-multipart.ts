import { publicWriteLanguage } from './language';
// Serialize exact original bytes once. No canvas, resizing or compression.
import {
  serializeUploadReplacementMultipart,
  type UploadImageFile,
} from './upload-multipart';
import { publicUuid } from './validation';
export type CoverMetadata = {
  locale: string;
  volume: string;
  description: string;
  main: boolean;
};
// Match Rust str::trim in catalog validation: Unicode White_Space includes
// NEL (U+0085), but not BOM (U+FEFF). JavaScript trim differs on both.
function nativeMetadataText(value: string): string {
  return value.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, '');
}
export function coverMetadata(input: unknown): CoverMetadata {
  if (!input || typeof input !== 'object' || Array.isArray(input))
    throw Error('Cover metadata is required');
  const row = input as Record<string, unknown>;
  if (
    Object.keys(row).some(
      (key) => !['locale', 'volume', 'description', 'main'].includes(key),
    ) ||
    typeof row.locale !== 'string' ||
    !publicWriteLanguage(row.locale) ||
    typeof row.volume !== 'string' ||
    row.volume.length > 32 ||
    typeof row.description !== 'string' ||
    new TextEncoder().encode(row.description).length > 20000 ||
    typeof row.main !== 'boolean' ||
    /\0/.test(row.volume + row.description)
  )
    throw Error('Invalid bounded cover metadata');
  return {
    locale: row.locale,
    volume: nativeMetadataText(row.volume),
    description: nativeMetadataText(row.description),
    main: row.main,
  };
}
export async function serializeCoverMultipart(
  file: UploadImageFile,
  version: number,
  input: CoverMetadata,
  expectedAccountId: string,
  options: { boundary?: string } = {},
) {
  const metadata = coverMetadata(input),
    account = publicUuid(expectedAccountId);
  if (
    !['image/png', 'image/jpeg', 'image/webp'].includes(file?.type) ||
    !{
      'image/png': /\.png$/i,
      'image/jpeg': /\.jpe?g$/i,
      'image/webp': /\.webp$/i,
    }[file.type]?.test(file.name)
  )
    throw Error(
      'Choose an original PNG, JPEG or WebP cover with its matching filename',
    );
  const serialized = await serializeUploadReplacementMultipart(
      file,
      version,
      options,
    ),
    encoder = new TextEncoder();
  const closing = encoder.encode(`\r\n--${serialized.boundary}--\r\n`),
    head = serialized.body.subarray(0, serialized.body.length - closing.length);
  const fields = Object.entries(metadata)
    .map(([key, value]) => {
      const text = String(value);
      if (text.includes('--' + serialized.boundary))
        throw Error('Multipart boundary collides with cover metadata');
      return `\r\n--${serialized.boundary}\r\nContent-Disposition: form-data; name="${key}"\r\n\r\n${text}`;
    })
    .join('');
  const prefix = encoder.encode(
    `--${serialized.boundary}\r\nContent-Disposition: form-data; name="expectedVersion"\r\n\r\n${version}\r\n`,
  );
  if (prefix.some((byte, index) => serialized.body[index] !== byte))
    throw Error('Cover multipart version prelude changed');
  const accountField = encoder.encode(
    `--${serialized.boundary}\r\nContent-Disposition: form-data; name="expectedAccountId"\r\n\r\n${account}\r\n`,
  );
  const tail = encoder.encode(fields + `\r\n--${serialized.boundary}--\r\n`),
    body = new Uint8Array(head.length + accountField.length + tail.length);
  body.set(prefix);
  body.set(accountField, prefix.length);
  body.set(head.subarray(prefix.length), prefix.length + accountField.length);
  body.set(tail, head.length + accountField.length);
  if (body.length > 32 * 1024 * 1024)
    throw Error('Cover multipart exceeds signed request budget');
  return Object.freeze({ ...serialized, body, metadata });
}

// PUT is a patch, not a re-upload. Validate only the fields emitted by this
// edit; do not require an imported native-only locale to change incidentally.
export function coverMetadataPatch(input: unknown, previous: {locale:string|null;volume:string;description:string;main:boolean}): Partial<CoverMetadata> {
 if(!input||typeof input!=='object'||Array.isArray(input))throw Error('Cover metadata is required');
 const row=input as Record<string,unknown>,out:Partial<CoverMetadata>={};
 if(Object.keys(row).some(key=>!['locale','volume','description','main'].includes(key)))throw Error('Unsupported cover metadata');
 for(const key of ['locale','volume','description','main'] as const){
  if(!Object.hasOwn(row,key))continue;
  // The form renders nullable locale as an empty input without inventing a code.
  if(row[key]===(previous[key]??''))continue;
  if(key==='locale')out.locale=publicWriteLanguage(row.locale);
  else if(key==='main'){if(typeof row.main!=='boolean')throw Error('Invalid cover main flag');out.main=row.main;}
  else {const value=row[key];if(typeof value!=='string'||/\0/.test(value)||(key==='volume'?value.length>32:new TextEncoder().encode(value).byteLength>20000))throw Error('Invalid bounded cover metadata');const normalized=nativeMetadataText(value);if(normalized!==previous[key])out[key]=normalized;}
 }
 return out;
}
