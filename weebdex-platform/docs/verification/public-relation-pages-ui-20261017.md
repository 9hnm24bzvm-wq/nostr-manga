# Public Related tab: bounded snapshot pages

## Source change

The existing Related tab now opts in to the public instance's bounded relation-page extension. Recommendations retain their existing upstream request/response contract. No UI panel or discovery fallback is added.

The adapter requests 32 complete edges, appends them in server order and follows the actual `nextOffset`, including short nonterminal prefixes. Every page must retain the typed requested/canonical manga proof, owner revision, visible total and snapshot token. Continuations carry the captured owner revision and token. Edge and target UUIDs must be unique, with one non-self manga target and the real owner revision per edge. Public `persistedTotal` is rejected.

Only after all relation pages validate does the adapter hydrate exact 32-ID target batches for existing covers/creators. Hydrated revisions must equal the target revisions captured in the snapshot. A changed, missing, duplicate or substituted target fails the whole read. No partial list, automatic restart or retry masks a 409.

One abortable 60-second cancellation deadline covers pages and hydration. Relation pages have a 1 MiB normalized JSON check; all pages and hydration bodies share a 16 MiB normalized JSON budget. Oversized work fails explicitly. These client checks do not measure raw HTTP transfer bytes. The server independently enforces its complete-resource page wire budget.

## Qualified here

- 127 finite/source cases passed: 74 existing recovery cases, 46 focused paging cases, 4 composition/preparation cases and 3 standalone probe self-tests over local HTTP stand-ins.
- The focused suite executes the actual catalogue adapter, public client, validator and normalizers. Only transport and clock are stand-ins.
- The 4096-edge case traverses 128 pages and 128 hydration batches, preserving every edge's order and identity. It is a finite contract case, not a live database/load result.
- Failures cover continuity/proof/visibility-version changes, duplicate/self/substituted targets, shifted/truncated pages, terminal HTTP errors, abort/deadline cleanup and byte budgets.
- The retained layout's actual leading-question-mark prototype override is executed in a regression.
- Direct strict TypeScript checking of this module's import closure completed with zero diagnostics using an existing warm dependency installation.
- Both full composed application strict gates passed on the exact frozen source: gate 0 had zero errors / 15 warnings; gate 1 had zero errors / 12 warnings. Each preserved all 679 canonical frontend files. Existing warm dependencies were reused through symlinks without installation or cache copies.
- Canonical frontend source is untouched; `git diff --check` is clean.

## Remaining gates

The backend paging/source repair is a separate candidate and must be composed before live use. Live HTTP/DB 4096-cap qualification, later fully integrated runtime gates and native-browser acceptance are not established by this patch. The feature's existing device/publication readiness fences are unchanged.

Reproducible commands, raw finite output, direct type-check script/output and source hashes are in `test-evidence/frontend/public-relation-pages-20261017/`.

## Live-fixture bridge

`tests/ci/probe_public_relation_pages.mjs` accepts an HTTP loopback `PUBLIC_API_ORIGIN`, canonical `MANGA_ID` and optional `EXPECTED_RELATIONS` (default 4096), and runs the unchanged production adapter/client/fetch transport with the same 60-second budget. `RELATION_PROBE_RECEIPT` optionally saves its JSON receipt. It uses only Node built-ins and reads the exact local TypeScript closure. The receipt contains source/script/request/response hashes, page/hydration counts, raw HTTP response bytes and timings. Its three local HTTP self-tests qualify the probe mechanism, not the real database capacity. The separate backend owner will run it synchronously while the owned database fixture server exists.

## First actual database adapter run

The backend owner invoked this exact frozen probe synchronously inside its owned live HTTP/PostgreSQL fixture. The adapter returned all 4096 relations after 128 pages and 128 hydration requests in 48,970.248 ms, with 7,841,019 raw response bytes. The maximum relation page was 32,784 bytes and maximum hydration response 28,561 bytes. All four loaded production module hashes and the probe hash match this branch. The original budgets were unchanged. The retained receipt is `actual-db-adapter-first-pass.json` (SHA256 `d44cdbf152c5fe2f7fc4ea76acad1ab2b80b34c825d4e55435dc76b0cf178ff8`).

This establishes the actual adapter/API/DB relation-read boundary on that synthetic fixture. The surrounding fixture then failed owner Manga GET at the 4096 cap with HTTP503. The backend owner is repairing that separately measured root/alias budget issue. The full title-detail journey and final backend candidate are therefore still pending; no frontend truncation or budget increase was introduced. The owner reported the fixture guard closed and no surviving processes.

## Corrected whole-fixture qualification

The separately measured owner-detail blocker is now repaired and qualified by backend commit `3e2240b08236ac5a1104562fe4f0762a41da6310`. Its bounded Manga alias allowance is exactly 4097 (one owner plus 4096 relation targets); other kinds retain 4096. The corrected fixture passed owner Manga GET at the relation cap with HTTP200 and rejected the one-over budget with503. Legal 32KiB metadata prefixes and version/visibility continuity cases also passed.

The unchanged frontend adapter again returned all4096 relations through128 pages plus128 hydrations in49,371.695ms, transferring7,841,019 raw response bytes within the unchanged budgets. Its four production source hashes still match this branch. The final raw receipt is retained by the backend commit at `test-evidence/public-relation-pages-20261017/actual-frontend-adapter.json`, SHA256 `480faa5baca461933552deeaa934dd25f25e8aca52a67c306a6f2547c95ea71c`; a compact association is retained here as `actual-db-adapter-corrected-summary.json`. The earlier failure/first successful boundary evidence remains historical, not the final outcome. This is a synthetic live HTTP/DB adapter qualification, not rendered-browser or general production-capacity acceptance.
