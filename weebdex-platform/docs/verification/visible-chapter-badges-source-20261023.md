# Real-framework visible-badge fixture: source preparation

Status: authored source, uncompiled and unexecuted. This is not mounted badge
qualification. Current production revision is
`b2817b1c93d37b7286b22146d063fbd1059aaa4d`; comparison revision is
`14a7bab8a925928f658fbd8ebd160b3388e4b540`.

## What is actually mounted later

The new SvelteKit fixture has three routes: title, a plain chapter destination,
and an outside-group route. The title wrapper uses the unchanged current
VisibleBadgeHarness, which imports the actual MangaActions and
MangaChapterListSection. It does not mount full Manga or every app route. The
same production overlay factories, order, exact anchors, local API resolver and
case repairs compose those consumers. No framework state/router substitute is
provided. Framework modules and fallback/bootstrap must come from a fresh real
Vite/SvelteKit build.

New fixture infrastructure consists of configs, template, route wrappers,
command/observation bridge, DOM/emitted-module driver, HTTP/XHR/IDB boundaries,
and runner. Those are not unchanged production source. The copied root
+layout.ts is byte-identical to the current production raw initializer. The
config maps only that exact copied file back to its production overlay identity,
so the original root receives the actual reviewed production composition, not a
cherry-picked URLSearchParams initializer. Both raw and composed identities are
recorded. Its literal ssr=false/prerender=false are retained.

The nested group initializes installReaderNavigationGuard during component
initialization, not inside onMount. The real Kit beforeNavigate registration
belongs to that component; the actual guard onDestroy invalidates epoch/intent
and removes hashchange/popstate listeners. The outside route really destroys
that component. The persistent root bridge calls genuine goto, Session, and
library APIs; its snapshots read actual $app/state and read-only stores. It has
no store/epoch/page/controller reset command.

## SSR and generated closure

Pinned Kit 2.56.1 statically extracts literal universal route options and carries
parent ssr=false into node options. Its normal server-node generation emits
universal options instead of importing the browser-only initializer when SSR is
false; analysis does not import client components. This source analysis removes
the supposed need for a DOM shim during SSR analysis. It is not a measured build
result: later inspect actual emitted server nodes and fallback. Type-only
`svelte-kit sync` does not create the required app/root closure; the actual Vite
plugin build does.

The config records raw and production-overlay transform identities and actual
virtual-source text/hashes, plus chunk-to-module provenance. Real generated
root/app/node/manifest files, all client assets and exact adapter fallback/inline
bootstrap require a separately reviewed manifest. A self-declared
frameworkReview status is not admission: the outer owner must verify the exact
review file, input/output identity linkage and full dependency closure before
running the runner. Package/entry hashes checked by the driver alone are not a
transitive-package closure proof.

## Profiles and cases

Native0 runs the unchanged twelve original groups in their original order.
Public1 runs seven separately named device-only groups, never forced account
mode. Two new native terminal supplements cover returned-body read/cancel
settlement across same-graph remount and permanent uncertain-mutation fencing
across same-graph remount. Framework controls separately exercise genuine pending,
hash, cancelled, load-error, A-B-A destroy/remount, and guard-omitted cases. A
fixture-only XHR abort/reuse control checks stale bodies are cancelled/drained
and cannot mutate a reused XHR.

Held identity, completion headers, chapter feed, returned body EOF, body-cancel,
IDB terminal and route-loader gates are separate. Pending navigation does not
commit a fake page. Cancellation has an actual navigation.cancel event witness;
Kit's callback forEach still invokes the nested guard. A loader error can commit
a real error page and is not called cancellation.

Each later independent configure recreates an authentic document/module graph
only after zero outstanding HTTP/body/IDB terminal evidence. This isolates
intentionally persistent product state between independent scenarios. Remount
and uncertain-fence supplements never recreate the graph, reset a controller,
or release an unsettled owner to make a case pass.

The old comparison is a separately generated graph. That revision lacks
VisibleBadgeHarness, so its input manifest must identify the exact current
harness and these new wrappers/adapters as shared fixture infrastructure. Actual
old production consumers and overlays come from OLD. The old initializer raw
hash is identical to current. Missing old assets, a build/import failure, empty
feed or unhealthy router are infrastructure failures, never a successful
negative. The semantic control requires all 51 actual old feed rows and an
observed read/unread inference before a complete bounded projection.

## Toolchain and scope

Use retained Svelte 5.55.1, Kit 2.56.1, Vite 7.3.1, plugin-svelte 6.2.4,
adapter-static 3.0.10, Vite's nested Rollup 4.60.1 and esbuild 0.27.7. Existing
Noble curves/hashes 2.0.1 inputs are pinned separately. The DOM owner uses
jsdom 27.0.1 and fake-indexeddb 6.2.4. No dependency installation or copies are
part of this preparation. One actual Svelte runtime identity is required.

The driver executes exact generated classic bootstrap text with synchronous
currentScript semantics and only manifest-declared same-origin emitted assets.
It supplies finite fetch/XHR/IDB boundaries, not app/module mocks. It supports
zero-scroll and an inert preload IntersectionObserver whose observe() fails;
that is an explicit nonvisual-fixture limitation. Unknown DOM or framework
semantics fail infrastructure. This does not qualify real browser rendering,
layout, pointer geometry, full reader UI, PWA or the full application.

## Next admission, not execution

First review the frozen postimages, overlay-aware input inventory and exact
package closure. Materialize only allowlisted source Git blobs and these
postimages into a fresh owned projection, with read-only bindings to existing
package roots. Eager overlay anchor checks need some raw files outside the
rendered graph; those are validation inputs, not routes or imported full Manga.

Then admit one native0/guard-installed generation using the exact bounded plan.
Use the existing reviewed pidfd/subreaper owner and its established resource
floors, bounded logs and terminal cleanup. Preserve all failed attempts. There
is no new OS/quota/security setting requirement. Report sampled maxima/cadence
as sampled, never as a guaranteed hard peak. Inspect the new generated closure
before admitting any mount. Public, old-source and guard-omitted generations
are sequential, each with its own source/output identities and shared total
allocation accounting. No historical generated snapshot may fill a gap.

The authored badge-framework-generate.py is a native0-only generation launcher, not a test launcher. Its external generation-freeze.json pins source modes/bytes, full retained package-tree digests, executables and the unchanged reviewed lifecycle owner. It checks the proposed 128 MiB envelope periodically, records disappearance races in sampled allocation walks, and uses strict final censuses. Source bytes/modes and package trees are reverified after generation. No sampled reading is a hard peak guarantee. Generated manifests remain UNREVIEWED and cannot satisfy the mounted runner until separate actual closure review.
