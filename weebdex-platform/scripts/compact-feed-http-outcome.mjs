// Closed outcome vocabulary for the compact fixture preflight. No runtime
// response body, status number, URL or exception text enters this projection.
export const compactFeedHttpOutcomes = Object.freeze([
  'success200', 'rate-limited429', 'unavailable503', 'other4xx',
  'other5xx', 'otherstatus', 'transportfailure', 'invalidJSON'
]);

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
  catch (error) { observeOutcome('transportfailure'); throw error; }
  const status = response.status();
  let body;
  try { body = /json/.test(response.headers()['content-type'] ?? '') ? await response.json() : undefined; }
  catch (error) { observeOutcome('invalidJSON'); throw error; }
  observeOutcome(compactFeedHttpStatus(status));
  return { status, body };
}
