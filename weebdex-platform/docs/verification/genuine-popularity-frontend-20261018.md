# Observed popularity frontend successor

Source: `06963196aa4295e548e195a3cef78e3ed616e795`.
Backend wire contract: `179dbae` (actual backend qualification is separate).
This isolated successor does not overwrite the earlier qualified683-case freeze.
No heavy frontend build, installation or dependency copy was performed here.

The homepage's existing Manga Trending call now consumes genuine instance-local
window rankings. The retained Popular page activates the original daily, weekly
and monthly title components plus their original chapter cards for the same
windows. Actual eligible empty collections remain empty; failed reads produce
an error rather than a false zero-observation result. Canonical source is unchanged.

`popularity.ts` preserves declared filters/includes, supplies limit10 before
translation, accepts only rank=read and24h/7d/30d, rejects client order and nonzero
offset, and verifies the exact observed window/source/order metadata. Resource
UUIDs must be unique; positive BIGINT scores remain exact decimal strings and
must follow server score order. Equal-score server ties are not re-sorted by
unrelated public UUIDs. Chapter cards display the actual window score and only
real hydrated native forum counters. They do not invent all-time views or thread
statistics. All existing title/account/reader fences remain scoped.

The frontend-only no-rating sentinel is validated as an exclusive selection and
resolved to an explicitly labelled local empty view without issuing a default
rating request or inventing a server observation. Limit0 still queries the actual
server eligible total. Manga preferences and chapter language/group/uploader
preferences remain in their respective top requests. The existing homepage
latest-query lifecycle is preserved.

Existing anonymous view calls now use exact same-origin POST paths, preserving
the backend visitor-cookie dedup/rate/privacy policy. Only empty204 acknowledges
success. The original published/readable-page, bot and one-second dwell checks
remain. Each mounted analytics component owns its timer and writes a typed
session marker only after a confirmed response. Errors are not replayed or
marked successful. Unrelated native catalogue writes stay fenced.

## Lightweight evidence

`test-evidence/frontend/genuine-popularity-20261018/` retains the exact source
pins, final raw log, result and rerun recipe.

- 120/120 cases pass, zero skipped: 23 new production-client finite cases,
  11 actual composed analytics-caller script cases, six compiled page/window/
  chapter Svelte/JSDOM cases and80 affected regressions.
- Real zero observations, explicit empty preferences, BIGINT precision, exact
  windows, score order/ties, malformed metadata, bad paging/sorts, unavailable
  endpoints and the actual retained query-string delimiter override are covered.
- Analytics tests cover exact typed UUID/legacy resolution, same-origin cookie
  intent, Retry-After,204-only acknowledgement and disposal/dedup/dwell guards.
- Actual production preparation runs under both gates and verifies all original
  markup after narrowly enumerated exceptions. Registry growth is explicit:
  46 source files, nine availability pairs, three media and ten feature exceptions.
  The registry mutation test restores its captured original length correctly.
- Independent read-only review of all six production paths, the final sort[]
  refinement and six compiled DOM seams found no concrete blocker.

Production clients/query/normalization and the actual composed original page,
window and chapter components execute. HTTP, prefs store, leaf controls/cards,
and the analytics script timer/storage lifecycle are explicit finite boundaries.
This evidence does not establish the whole mounted app, real backend HTTP,
native-browser behavior, final composed strict/Vite acceptance, production
readiness or upstream parity. Those qualifications require their own exact pins.
