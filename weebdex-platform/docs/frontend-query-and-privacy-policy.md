# Frontend query startup and privacy policy

## Corrected query interpretation

The attachment intentionally overrides `URLSearchParams.prototype.toString()` in
the root universal `src/routes/+layout.ts` module. It prefixes nonempty query
strings with `?`. The retained API wrappers therefore concatenate endpoint and
query string correctly after root layout initialization. Empty query strings
remain empty.

The global root layout module is loaded as part of normal SvelteKit route startup.
Mounted component requests run after route loading/rendering. The early
session-module request is `/auth/check`, with no query parameters. Installed
SvelteKit 2.56.1 starts root layout preloading eagerly, but loads route modules
and universal loaders in parallel. Static inspection therefore does not prove
patch ordering for every fresh deep-link page loader. A test that bypasses root
initialization and constructs malformed endpoint paths does not establish a
defect in the normal frontend. Runtime checks should include fresh `/title/random`
and normal homepage/search initialization.

A speculative server URI-rewriting module was removed. The server should retain
ordinary route and query semantics rather than expose alternative malformed
paths. Actual browser startup and request URLs still need browser verification;
source inspection is not a replacement for that runtime check.

## Deliberate deployment privacy policy

The supplied root component embeds the upstream Google Analytics property
`G-W464K5049G`. Its original source bytes remain preserved, but testing and
deployment must not send test or visitor data to that upstream property.

The Rust server should set a default Content Security Policy that blocks external
analytics script loading, connect requests, image beacons, and frames. The local
application bootstrap must be allowed using verified script hashes or nonces.
Do not weaken script policy with unrestricted inline or external execution in
order to run the retained tracking code. The same-origin platform API, auth,
media, styles, fonts, and required application assets are the intended defaults.

Browser verification must use the Rust-facing same-origin endpoint configuration
and block original upstream API/auth/data services and analytics destinations.
Preserving provided source is distinct from allowing the original external
tracking behavior in deployment.

The production fallback SHA-256 is
`197f564b569993884cbc994fa0fcbf7f641ad917e31a942e1012e33e2d552865`.
Its single recognized inline bootstrap hash is
`sha256-U6mIsgsEotS3ywlYStsKln2EvM1pJblQUP4MTcei7GQ=`.
`backend/src/privacy.rs` computes hashes from the served index at router creation;
missing or unrecognized bootstrap input fails closed. The router's configured
UI directory must identify this build before server startup. The default policy
allows script/connect only from self, exact bootstrap hashes, required local
font/media/worker assets, and data/blob image previews. No unrestricted inline
script, eval, external script, analytics connect or beacon permission is added.
Inline style is retained for existing dynamic component positioning; this does
not permit inline script. Actual startup under the policy is still untested.
