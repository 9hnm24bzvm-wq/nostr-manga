import {restoreReviewedScheduledSource} from './reviewed-scheduled-source-successors.mjs';
// Exact source-only associations to reviewed product freeze 312dee7.
// Client/editor predecessors are 0f39069; the adapter fixture additionally keeps
// its e33974e pin through the separately guarded prior chapter integrations
// 903f02f and 4ed6a74. That older fixture layer is not language-only.
// Preserve historical hashes while consumers execute untouched current bytes.
// Literal reversals require no Git history at test runtime and do not establish
// browser, hosted-run, signature or publication acceptance.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';

const digest = value => createHash('sha256').update(value).digest('hex');
const reversals = {
  "uploadClient": {
    "successor_sha256": "2ef51d1d4590a0154e79ba38a7af55566ca42f8c3e6a007064abc58b8c6a88aa",
    "predecessor_sha256": "c8d3e55319a2f93ae1f9b4e11875c2d4711a1d8d985e4dc7df5ab722f26dca8f",
    "changes": [
      [
        "import { publicWriteLanguage } from './language';\n",
        ""
      ],
      [
        "          for (const key of ['title', 'volume', 'chapter', 'language']) if (input[key] !== undefined) { if (typeof input[key] !== 'string') fail('Invalid chapter metadata', 422); output[key === 'language' ? 'translatedLanguage' : key] = key === 'language' ? publicWriteLanguage(input[key]) : input[key]; }\n",
        "          for (const key of ['title', 'volume', 'chapter', 'language']) if (input[key] !== undefined) { if (typeof input[key] !== 'string') fail('Invalid chapter metadata', 422); output[key === 'language' ? 'translatedLanguage' : key] = input[key]; }\n"
      ],
      [
        "      if (route === '/upload/check-approval-required' && method === 'POST') { const value = record(payload); const locale = publicWriteLanguage(value.language ?? value.locale); const manga = owned.latest?.manga_id ?? id; const response = await raw(owned, route, method, { manga, locale }); if (response.status === 200 && (record(response.body).result !== 'ok' || typeof record(response.body).requiresApproval !== 'boolean')) fail('Invalid actual approval decision'); return callerView<T>(response); }\n",
        "      if (route === '/upload/check-approval-required' && method === 'POST') { const value = record(payload); const locale = text(value.language ?? value.locale, 'upload language'); const manga = owned.latest?.manga_id ?? id; const response = await raw(owned, route, method, { manga, locale }); if (response.status === 200 && (record(response.body).result !== 'ok' || typeof record(response.body).requiresApproval !== 'boolean')) fail('Invalid actual approval decision'); return callerView<T>(response); }\n"
      ],
      [
        "        const body = { chapterDraft: { volume: candidate.draft.volume || null, chapter: candidate.draft.chapter || null, title: candidate.draft.title || null, translatedLanguage: publicWriteLanguage(candidate.draft.language) }, pageOrder: candidate.page_order, termsAccepted: true };\n",
        "        const body = { chapterDraft: { volume: candidate.draft.volume || null, chapter: candidate.draft.chapter || null, title: candidate.draft.title || null, translatedLanguage: candidate.draft.language }, pageOrder: candidate.page_order, termsAccepted: true };\n"
      ],
      [
        "      if (method === 'PATCH' && action === undefined) { const value = record(payload); const current = await exact(owned, sessionId); if (!current.can_mutate || !account.facts.canManage) fail('Upload is read-only', 409); const body = { version: integer(value.version, 1), draft: draft(value.draft), pageOrder: ids(value.page_order ?? value.pageOrder) }; if(body.draft.language)publicWriteLanguage(body.draft.language); const response = await raw(owned, `/nostr/uploads/${sessionId}`, 'PATCH', body); return callerView<T>(response.status === 200 ? { ...response, body: (owned.latest = normalizeSession(response.body, sessionId)) } : response); }\n",
        "      if (method === 'PATCH' && action === undefined) { const value = record(payload); const current = await exact(owned, sessionId); if (!current.can_mutate || !account.facts.canManage) fail('Upload is read-only', 409); const body = { version: integer(value.version, 1), draft: draft(value.draft), pageOrder: ids(value.page_order ?? value.pageOrder) }; const response = await raw(owned, `/nostr/uploads/${sessionId}`, 'PATCH', body); return callerView<T>(response.status === 200 ? { ...response, body: (owned.latest = normalizeSession(response.body, sessionId)) } : response); }\n"
      ]
    ]
  },
  "chapterEditor": {
    "successor_sha256": "e4f1c783774682001b814d1a6ccf59041e193ab15590c9f3c4d51459703e58fd",
    "predecessor_sha256": "b368c2419b3eb1babe92bc3c13d274265249cf7c2200f63fb43415467689d328",
    "changes": [
      [
        "  import { chapterMetadataPatch } from './language';\n",
        ""
      ],
      [
        "      const payload = action === 'update' ? chapterMetadataPatch(chapter, { title, volume, chapter: number, language, groups: groups.split(',').map(value => value.trim()).filter(Boolean) }) : action === 'remove' ? { removalMode } : undefined;\n",
        "      const payload = action === 'update' ? { version: chapter.version, title, volume, chapter: number, language, groups: groups.split(',').map(value => value.trim()).filter(Boolean) } : action === 'remove' ? { removalMode } : undefined;\n"
      ],
      [
        "      <label class=\"block\">Language<input class=\"block w-full rounded border p-2\" bind:value={language} /></label>\n      <p class=\"text-sm\">New language values use lowercase two-letter codes such as ta or pt-br. An unchanged imported label is kept exactly and is not resubmitted.</p>\n",
        "      <label class=\"block\">Language<input class=\"block w-full rounded border p-2\" bind:value={language} maxlength=\"12\" /></label>\n"
      ]
    ]
  },
  "adapterFixturePrefix": {
    "successor_sha256": "eaaf343f1c09c19a609f172b3bb70273040a13e0a67e1fddba23373e86d83a2a",
    "predecessor_sha256": "ab243b74e652d5766e41f3e10af337ac16bbbba9df04c4af5246dfa6a05f48a0",
    "historical_source_commit": "e33974ef063a44ac455b0c55c11bfed8841c2ba4",
    "stages": [
      {
        "scope": "reviewed-public-language-allowlist",
        "reviewed_source_commits": [
          "ddb70e39c52c6f4077bd29110bc6e74437f11c21"
        ],
        "successor_sha256": "eaaf343f1c09c19a609f172b3bb70273040a13e0a67e1fddba23373e86d83a2a",
        "predecessor_sha256": "0fe7990cddf4109b6e9073a46d1e97c0e5d92487c1c74848e7258da1e22582dc",
        "changes": [
          [
            "    if (!['./language', './validation', './upload-multipart', './chapter-access', './chapter-command-fence'].includes(specifier)) throw Error('Unexpected adapter dependency ' + specifier);\n",
            "    if (!['./validation', './upload-multipart', './chapter-access', './chapter-command-fence'].includes(specifier)) throw Error('Unexpected adapter dependency ' + specifier);\n"
          ]
        ]
      },
      {
        "scope": "prior-reviewed-chapter-integration-not-language-only",
        "reviewed_source_commits": [
          "903f02fc1350284046e8b7a84bc54ab430993ba4",
          "4ed6a7458f60def1b1a0a8ef83ccaa98257cc31d"
        ],
        "successor_sha256": "0fe7990cddf4109b6e9073a46d1e97c0e5d92487c1c74848e7258da1e22582dc",
        "predecessor_sha256": "ab243b74e652d5766e41f3e10af337ac16bbbba9df04c4af5246dfa6a05f48a0",
        "changes": [
          [
            "class RequestFailure extends Error {constructor(message,outcome,status){super(message);this.outcome=outcome;this.status=status;this.bodyAvailable=false;}}\n",
            ""
          ],
          [
            "    if (specifier === './nostr-upload-request') return synthetic(specifier, { createNostrUploadRequest: forbidden, PublicUploadRequestError: RequestFailure });\n",
            "    if (specifier === './nostr-upload-request') return synthetic(specifier, { createNostrUploadRequest: forbidden });\n"
          ],
          [
            "    if (!['./validation', './upload-multipart', './chapter-access', './chapter-command-fence'].includes(specifier)) throw Error('Unexpected adapter dependency ' + specifier);\n",
            "    if (!['./validation', './upload-multipart'].includes(specifier)) throw Error('Unexpected adapter dependency ' + specifier);\n"
          ],
          [
            "    if(route==='/chapter/'+CHAPTER&&request.method==='GET')return response({result:'ok',response:'entity',data:chapterResource(2)});\n",
            ""
          ],
          [
            "    createRequest: captured => async request => { state.preparing = request; try { captured.assertCurrent(); await captured.beforeSend?.(new AbortController().signal); captured.assertCurrent(); } catch (error) { throw Object.assign(error, { outcome: 'unsent' }); } finally { state.preparing = undefined; } requests.push(request); const result = await (request.method==='GET'&&new URL(request.url).pathname==='/api/v1/chapter/'+CHAPTER ? defaultRequest(request) : options.request ? options.request(request, defaultRequest, state) : defaultRequest(request)); captured.assertCurrent(); options.afterTransport?.(request, state); return result; },\n",
            "    createRequest: captured => async request => { state.preparing = request; try { captured.assertCurrent(); await captured.beforeSend?.(new AbortController().signal); captured.assertCurrent(); } catch (error) { throw Object.assign(error, { outcome: 'unsent' }); } finally { state.preparing = undefined; } requests.push(request); const result = await (options.request ? options.request(request, defaultRequest, state) : defaultRequest(request)); captured.assertCurrent(); options.afterTransport?.(request, state); return result; },\n"
          ]
        ]
      }
    ]
  }
};

export function restoreReviewedLanguageSource(input, kind) {
  assert(Object.hasOwn(reversals, kind), 'Unknown reviewed language source kind');
  if (kind === 'uploadClient' || kind === 'adapterFixturePrefix') input = restoreReviewedScheduledSource(input, kind);
  const row = reversals[kind];
  const message = 'Unknown reviewed language successor: ' + kind;
  assert.equal(typeof input, 'string', message);
  // Historical bytes are comparison output, never accepted as current input.
  assert.equal(digest(input), row.successor_sha256, message);
  for (const stage of row.stages ?? [row]) {
    assert.equal(digest(input), stage.successor_sha256, message);
    for (const [successor, predecessor] of stage.changes) {
      assert.equal(input.split(successor).length - 1, 1, message);
      input = input.replace(successor, predecessor);
    }
    assert.equal(digest(input), stage.predecessor_sha256, message);
  }
  assert.equal(digest(input), row.predecessor_sha256, message);
  return input;
}
