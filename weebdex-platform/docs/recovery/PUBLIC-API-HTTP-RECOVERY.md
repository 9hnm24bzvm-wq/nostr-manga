# Reconstructed HTTP acceptance runner

The recovery runner `scripts/public-api-client-acceptance.mjs` imports the actual typed public client and retained-view bridge through Node24 VM modules. It performs genuine HTTP requests, never intercepts API responses and rejects requests outside `/api/v1` plus the explicit namespace-capabilities exception. Media requests are bound to the owned fixture origin. Authorization headers and the user's60123 preview are rejected.

It requires a disposable fixture manifest containing `fixture_only:true`, `origin`, an exact `source_commit`, `ids:{manga,chapter,cover,author,group,tag}`, `native_ids:{manga}`, and `title`. The fixture must seed real published chapter pages, cover original/256/512 derivatives, and actual native forum records or an honest null context. The runner checks the actual typed browse/detail/feed/reader, aggregates, tag/author/group, media bytes, aliases, partial statistics, missing resources and no native catalogue fallback.

Run with `node --experimental-vm-modules scripts/public-api-client-acceptance.mjs`, `WEEBDEX_PUBLIC_API_FIXTURE_MANIFEST` set to the owned absolute manifest and `WEEBDEX_PUBLIC_API_FIXTURE_SOURCE` set to its expected40-hex source commit. Optional `WEEBDEX_PUBLIC_API_ACCEPTANCE_OUTPUT` must be a fresh owned absolute output path. Run clients in the fixture's process namespace when the executor isolates each tool call's network.

This new runner was syntax checked only. No rebuilt HTTP fixture is available yet. Historical nine-step/31-request success from the lost source does not apply to these reconstructed bytes. Its media checks confirm actual HTTP bytes and content type, not browser image decoding. The complete browser acceptance and durable device-ID migration remain open.

Compile a test fixture executable with `cargo test --no-run`, copy the exact identified executable into an owned acceptance stage and record its source/executable hashes before launch. Run the copied executable directly under the disposable PostgreSQL wrapper. Do not hold the shared Cargo target lock for the life of an HTTP/browser fixture.
