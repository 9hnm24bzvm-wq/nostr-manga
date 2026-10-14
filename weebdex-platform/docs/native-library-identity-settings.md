# Native Library and Public identity settings

The native Settings navigation owns three anonymous pages:

- Preferences retains the existing reading/display preferences.
- Library (`/settings/library`) owns device/account choice, private device JSON
  backups, saved lists, optional public sync, and deliberate recovery tools.
- Public identity (`/settings/identity`) owns connecting a browser signer,
  editing a public name/profile/follows, and checking saved public actions.

Account and API Clients keep their native visibility and authorization guards.
Library management itself has no account, catalogue-scope, or storage-readiness
wrapper. A signed-out account selection or failed IndexedDB open cannot hide the
mode choice, backups, recovery link, or an honest error. The existing CSV/XML
controls retain their reviewed device/account destinations and whole-import
context fences; their destination is explicit, and actual account operations
require a session. Original 676 supplied files and three approved announcement
additions remain byte-identical. The identity route is materialized only in the
build/check projection from an explicit four-route registry.

The pages use existing tab anchors, Button/Input/Dialog components, headings and
row spacing. There are no settings accordions. Optional tasks open bounded native
dialogs; sync setup, publishing, public recovery and saved attempts are separate
tasks. Ordinary pages show no raw keys or record dumps. Exact identity/source,
canonical records, saved-event proof and backup interpretation remain available
when explicitly inspecting or reviewing the relevant action. Uncached saved IDs
are retained in list/recovery tasks when needed to distinguish titles; no title
is invented and no old-scope ID is reinterpreted.

Every dialog/task has a synchronous open/generation fence as well as the child's
route/disposal fence. Close, Escape, task replacement or navigation invalidates
pending reviews and new signer/transport work. An already returned signed snapshot
still passes the original CAS checks and is durably saved before transport stops.
The existing final asynchronous IDB/beforeSend fence is retained. Lists check both
dialog and route ownership after awaited reads before changing mode or navigating.
No IndexedDB schema, record normalization, publication policy or backend query is
changed.

Tailwind v4's automatic scan covered the copied frontend but omitted external
integration components. The production Vite integration now adds one build-local
source declaration for the reviewed integration directory. Original CSS is not
edited; an emitted integration-only native-dialog selector is checked, and build
paths do not appear in static CSS. Actual browser geometry remains a separate gate.

Tests use pinned Svelte/TypeScript, real signing/publication/queue/IDB modules and
real Noble signatures, with explicit DOM/native-dialog/worker/locks/IDB/network
adapters. They include actual compiled-page Close/Escape/task-switch/route fences,
exact saved-job bytes, zero late POST, unchanged storage reopen, no-thread readers,
anonymous route ownership and technical-detail disclosure. These are not browser
focus/layout/extension/server acceptance. The earlier a424 browser run and failed
mobile-arrival probe are separate historical lineages; neither proves this build.
