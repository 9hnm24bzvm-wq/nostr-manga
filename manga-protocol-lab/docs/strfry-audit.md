# strfry moderation integration audit

Pinned checkout: `4cd3cf64850caf47dda46c2a2abbbf3525a64d10`, checked 2026-10-06. Findings below are source inspection until explicitly marked runtime-tested.

## Finding 1: accepting a kind-5 request executes deletion

`src/events.cpp::writeEvents` processes kind-5 requests in the same write path as event storage. Matching-author e-tag targets are scheduled for deletion; the transaction inserts the request and removes targets. `golpe.yaml` additionally indexes e-tag target+author pairs as deletion records. A later matching target is rejected by `writeEvents`, even if the request arrived before its target.

This directly conflicts with the requested combination of retaining/propagating a signed deletion request while keeping a chapter or series until a local moderator approves. It is expected NIP-09-oriented behavior, not a claimed upstream vulnerability.

Sources:
- https://github.com/hoytech/strfry/blob/4cd3cf64850caf47dda46c2a2abbbf3525a64d10/src/events.cpp
- https://github.com/hoytech/strfry/blob/4cd3cf64850caf47dda46c2a2abbbf3525a64d10/golpe.yaml

## Finding 2: write-policy acceptance is event-wide

`PluginEventSifter.h` returns Accept, Reject, or ShadowReject. `RelayWriter.cpp` calls the plugin before `writeEvents`. Rejecting a kind-5 event prevents its storage in this path; accepting it permits built-in deletion. ShadowReject acknowledges without storing, which is not the required durable retention and propagation behavior.

A signed deletion request can contain both protected chapter targets and ordinary social targets. Splitting or rewriting it invalidates its original signature. Policy must therefore be evaluated per target while retaining the original envelope, or the wrapper must own a separate authoritative request store and compatible read/sync paths. A simple accept/reject plugin cannot by itself provide store-without-delete semantics.

Sources:
- https://github.com/hoytech/strfry/blob/4cd3cf64850caf47dda46c2a2abbbf3525a64d10/src/PluginEventSifter.h
- https://github.com/hoytech/strfry/blob/4cd3cf64850caf47dda46c2a2abbbf3525a64d10/src/apps/relay/RelayWriter.cpp

## Finding 3: import is a distinct policy boundary

`cmd_import.cpp` feeds `WriterPipeline`, which calls `writeEvents` without invoking the relay write-policy plugin. The import command explicitly disables event-time validation; ordinary imports still verify signatures unless `--no-verify` is selected. Fast `--fried` imports take a different packed-event route. Live sync and stream have plugin calls; router uses separately configured directional plugins. No production ingest route should be assumed covered merely because the public WebSocket route is covered.

Sources:
- https://github.com/hoytech/strfry/blob/4cd3cf64850caf47dda46c2a2abbbf3525a64d10/src/apps/dbutils/cmd_import.cpp
- https://github.com/hoytech/strfry/blob/4cd3cf64850caf47dda46c2a2abbbf3525a64d10/src/WriterPipeline.h
- https://github.com/hoytech/strfry/blob/4cd3cf64850caf47dda46c2a2abbbf3525a64d10/src/apps/mesh/cmd_sync.cpp
- https://github.com/hoytech/strfry/blob/4cd3cf64850caf47dda46c2a2abbbf3525a64d10/src/apps/mesh/cmd_router.cpp

## Design alternatives, not adopted decisions

1. Small upstream-compatible fork/hook at the storage boundary: preserve request storage, separate per-target deletion authorization and tombstone indexing. Requires maintaining a patch and tests across upgrades.
2. Wrapper-owned kind-5 store and merged read/reconciliation interfaces: prevent protected/unknown requests from reaching automatic deletion paths, while retaining exact signed bytes externally. Avoids some upstream mutation but greatly increases wrapper responsibility, especially mixed-target requests and unknown targets.
3. Change the protocol's deletion event design. This revisits accepted choices and cannot be adopted autonomously.

Runtime experiments will use synthetic keys/events and private temporary databases, never a public relay or real publications.
