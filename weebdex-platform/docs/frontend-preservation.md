# Frontend preservation and integration

The `frontend/` directory is the provided WeebDex attachment, copied without
changing any of its 676 regular files. Its source markup, CSS, Svelte components,
static assets, configuration, scripts, and Yarn lockfile are preserved literally.

Attachment SHA-256:
`845aa9bb7964cc02cc3353144c97ae42e64e09c5128e98076fa090f833f6cadd`.

`test-evidence/frontend/source-parity.json` fingerprints every copied attachment
file. From the project root, run `python3 scripts/verify-frontend-parity.py` to
verify preservation after a build. Generated dependency/build directories are
additional files and are not attachment source.

## Build commands

Use Node 24 and Yarn Classic 1.22.22, a fresh cache, frozen lockfile, and disabled
dependency lifecycle scripts. The expected restore command is:

```
cd frontend
COREPACK_ENABLE_AUTO_PIN=0 corepack yarn@1.22.22 install --frozen-lockfile --ignore-scripts --cache-folder ../.cache/yarn
```

The archive's `prepare` command is skipped during restore. SvelteKit sync is a
reviewed explicit command; run `node node_modules/@sveltejs/kit/svelte-kit.js sync`
before checks if needed. From the project root, `node scripts/build-frontend.mjs`
executes the reviewed Vite build and original post-build script.

The completed isolated build in this workspace used:

```
WEEBDEX_FRONTEND_ROOT="$PWD/.build/frontend-restore3" NODE_OPTIONS=--max-old-space-size=2048 node scripts/build-frontend.mjs
```

Do not reuse the earlier unknown install outputs. A fresh checkout may restore
from the unchanged lockfile into its own dependency/cache directories. The
verified recovery here used an external offline mirror and official exact-lock
archives; those mutable caches are not required for serving `artifacts/frontend`.

The original post-build script writes four Go templates into a directory it
does not create and then removes `build/index.html`. The external wrapper creates
that output directory and restores the exact Vite-generated fallback after the
unchanged original script runs. The four original Go templates remain available.
Rust SPA serving uses the restored fallback; dynamic template rendering requires
separate implementation and verification.

## Configuration-only seam

The frontend imports `$env/static/public`, so these values are compiled at build
time. The endpoint settings alone need no original-file edits. Necessary workflow repairs and local-first/protocol UI extensions now execute through separately manifested anchored transforms; supplied original files remain unchanged. The wrapper uses same-origin defaults and accepts explicit environment overrides:

| Variable | Default Rust-facing endpoint | Requirement |
| --- | --- | --- |
| `PUBLIC_SITE_NAME` | `WeebDex` | Preserve product label |
| `PUBLIC_API_URL` | `/api` | Exact attachment API contracts |
| `PUBLIC_KRATOS_PUBLIC_API` | `/auth` | Ory/Kratos-compatible browser-flow façade |
| `PUBLIC_COVER_URL` | `/covers` | Original cover and thumbnail path conventions |
| `PUBLIC_DATA_URL` | `/data` | Original chapter image data paths |
| `PUBLIC_UPLOAD_URL` | `/uploads` | Upload preview file paths |
| `PUBLIC_PROXY_URL` | `/proxy` | Image URL query parameter; server-side fetch safety |
| `PUBLIC_OPENGRAPH_URL` | `/og-image` | Entity social metadata image routes |

The wrapper also publishes a portable generated copy at `artifacts/frontend/`. Configure
`PLATFORM_UI_DIR` to this absolute directory; the Rust server uses its `index.html` as the
SPA fallback for frontend routes. Mount API and authentication before fallback.
Avoid redirecting missing assets or API requests to the SPA fallback.

## Essential backend compatibility

- JSON feeds use `{total, limit, page, data}`. Mapped chapter/history feeds add
  `map.manga`. Types in `frontend/src/@types/` are the attachment's DTO reference.
- The unchanged `HTTP` helper uses XHR; protected operations set credentials.
  JSON responses need an `application/json` content type and truthful status.
- `src/lib/Kratos.ts` uses `@ory/client` with credentials. Login, registration,
  recovery, verification, settings, logout, and flow-error browser APIs need the
  exact SDK paths, flow IDs, expiry, CSRF, session cookies, UI nodes, validation
  errors, and redirect semantics. `/api/auth/check` alone is insufficient.
- Frontend auth does not create protocol publishing authority. The Rust platform
  session/role identity must bind to protocol keys through an explicit adapter.
- The attachment disables SSR and prerendering. Server route fallback is needed
  for direct links and browser navigation to each retained client route.

## Verification limits

Literal source parity establishes unchanged HTML/CSS/Svelte/static inputs.
It does not establish rendered pixel parity, functioning APIs, authentication,
reader images, or feature completion. Browser parity needs the original build
and platform build rendered with equivalent data and environment settings.

There is no license or upstream repository identity in the attachment. Keep its
provenance recorded and do not infer permission to publish it publicly.

## Verified attachment defects

