# Finite UI7/UI8 source handoff

Current exact source hashes and anchored changes: `test-evidence/frontend/round7-owner-source-repairs/summary.json` and `integration-source-manifest.json`. Source syntax/Svelte preservation checks pass. No browser/application test has executed. This owner handoff is not independent signoff or production-readiness evidence.

## Native write authority and direct transport inventory

The pure `mutation-authority.ts` registry binds synchronously to the real Session store before initialization. Reset-only recovery and anonymous state have no actor. Every Session write advances revision for stale-write fencing. Only actual actor/role/permission/verification changes advance stable lifecycle epoch; logout then login, including the same actor, advances it. Harmless follow/history/preferences refresh cannot remount Reader and trigger repeated automatic history writes.

Credentialed same-origin native API POST/PUT/PATCH/DELETE captures actor and immutable JSON/FormData intent before limiter waits, adds expected-account only when absent, rejects mismatched/duplicate explicit actor headers, cancels its owned timer/request on authority changes, and fences immediately before send and response attachment. Mutations are never automatically replayed. An ambiguous already-sent mutation keeps truthful may-have-succeeded/reload guidance. The original57 REST mutation-function inventory has55 credentialed writes and2 deliberately anonymous same-instance view counters; these are not account writes. The inventory also separately classifies local IndexedDB, explicit signed original-protocol publication, auth SDK flows, read-only mapping POSTs and announcement direct fetch.

Thread create/edit/remove/delete/vote/unvote uses captured component actor/target/disposal intent, not a later cookie identity. Desired vote state is captured and applied only after matching acknowledgement; no optimistic rollback can overwrite another account/target. Current personal replies/votes/follows/announcement reads omit anonymous credentials or recheck actual authenticated identity, reject late contexts and never restore private DTOs from history.

## Discussion lifetime and preferences

The generic discussion provider reads its current Thread prop through a getter and keys children by thread plus stable authority epoch. It resets private transient state only when that actual context changes. It covers forum, title, chapter, group and user discussions. Feed completions are bound to exact thread/query/authority/disposal; no delayed completion writes whatever route is currently in history. Public statistics also reject disposed targets.

Settings, Group and User preference actions snapshot baseline and payload, leave current blocked arrays untouched before acknowledgement, and apply a real response only through `applyAccountPreferences(expectedActor, capturedRevision, actualBody, intent)`. Failed/stale writes cannot restore another account's preferences or mutate its arrays. Group/User Follow intent is captured synchronously before any await; no delayed `Session.isValid` admits an old click as a later actor.

## Resource/control behavior

Duplicate page removal keeps all pages and blob thumbnails on denial or ambiguous server outcome. Only successful acknowledged remote deletion or explicit local discard releases unused URLs; retained shared URLs stay alive. Only acknowledged64-hex content hashes prove duplicates. Unknown/unsent/rejected hashless pages remain distinct even when two folders use the same filename; no upload POST is replayed.

The retained Thread Share button now has one exact approved handler/title/accessible-name anchor. It invokes Web Share immediately from the user gesture or awaits real clipboard copy of a same-instance canonical thread URL. Cancellation is not an error, failures are truthful, and no automatic external posting occurs. Existing layout/classes/styles remain unchanged.

## Deferred E2E requirements

Queued/delayed A mutations -> logout/B/A-again and target navigation; no B audit actor, no stale completion/rollback, truthful lost-response reconciliation. Thread A -> B/query changes with delayed feed and no private history restore. Account-mode reader mount/history refresh must settle without repeated POST/remount. Settings/block responses held across account switch must not touch B. Group/User follow click while forced auth refresh holds must never re-admit as B. Native Web Share success/cancel and clipboard success/failure through the actual Share button. Duplicate DELETE403/500/lost response retains live previews; distinct same-basename known-unsent images stay ordered and distinct. All are acceptance requirements, not passing results.

Original676 bytes remain identical. The provider key and existing-share handler/name are explicitly enumerated behavior exceptions, alongside prior approved local panels/staging/recovery/target keys. Globally identical executed HTML is not claimed. Fragment files and frontend-behavior/HTTP.ts are historical explanatory copies, not mounted authority; exact overlay `after` bodies and imported external modules determine execution.
