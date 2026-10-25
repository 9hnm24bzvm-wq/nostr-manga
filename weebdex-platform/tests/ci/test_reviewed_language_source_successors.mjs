// Finite source associations only. No Git history, product execution, browser,
// hosted run, genuine signatures or publication acceptance is claimed here.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { restoreReviewedLanguageSource } from './reviewed-language-source-successors.mjs';
import { restoreLocalPageSource } from './reviewed-local-page-source-successors.mjs';
// Preserve the historical helper, hashes and every negative control. Only the
// comparison input is reconstructed; no runtime evaluates these returned bytes.

const digest = value => createHash('sha256').update(value).digest('hex');
const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const adapter = read('./test_public_upload_adapter_recovery.mjs');
const start = adapter.indexOf('const context ='), end = adapter.indexOf('async function ready(');
assert.ok(start > 0 && end > start);
const sources = [
  {
    kind: 'uploadClient',
    comparison: restoreLocalPageSource(read('../../platform-integrations/public-api/upload-client.ts'), 'uploadClient'),
    comparisonSha: '04d9c29cd44b554fe474ced74854cbd165a828ee0355a262780063f3d1bccf16',
    historicalSha: 'c8d3e55319a2f93ae1f9b4e11875c2d4711a1d8d985e4dc7df5ab722f26dca8f',
    partialReversal: "import { publicWriteLanguage } from './language';\n",
  },
  {
    kind: 'chapterEditor',
    comparison: restoreLocalPageSource(read('../../platform-integrations/public-api/PublicChapterEditor.svelte'), 'chapterEditor'),
    comparisonSha: 'e4f1c783774682001b814d1a6ccf59041e193ab15590c9f3c4d51459703e58fd',
    historicalSha: 'b368c2419b3eb1babe92bc3c13d274265249cf7c2200f63fb43415467689d328',
    partialReversal: "  import { chapterMetadataPatch } from './language';\n",
  },
  {
    kind: 'adapterFixturePrefix',
    comparison: restoreLocalPageSource(adapter.slice(start, end), 'adapterFixturePrefix'),
    comparisonSha: '918945cd002ee79351f9d7e0e1a7750c7db7cf3d216215fa651be77f2f5f0567',
    historicalSha: 'ab243b74e652d5766e41f3e10af337ac16bbbba9df04c4af5246dfa6a05f48a0',
    // This gives the exact pre-language intermediate, not the older immutable
    // fixture pin. Neither intermediate nor historical bytes can pose as current.
    partialReversal: "'./language', ",
  },
];

for (const row of sources) {
  test(row.kind + ': exact historical language-stage comparison reconstructs its immutable earlier pin', () => {
    assert.equal(digest(row.comparison), row.comparisonSha);
    const historical = restoreReviewedLanguageSource(row.comparison, row.kind);
    assert.equal(digest(historical), row.historicalSha);
    assert.notEqual(historical, row.comparison);
    assert.equal(digest(row.comparison), row.comparisonSha);
  });
  test(row.kind + ': arbitrary comparison-stage modification is refused', () => {
    for (const changed of [row.comparison + '\n// unreviewed source\n', row.comparison.slice(0, -1)]) {
      assert.throws(() => restoreReviewedLanguageSource(changed, row.kind), /Unknown reviewed language successor/);
    }
  });
  test(row.kind + ': partial reversal is refused as historical-stage input', () => {
    assert.equal(row.comparison.split(row.partialReversal).length - 1, 1);
    const partial = row.comparison.replace(row.partialReversal, '');
    assert.notEqual(digest(partial), row.comparisonSha);
    assert.notEqual(digest(partial), row.historicalSha);
    assert.throws(() => restoreReviewedLanguageSource(partial, row.kind), /Unknown reviewed language successor/);
  });
  test(row.kind + ': duplicate reversal hunk is refused', () => {
    assert.equal(row.comparison.split(row.partialReversal).length - 1, 1);
    const duplicate = row.comparison + row.partialReversal;
    assert.equal(duplicate.split(row.partialReversal).length - 1, 2);
    assert.throws(() => restoreReviewedLanguageSource(duplicate, row.kind), /Unknown reviewed language successor/);
  });
  test(row.kind + ': earlier historical bytes are refused as language-stage input', () => {
    const historical = restoreReviewedLanguageSource(row.comparison, row.kind);
    assert.equal(digest(historical), row.historicalSha);
    assert.throws(() => restoreReviewedLanguageSource(historical, row.kind), /Unknown reviewed language successor/);
  });
}

test('unknown source kinds and non-string inputs are refused', () => {
  for (const kind of ['unknown', '__proto__', 'constructor']) {
    assert.throws(() => restoreReviewedLanguageSource(sources[0].comparison, kind), /Unknown reviewed language source kind/);
  }
  for (const row of sources) {
    for (const value of [undefined, null, 1, Buffer.from(row.comparison)]) {
      assert.throws(() => restoreReviewedLanguageSource(value, row.kind));
    }
  }
});
