# Device updates continuation candidate

This source candidate replaces eager device followed-title chapter hydration in
both My Updates and the personal latest-title carousel. Account endpoints are
unchanged. Exact locally notified parents remain authoritative; their captured
revision, account authority, namespace, preferences and navigation own each read.

Each explicit action processes at most 20 parent pages and 1,000 observed chapter
rows, also bounded by the existing 40 requests, 8 MiB network, 1 MiB response,
4 MiB retained-representation, 10-second time and settled cleanup budgets. The
public source first filters one exact parent, then reads one exact chapter page;
selected output hydration reuses the bounded ID+parent history adapter. Native
queries retain native serialization and catalogue comparison keys.

Global order is withheld until all parents have been considered. The latest-only
carousel first chooses each parent's greatest normalized publication instant,
with ascending chapter ID tie breaking, separately from the requested global
order. Its selected publication fingerprint is persisted and checked again on
hydration, as are parent, identity, visibility and global comparison keys.

The temporary shared title-index v2 adds an owner/identity unique index and an
owner-scoped observations store for nonwinning latest-only chapters. Both share
transactional record/byte admission and cleanup. No saved-library store changes.
Blocked schema opens remain owned until actual settlement; old connections close
on versionchange, and failed migrations abort without deleting old scratch data.

## Qualification and limitations

Source `7156194` passed 20 actual pager/fake-IndexedDB controls, 5 hash-pinned
historical/current counterfactual controls and 9 compiled/mounted Svelte controls.
The mounted actual pager traversed 1,005 sparse parents in 51 indexing actions
with actual fake-IndexedDB transaction settlement. Eight actual public-adapter
controls passed at `5bee010` after correcting the VM shared-dependency loader.
Two original-source/local-overlay controls then caught an ambiguous carousel
anchor shared by its module and instance scripts. After narrowing the anchor to
the unique instance-script/markup boundary, both controls passed at `85cb71b`.
Across these pinned runs, 44 scoped controls pass. This is not a single full
composition or full application typecheck receipt.
The failed suite and mounted success are retained as separate receipts. Earlier
18/8-control receipts from the source committed as `48578a0` remain historical.
This is not native
browser, real HTTP, sustained load, memory measurement, full composed strict or
30K/500K acceptance. One initial assertion expected the word "timestamp" while
the actual correct invalid-date refusal says "Invalid saved-device date"; the
narrow expectation was corrected. No product change was made for that failure.

The 1,005 sparse-parent control needs 51 explicit indexing actions, rather than
one action per parent. That remains a usability limitation and is not a large
library acceptance claim. DTOs are not fetched or retained as a whole catalogue;
lightweight parent metadata is O(N), and temporary keys/observations are bounded
by the shared scratch quotas. Quota refusal never silently truncates results.

The public adapter refuses offsets beyond 10,000 within one parent. There is no
existing stable snapshot/keyset contract on this path. Totals, monotonic order,
identity collisions and changed selected rows detect some live changes, but
cannot detect all offset-page omissions or same-total replacements. Completed
counts describe observed traversal, never a server-atomic snapshot. Per-parent
ceiling, live-catalogue consistency, whole-library performance and native browser
acceptance remain explicitly incomplete.

Public chapter SQL uses the primary value then an unexposed native ID, whereas
the retained global device-library comparator uses the same primary value,
secondary chapter/volume text and public UUID. Public scan monotonicity therefore
checks only the exposed primary (ties allowed); native scans check their complete
comparison key. The final index preserves the retained global client comparator.
This is an explicit compatibility interpretation, not literal public SQL tie parity.

## Retained caller boundary

The retained authored TS/Svelte graph has exactly two personal-update callers:
`frontend/src/routes/(app)/library/updates/+page.svelte:25` calls
`MangaFollow.LatestFeed`; `frontend/src/lib/components/MyUpdatesCarousel.svelte:32`
calls `MangaFollow.LatestUpdates`. The route's explicit device branch and the
carousel's explicit device branch plus drain guard replace both with
`DeviceUpdatesFeed`, which calls `openDeviceUpdatesFeed` directly. Public updates
components use `API.Chapter`, not this personal-library compatibility path.

The legacy device `LatestFeed`/`LatestUpdates` wrapper deliberately returns 409
after its one bounded indexing action for any nonempty notified-parent snapshot,
even if filtering leaves zero chapters. Only an empty parent snapshot can return
a ready empty response immediately. This is migrated retained-UI coverage, not
generic legacy API parity. Native account paths are unchanged. Full composed
preparation and strict qualification of both explicit branches remain required.
