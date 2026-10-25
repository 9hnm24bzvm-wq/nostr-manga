// QA-only same-module read. No global hook, DOM carrier or request interception.
// Private event IDs are retained only for the closed public projection's join.
export const uploadRefusalReaderPath = '/_app/immutable/qa/public-upload-refusals.js';
export const uploadCorrelatedRefusalStages = Object.freeze([
  'checkpoint', 'fetch', 'response-header', 'response-media', 'declared-size',
  'body-read', 'body-budget', 'decode', 'other',
]);
const eventId = /^[0-9a-f]{64}$/;
const sameKeys = (value, keys) => value !== null && typeof value === 'object' && !Array.isArray(value) &&
  Object.keys(value).sort().join(',') === [...keys].sort().join(',');
export function validateUploadRefusalRead(raw) {
  try {
    if (!sameKeys(raw, ['schema','enabled','record_valid','truncated','records']) || raw.schema !== 1 ||
        typeof raw.enabled !== 'boolean' || typeof raw.record_valid !== 'boolean' || typeof raw.truncated !== 'boolean' ||
        !Array.isArray(raw.records) || raw.records.length > 16 || raw.truncated && raw.records.length !== 16 ||
        !raw.enabled && (raw.records.length || raw.truncated || !raw.record_valid)) return false;
    const ids = new Set();
    for (const row of raw.records) {
      if (!sameKeys(row, ['event_id','stage','transport_status']) || typeof row.event_id !== 'string' ||
          !eventId.test(row.event_id) || ids.has(row.event_id) || !uploadCorrelatedRefusalStages.includes(row.stage) ||
          row.transport_status !== null && (!Number.isInteger(row.transport_status) || row.transport_status < 100 || row.transport_status > 599)) return false;
      ids.add(row.event_id);
    }
    return true;
  } catch { return false; }
}

export async function captureUploadRequestRefusals(page, expectedIds = []) {
  const invalid = { capture_valid: false, snapshot: null };
  let timer;
  try {
    if (!Array.isArray(expectedIds) || expectedIds.length > 16 ||
        expectedIds.some(id => typeof id !== 'string' || !eventId.test(id)) || new Set(expectedIds).size !== expectedIds.length) return invalid;
    const raw = await Promise.race([
      page.evaluate(async ({ readerPath, expectedIds }) => {
        const reader = await import(readerPath);
        if (Object.keys(reader).length !== 1 || typeof reader.readUploadRequestRefusals !== 'function') return null;
        // A caught transport records before its final cleanup abort. An initiating
        // timer/external abort may precede the catch. Wait only for the expected
        // private event IDs, independently of UI flush.
        for (let attempt = 0; attempt <= 16; attempt++) {
          const value = reader.readUploadRequestRefusals();
          if (!value || !Array.isArray(value.records)) return value;
          if (!value.enabled || !value.record_valid || value.truncated ||
              expectedIds.every(id => value.records.some(row => row?.event_id === id)) || attempt === 16) return value;
          await new Promise(resolve => setTimeout(resolve, 25));
        }
      }, { readerPath: uploadRefusalReaderPath, expectedIds }),
      new Promise(resolve => { timer = setTimeout(() => resolve(null), 1000); }),
    ]);
    return validateUploadRefusalRead(raw) && raw.enabled === true
      ? { capture_valid: true, snapshot: raw } : invalid;
  } catch { return invalid; }
  finally { if (timer !== undefined) clearTimeout(timer); }
}

export async function uploadRefusalReaderReady(page) {
  const value = await captureUploadRequestRefusals(page);
  return value.capture_valid === true && value.snapshot.record_valid === true &&
    value.snapshot.truncated === false && value.snapshot.records.length === 0;
}

