# Signed draft preview navigation repair

The enabled public UUID overlay now makes the retained
`/title/[id]/[slug]/+page.ts` re-export `load` from `../+page`. Both route shapes
therefore use the same existing signed draft reader, not copied authorization
logic. The original loader, title client, request/account/service checks, response
normalization, and retained tab markup are unchanged. The source hash and
recovery-anchor generator include the slug route. With the public gate off,
both original native loaders retain their cookie/grant checks.

## Executed finite checks

- 17 new pinned Svelte 5.55.1 / JSDOM 27.0.1 cases execute the actual composed
  loaders, both route page components, and the retained Manga component.
- Direct slug preview accepts actual draft/submitted/rejected revisions. The
  retained Chapters/Covers links preserve the slug and draft query.
- Covers click, Back, Forward, and route reload re-read current state/revision.
  History entries are moved with JSDOM history APIs. A finite router invokes the
  actual loaders and applies current results; this is not SvelteKit runtime QA.
- The old slug-loader negative control successfully enters the signed direct
  preview, then fails on its actual Covers link with native HTTP403 admission.
- Gate-off native admission/grants, non-draft public reads, private refusal,
  disconnected/mismatched signer/service, delayed route/signer rejection,
  newer navigation, and post-entry signer loss remain fail-closed in these cases.
- 169 cases pass across the new navigation suite plus report/draft controller,
  report/draft DOM, title controller, and title-edit route DOM suites.
- All 676 attachment files and the three already tracked announcement additions
  match their baseline bytes. Enabled integration preparation passes the strict
  source/markup preservation gate for all 153 transformed source paths.
- The recovery generator reproduces the complete current public anchors and
  source-hash manifest without writing source in its regression test.

The finite transport supplies synthetic account/service identity and resource
responses. Visual leaves, tab content components, library view identity, and
routing are declared boundaries. No real signing credential, live request,
database, service, Cargo task, or native browser was used. Native Chromium remains
unexecuted; no replacement browser route was attempted. Full production bundling
and hosted/native browser acceptance are not established by this patch.

## Reproduction

From repository root, point `WEEBDEX_FRONTEND_ROOT` at the verified pinned frontend
restore and `WEEBDEX_BROWSER_DOM_NODE_MODULES` at the pinned JSDOM modules, then run:

```sh
node --experimental-vm-modules --test \
  weebdex-platform/tests/ci/test_public_draft_navigation_dom.mjs \
  weebdex-platform/tests/ci/test_public_reports_and_drafts.mjs \
  weebdex-platform/tests/ci/test_public_report_draft_dom.mjs \
  weebdex-platform/tests/ci/test_public_title_edit_route_dom.mjs \
  weebdex-platform/tests/ci/test_public_title_commands.mjs
python3 weebdex-platform/scripts/verify-frontend-parity.py
```

## Prospective feature qualification delta

Candidate `f12ddfb` is not changed or included here. Its existing source pins do
not include the navigation anchors, hash manifest, loader, retained Manga tabs,
or this new regression. Integration should explicitly add those source pins,
record this finite result separately, and extend future real-browser draft
journeys through enabled tabs plus Back/Forward/reload. Do not relabel either
candidate as browser-executed or silently replace its qualification records.
