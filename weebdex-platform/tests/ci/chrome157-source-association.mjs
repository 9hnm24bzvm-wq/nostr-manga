// Exact one-line runtime identity association; no acceptance rule is removed.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
const sha = value => createHash('sha256').update(value).digest('hex');
export function restoreChrome157Evidence(value) {
  if (sha(value) !== '9ad4c9e50f680ce8cf3953b7e768a40895513cf068c82e7a468d0fe56258be34') return value;
  const before = "        insist(hashes <= set(value) and value['source_files_checked'] == 676 and value['frontend_preserved'] is True and value.get('fresh_build') is True and value.get('historical_binaries_reused') is False and ((value.get('playwright'), value.get('chromium_revision')) == ('1.62.1', '1234') or (value.get('playwright'), value.get('chromium_revision'), value['chromium_binary_sha256']) == ('1.64.1', 'cft-157.0.8103.6', 'ec4e78d65a5382c2b50aaf28c6a655a624973b3d3c711a2a4ae34369fd9a3444')) and value['stage'] == 'complete')", after = "        insist(hashes <= set(value) and value['source_files_checked'] == 676 and value['frontend_preserved'] is True and value.get('fresh_build') is True and value.get('historical_binaries_reused') is False and value.get('playwright') == '1.62.1' and value.get('chromium_revision') == '1234' and value['stage'] == 'complete')";
  assert.equal(value.split(before).length - 1, 1, 'Unknown qualified evidence source');
  const restored = value.replace(before, after);
  assert.equal(sha(restored), 'e2d01a31f15cc578e5bb6e3b8a4dbb2273f6448f12764bb6fd675c2cf3ea2628', 'Unknown qualified evidence predecessor');
  return restored;
}
