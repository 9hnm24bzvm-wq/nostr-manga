# Local-library build integration

This is the portable build/provenance map for the external browser-local-first adapter. Detailed runtime/authority contracts and currently required acceptance cases are in [browser-local-library.md](browser-local-library.md).

## Sources and mounts

- `platform-integrations/local-library/device-store.ts`: scoped native ten-character extension records, atomic IndexedDB read-modify-write, revision-checked import and instance namespace/service identity verification
- `library.ts`, `catalog.ts`, `MangaFollow.ts`, `ChapterHistory.ts`: default device source, explicit password-account source, unchanged API-like DTO contracts, real server catalogue sorting/filtering, identity-bound account import and no mutation replay
- `LocalLibraryPanel.svelte`, `LibraryProtected.svelte`: explicit device controls, error/status reporting, backups and portable canonical restore, list/record management and explicit reviewed original publication/recovery
- `publication.ts`, `projection-worker.ts`: original NIP-07 signing, canonical graph preparation off the UI thread, source-scope mapping checks and original durable graph outbox
- `protocol-identity.ts`, `ProtocolIdentityPanel.svelte`: separately reviewed original public profile/follow/relay/social workflows, signature-verified visible counterparts and an exact signed-public-action durable retry/reconciliation outbox
- `overlays.mjs`: exact original-source hashes and anchors, narrowly approved availability/key exceptions, and replacement only of the two original private-library API modules

The frontend owner mounts the library panel in the existing app layout and the public-identity panel on title and both reader page entry routes. All supplied frontend files remain byte-identical. Runtime script composition, exact expression/key exceptions and additive mounts are verified and recorded by `scripts/prepare-frontend-integration.mjs`. A preservation/source compilation result is not browser acceptance.

## Original code and dependencies

The nine files under `local-library/protocol` are byte-identical original browser-client modules. `protocol-source-manifest.json` lists original and copied paths with SHA-256. No private signer, dummy signature, synthetic protocol ID or cookie-account signer replaces them.

The original dependency versions are `@noble/curves@2.0.1` and `@noble/hashes@2.0.1`. Build integration resolves the officially downloaded, lock-integrity-verified packages from `.build/local-library-noble-2.0.1/node_modules`, independently of any historical build tree. Package provenance is recorded in `test-evidence/frontend/noble-restore/package-provenance.json`. Restoring the build environment requires these exact packages and normal retained frontend dependencies; supplied package/yarn source files are unchanged.

Vite aliases `$localLibrary` to this external module root, deduplicates Svelte and resolves the exact Noble versions. The source-hash-preserving shared overlay runs first; the library overlay verifies original source files and every remaining exact anchor against the composed source. Worker compilation uses a real module worker URL and unchanged original CBOR/crypto dependencies.

## Operator prerequisites and intentional limits

- Local IndexedDB opens immediately without waiting for a network lookup. Current catalogue scope is checked asynchronously with an eight-second bound. Local records and backups remain available on a stalled/offline network. Current native catalogue hydration and public source mapping require successful scope verification.
- Native device backups bind API origin, catalogue namespace epoch and pinned service key. Foreign native ID import/rebinding is blocked; only canonical protocol backups are portable between instances.
- Existing populated origin-only scope requires explicit adoption with current native ID/title/missing-ID review, rendered in bounded pages. Namespace/service identity changes persist a cross-tab mutation fence in the same IndexedDB transaction as scope verification. The old records remain inspectable/exportable, but native mutations and current catalogue hydration are blocked until the original scope is verified or a migration is reviewed. No automatic local rekey or login-driven replacement is implemented.
- Browser publication requires supported Web Locks, an external NIP-07 signer, explicitly verified recipients allowed by exact CSP/relay policy, correct original CORS where cross-origin, and admitted public-key authority at each recipient. Password sessions confer none of that authority.
- Browser public templates and signature reads use the unchanged original 16 KiB signed-event budget. Oversized content/tags/relay URLs are rejected during review before signer access. Public lists have bounded counts, but their actual encoded template must also fit that byte budget.
- Original canonical imports use bounded streamed staged replacement and revision CAS. Native JSON backup preview has a 256 MiB safety bound. Both use the original two-million-record admission ceiling where applicable; no measured scale claim is implied.
- The native account source uses `X-WeebDex-Expected-Account` only as an optional narrowing constraint on an already authenticated principal. Import additionally checks authoritative cookie-bound account identity before/after each page and before local apply. No account data is imported on login.
- Local storage eviction, unsupported IndexedDB/version changes, quota failure, signer timeout, denied admission, exact origin/CORS/CSP rejection, projection pending and ambiguous request outcome are surfaced. No fallback creates server storage, publications or silent replay.

## Required evidence

Preservation/source compilation, exact dependency integrity and permitted bundler execution are recorded separately from actual runtime tests. The independent full-wiring source audit must clear before runtime/browser tests start. Release/checkpoint packaging excludes live IndexedDB/runtime databases, credentials, signing keys, personal data and synthetic runtime sessions; include source, documentation and verified build/test receipts instead.