The homepage imports `./announcements/announcement.svelte` and
`./announcements/AnnouncementEntry.svelte`, neither of which exists in the
attachment. The sidebar also links to `/announcements`, which has no retained
route implementation. The exact original announcement data, markup, styles,
and route cannot be recovered from the supplied archive. Missing source is a
feature and build blocker; substitutes must be separately identified and cannot
establish exact parity.

Linux also exposes two retained case-sensitive import errors: the stores barrel
imports `./session`, while the file is `Session.ts`; generated UI components
import `$lib/utils.js`, while the file is `Utils.ts`. External resolver aliases
can correct these specific paths without changing their source bytes. They do
not resolve the absent announcement feature.

`scripts/vite.platform.config.mjs` merges the unchanged attachment configuration
with two exact-target import resolver aliases for these defects. It does not
apply generic case-insensitive resolution or synthesize missing feature files.

The separately labelled reconstruction under
`platform-integrations/frontend-reconstruction/announcements/` supplies the
missing homepage imports and public listing/detail plus staff management routes.
It reads and mutates the persisted Rust announcement domain, creates no invented
rows, and reuses retained components/styles. See `announcements-reconstruction.md`.
Original absent announcement markup and rendered parity remain unknown.

Reviewed script-only repairs are applied before Svelte compilation from
`platform-integrations/frontend-overlays.mjs`. They do not write any supplied
source. `test-evidence/frontend/integration-source-manifest.json` records exact
original/transformed hashes, labels, and additive route hashes. The preparation
step verifies strict retained Svelte HTML/CSS equality after stripping module
script bodies and normalizing exactly one parent-approved GroupClaimDialog
onclick JavaScript body back to its original text. That event handler is the only
approved exception; no generic attribute or markup normalization is allowed.
Original source bytes, rendered DOM structure, classes and styles are preserved.
Executed JavaScript intentionally differs, including that handler and the new
reconstructed announcement routes.

For isolated dependency recovery, `WEEBDEX_FRONTEND_ROOT` can select a fresh
staging copy made from the same immutable attachment. This avoids sharing output
or dependencies with a prior install whose terminal state was lost. The wrapper
still loads that copy's unchanged Vite configuration and keeps integrations
outside its source.

The static import audit also found 36 absent legacy Geist font variants. All nine
referenced modern `.woff2` weights exist. Those missing fallback variants are
recorded by the attachment inspector; no replacement asset bytes are invented.

## Current execution evidence

Production bundling passed with command exit0 and a terminal ownership receipt
at `test-evidence/frontend/final-build18/build-result.json`. No owned descendants
remained and all1,632 protected compiler inputs were unchanged. The generated
portable UI has401 files (7,008,454 bytes); all111 supplied static files
are byte-identical in output. `artifact-manifest.json` fingerprints every output.

All676 supplied source files remain byte-identical. The isolated restore verified
842 exact original-lockfile official tarballs and completed a frozen offline Yarn
install with lifecycle scripts disabled. Historical lost-session/deadline/import
receipts are retained rather than represented as current failures or cleanup proof.
Current frozen composition has534 anchored definitions/543 occurrences across
106 originals, plus separately inventoried external modules and three reconstructed
announcement routes. Exact approved handler/guard/key/control/panel exceptions are
listed in the source manifest. Unrelated markup and styles retain strict byte
comparisons. Executed JavaScript and globally rendered HTML are not claimed identical.

The finite receipts are `round14-owner-source-repairs/summary.json` and
`round15-owner-source-repairs/summary.json` and `round16-owner-source-repair/summary.json` and `round17-owner-source-repairs/summary.json` under `test-evidence/frontend`.
Composed source compilation passed93 Svelte components and56 scripts, with8
recorded initial-reference warnings. This is syntax/source compilation, not a
completed semantic typecheck or runtime proof. No real browser journey has run;
the new independent review gate still holds application tests. The public CSS/DOM
and private actor/library/target workflow acceptance cases remain to be executed.

Earlier lost install sessions still have no terminal receipt and remain unknown.
They were not resumed, signalled, or reused as unpacked dependencies. Verified
completed tarball salvage and official registry endpoint mappings are documented
in `test-evidence/frontend` receipts. The old uncertainty is not erased by the
successful new independent restore.

## Portable static artifact

`artifacts/frontend` is a clean single build generation, with no dependency-cache
or `.svelte-kit` dependency. Previous generated artifacts are preserved outside
the served directory. `python3 scripts/package-frontend.py` packages the complete
verified UI, manifest, bootstrap hash evidence and integration documentation into
`artifacts/frontend-static.tar.gz`. Each archived regular member is byte-compared
and every UI input is rehashed after packaging. This archive contains no runtime
database, private key, session or deployment credential. Serve the unpacked
`frontend/` directory through the configured Rust platform, rather than opening
the application through a file URL. API/auth/media services are still required.


Build19/source534 is the current successfully bundled generation. The finite relation-eligibility helper repair changes executed script behavior only; all676 supplied files and all111 supplied static files remain byte-identical. Fresh independent review and real browser acceptance remain required; no runtime or globally identical rendered HTML claim is made.
