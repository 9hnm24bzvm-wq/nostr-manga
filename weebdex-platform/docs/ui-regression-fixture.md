# Optional native UI regression fixtures

These private provisioner controls require the owned disposable browser fixture
marker, exact reserved loopback database and opt-in supervisor flag
`--ui-regression-controls`. They add no production endpoint or authentication
credential. Source/type-check/codegen receipts do not establish runtime success.

- `ui-regressions-seed`, scenario `pagination`: 505 Spanish compact chapters,
  101 French terminal-page chapters and 101 chapters isolated by group, author
  and uploader. Chapters use `CatalogRepository::publish_chapter`; original
  media references reuse a verified actual600×900 PNG from the base fixture.
- The same seed installs10,000 metadata-only native search titles matching
  `UI16CapFixture`. Actual title plus JSON storage is bounded4MiB. No fabricated
  activity, images or signed-protocol association is attached.
- Four preference titles have real seed-time recency. `UiPfLang01` is Spanish
  and has the existing `SeedGenre1` tag. Most Read tests must make genuine
  production analytics POSTs and verify204; no view counters are seeded.
- `ui-regressions-readers`, only after compact Spanish checks: real en/es
  alternatives, a4-page portrait reader and an8-page reader with landscape pages
  at positions3 and7. One actual900×600 PNG adds at most1MiB. Activating the
  bilingual Spanish chapter intentionally changes the global Spanish feed from
  505 to506; it is not hidden to preserve an artificial count.
- `ui-regressions-withdraw`, scenario `terminal` or `entity`: removes only the
  recorded oldest row through the production catalogue deletion writer with the
  current persisted fixture operator. Repeating the exact named action returns
  its observed soft-hide state. A browser may instead use ordinary authorized
  chapter DELETE. No arbitrary target, SQL or remote endpoint is accepted.
- `ui-regressions-status` returns the recorded descriptor with freshly queried
  visible chapter counts. Initial receipts are private create-new JSON files;
  the supervisor publishes only public fixture identifiers and dimensions.

Every control requires explicit phase/deadline acknowledgment in the supervisor.
Read-only API-client403, cookie401, public429 and public503 cases must use genuine
app behavior or separately labeled UI-only fault tests. This extension does not
provide a database-failure/capacity backdoor and does not claim power-loss,
performance, delivery or browser acceptance.
