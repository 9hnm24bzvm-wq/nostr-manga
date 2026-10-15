# Public API frontend reconstruction

This source was reconstructed on 2026-10-15 after execution storage was lost. These are new recovery commits, not byte-for-byte restorations of the lost commits.

The restored attachment remains the canonical immutable frontend. A fourth, finite public-API overlay follows the existing shared, reader, and local-library transforms in production Vite, source syntax compilation, semantic projection and the source manifest composer. The same `/api/v1` client and `$publicApi` alias are used by each. `PUBLIC_MANGADEX_CATALOGUE` accepts only `0` or `1` and defaults off. This is staging code: enabling it is not a claim that the complete site or every MangaDex endpoint works.

The public overlay pins 25 original attachment files to the recovered 676-file source-parity manifest. Three exact media URL markup expressions and four missing-comment availability expressions are separately enumerated. The reader availability anchor is checked against its immediately preceding composed source; the rest are original-source anchors. Normalization reverses these exact exceptions before comparing retained markup and styles. Query-bearing Svelte virtual IDs are excluded from original-source transforms.

Catalogue writes in the eight targeted API families reject explicitly when the gate is enabled. Other API families and local-library writes still require complete integration review. Durable device ID conversion remains independently hard-disabled. The large device-migration core was not recovered. Its detailed invariants are documented separately; no automatic migration may be inferred from the reconstructed transport.

The optional `WEEBDEX_QA_BUILD_RESOURCE_PROFILE=serial-terser` profile retains Terser minification and sets only its worker count to one and compressed-size reporting false. A build receipt identifies that profile and the caller's Node options. The default build is unchanged. No recovered-source production build, semantic check, runtime HTTP suite or browser journey has passed yet.

Recovery checks executed for this new source:
- Node syntax checks for the overlay, anchor tool and five composition/build scripts passed.
- The source manifest composer completed with the public gate off and on, including original hash and exact retained markup/style fences.

Those checks do not validate TypeScript semantics, actual backend responses, browser rendering, IndexedDB migration or product activation.

A newly reconstructed dependency-free Node VM suite now passes 18 finite client/query/DTO/media/native-forum/migration-transport seams. It imports the actual reconstructed TypeScript via Node's type-stripper; it uses fixture transports and performs no real network or database operations. The adult-parent chapter seam verifies that both visibility and chapter-parent proof send all four content ratings. This is not a replay of the old 62-test suite or lost HTTP/browser acceptance.

Run: `node --experimental-vm-modules --test weebdex-platform/tests/ci/public-api-recovery.test.mjs` from the repository root with Node24.
