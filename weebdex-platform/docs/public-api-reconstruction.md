# Public API reconstruction and contract boundary

The execution filesystem was replaced on15 October2026 around04:35UTC. The public
starting product was recovered exactly at82290c732cc19b393b128fec0879885937f41054.
The subsequent MangaDex feature source was reconstructed, not recovered byte for
byte. Only new checks against the current bytes establish acceptance.

## Version and routing

The source of truth is the derived113-operation/80-path/102-schema inventory at
`contracts/mangadex-5.13.1-inventory.json`, pinned to official OpenAPI5.13.1,
SHA256`004031ccdfc4b66727dd1f85d09b5300cf60235bbab2de6f6ea4816094bd6d33`.
The public base is `/api/v1`, with unchanged upstream suffixes. Existing `/api`
retains native modules and bounded legacy integration. Known pending operations
return explicit JSON501, unknown paths404, unsupported methods405; reserved API
and media namespaces do not fall through to successful SPA HTML.

Public resource UUIDs are persistent presentation aliases. Native database keys,
foreign keys, immutable source hashes/pubkeys, storage filenames, and signed
32-hex list IDs retain their exact identities. Author and artist share one author
identity with different relationship roles. Typed namespace resolution never
confers visibility or authentication. Source/user/list presentation aliases never
create accounts or bindings. All historical migration bytes remain unchanged.

## Intentional protocol and instance behavior

- Nostr proof/admission replaces MangaDex OAuth login, refresh and logout. Auth
  check verifies the actual principal; it is not a successful no-op.
- Protected native editorial/moderation/settings operations additionally require
  an explicit current native-account binding, active verified account and current
  native permissions, checked under the operation's transaction locks.
- Library read markers, lists and reading statuses use the one original signed
  public graph and final original head/admission checks. Unsigned JSON cannot ask
  the server to sign a replacement graph. Follow flags are not inferred from
  saved series or reading statuses. Original snapshots contain no reliable read
  dates, so a MangaDex reading-history timeline cannot be fabricated.
- At-home uses canonical chapter UUIDs as transport delivery tokens, retaining
  original signed hashes and opaque filenames. It advertises only real originals
  and persisted derivatives. `nostr.pageDimensions` and `dataSaverDimensions` are
  independently persisted rendition sizes; absent dimensions stay absent.
- Native forum contexts are an explicit read-only namespaced companion. They
  return real persisted native thread DTOs or null, with native thread IDs.
- Locally moderated reports, editorial metadata, native settings and counters
  are instance-local data. Native rating/follow counters are not a new signed
  per-user rating/follow model. Group popularity/relevance sorting remains501
  rather than inventing global ranking. Native publication/readable clocks share
  the actual publish timestamp; future filters act on that clock.
- Native custom tag categories retain explicit provenance rather than pretending
  to be a standard MangaDex group. Provider-specific captcha, numeric MangaDex
  legacy mapping, external forum creation and missing protocol mutations are
  pending adapter decisions or implementation, not automatic exemptions.

## Fresh acceptance and activation

New focused units and actual HTTP/PostgreSQL fixtures verify the identity89/edge90
boundary, genuine original alias import, reports, statistics and native editorial
core. The catalogue/media/UUID recovery fixture and genuine original-runtime
withdrawal/status suites passed at64973e5 with verified stopped isolated databases.
The newly restored editorial/draft transport separately passed genuine signed HTTP
and unchanged native CRUD regressions at53d21ea. Exact logs, hashes and scopes are
under `test-evidence/`; no old lost result verifies reconstructed source.

The frontend is composed as overlays over all679 retained canonical files. The
public catalogue gate remains opt-in (`PUBLIC_MANGADEX_CATALOGUE=1`, default0).
Fresh typed-client HTTP acceptance has passed, but that is not a browser or
product-activation result. Saved-library UUID publication remains fenced in the
preview until compatible writes and explicit migration readiness are established.
The operation inventory must remain honest about partial/pending features; this
checkpoint is not a full drop-in API or complete site-parity claim.
