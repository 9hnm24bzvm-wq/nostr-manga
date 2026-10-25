// Closed outcome vocabulary for the compact fixture preflight. No runtime
// response body, status number, URL or exception text enters this projection.
export const compactFeedHttpOutcomes = Object.freeze([
  'success200', 'rate-limited429', 'unavailable503', 'other4xx',
  'other5xx', 'otherstatus', 'transportfailure', 'invalidJSON'
]);

// The backend emits these only for explicitly enabled, non-production fixtures.
// A cancellation is observed as such; it does not assert a timeout root cause.
export const compactFeedDatabaseFailures = Object.freeze([
  'none', 'unavailable', 'pool-wait-timeout', 'pool-create-timeout',
  'pool-recycle-timeout', 'pool-closed', 'pool-backend', 'pool-other',
  'query-canceled', 'query-lock', 'query-connection', 'query-database', 'query-client'
]);

export function compactFeedDatabaseFailure(status, header) {
  if (status === 200 && header === undefined) return 'none';
  if (status === 503 && typeof header === 'string'
    && compactFeedDatabaseFailures.slice(2).includes(header)) return header;
  return 'unavailable';
}

export function compactFeedHttpStatus(status) {
  if (status === 200) return 'success200';
  if (status === 429) return 'rate-limited429';
  if (status === 503) return 'unavailable503';
  if (Number.isInteger(status) && status >= 400 && status <= 499) return 'other4xx';
  if (Number.isInteger(status) && status >= 500 && status <= 599) return 'other5xx';
  return 'otherstatus';
}

export async function observeCompactFeedResponse(fetchResponse, observeOutcome) {
  let response;
  try { response = await fetchResponse(); }
  catch (error) { observeOutcome('transportfailure', 'unavailable'); throw error; }
  const status = response.status();
  const headers = response.headers();
  const failure = compactFeedDatabaseFailure(status, headers['x-weebdex-fixture-feed-failure']);
  let body;
  try { body = /json/.test(headers['content-type'] ?? '') ? await response.json() : undefined; }
  catch (error) { observeOutcome('invalidJSON', failure); throw error; }
  observeOutcome(compactFeedHttpStatus(status), failure);
  return { status, body };
}
