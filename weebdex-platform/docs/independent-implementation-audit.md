# Independent implementation wiring audit

Updated: 2026-10-13 04:35 UTC. Status: **finite source-wiring review closed: 17 of 17 implementation findings source-closed. No known unwired implementation gap remains in the reviewed scope. AUD-15 is the separate unrun testing gate.**

This review examines actual callers, handlers, persisted authority, original-module
seams, failure paths and lifecycle ownership. It is not a route-count, compilation,
runtime, security-certification or production-readiness claim. No runtime test,
Cargo command or performance test was run during this review. Production-source
owners have reported their final freeze and the final namespace/publication caller
changes were reread. Line numbers identify inspected snapshots; hashes below pin
the final reviewed seams. A subsequent fresh independent review or test can reopen
a finding if it supplies a concrete substantive defect.

## Source findings and acceptance gates

| ID / severity | Evidence and user impact | Owner / current source disposition | Required observable acceptance |
| --- | --- | --- | --- |
| AUD-01 P1 | Original `frontend/src/lib/HTTP.ts:104-112` retries ambiguous mutation transport failures and can restart cancelled requests. Duplicate title/group/comment writes are possible. | Frontend. Replacement `platform-integrations/frontend-behavior/HTTP.ts:37-123` now retries only GET, owns cancellable timers, settles once and surfaces mutation outcome uncertainty. Account-mode raw-fetch writes in `platform-integrations/local-library/library.ts:150-165` also surface transport/body/post-write reconciliation uncertainty and never replay. Source reread satisfactory; execution pending. | Accept a mutation, drop its response and prove no duplicate submission. Cancel pending limiter/retry/upload work and prove it never restarts. UI must show uncertainty and reconciliation guidance. |
| AUD-02 P1 | Original `frontend/src/service-worker.ts:24-35,39-68` caches authority-sensitive bytes without honoring current revocation. First repair's NetworkOnly route was registered after the broad favicon route, so `/covers/favicons00/...` or URL query matching still entered stale cache. | Frontend. Repair in `platform-integrations/frontend-overlays.mjs` now registers NetworkOnly immediately after clientsClaim, before every cache route, and deletes exact legacy covers and favicons caches. Source reread satisfactory; installed-worker execution pending. | Install the worker, warm normal and matcher-adversarial private/public media, revoke/delete/logout/switch account, and verify network authority on every subsequent read and no stale private bytes. Offline local-library records must survive activation. |
| AUD-03 P1 | Original upload page deletes the current session on mount; edit cancellation ignores failed DELETE. Lost publication responses cannot restore exact draft/order from GET `{id}`. | Catalogue/bridge/frontend. Versioned PATCH, full owner DTO and durable exact native commit payload added in `backend/src/domains/uploads/sessions.rs:152-270` and migration `0072_upload_resume.sql`; external restore/autosave/immutable retry UI is mounted through anchored overlays and `frontend-behavior/upload-resume.ts` plus the two resume fragments. Current receipt and mutation/drag/removal boundaries reread satisfactory; production-source freeze reported, execution pending. | Reload staged work and restore images/order/metadata. Reload prepared/accepted publication and retry only identical saved input. 409/revocation/missing config must retain staged work, show truthful error and forbid edits/cancel. Successful cancel may clear UI only after server confirmation. |
| AUD-04 P1 | Previous follows/history were exclusively account/PostgreSQL API calls; library/history layouts required password Session. Original local-first browser library and signed modules existed separately, with no WeebDex adapter or opt-in publication UI. | Local-library/frontend/bridge. Actual device IndexedDB API replacements, default device guards/reader progress, explicit account mode/import, namespace-safe native backups, portable canonical import/worker/recovery/publication, and actual signed profile/social/withdrawal panels are mounted through composed overlays. Private/library history-state caches are bypassed and late results have view-identity guards; expected-account constraints are enforced server-side. Final mounted caller source reread satisfactory; production-source freeze reported, execution pending. | Logged-out browser follows/history/page progress work durably on-device with zero private account writes/public publication by default. Login/logout never replaces device records or signer identity. Explicit account mode/import/export and explicit signed-publication review/consent are distinguishable. Public profile/social/library use actual original supported operations where appropriate. |
| AUD-05 P1 | New `platform-integrations/local-library/library.ts:111-115` prepends `?` after calling an already monkeypatched URLSearchParams serializer, producing `??page=...`. Root original `+layout.ts` overrides prototype serializer. | Local-library/frontend. `library.ts:107-112` now serializes explicit entries at the fetch boundary. No second monkeypatched serializer is invoked. Source reread satisfactory; browser filter/page execution pending. | Filter/page/order inputs produce one correctly encoded query, return the intended catalogue subset and paginate without duplicate/missing pages. No caller depends on prototype invocation order. |
| AUD-06 P1 | Initial new `device-store.ts:9-10,44-55` backups contain native ten-character IDs without catalogue provenance. Foreign-instance import could reinterpret IDs and subsequently sign unrelated mapped records. | Local-library. Exact API origin, persisted catalogue namespace epoch (migration 0073), and original service identity equality are required. Foreign same-service/different-catalogue import is refused. Transaction-bound metadata scopes snapshots/import/adoption. `device-store.ts:99-152` now persists detected mismatch and checks it inside every native mutation transaction; `library.ts:12-17,43-71`, ChapterHistory and mounted reader/follow controls suppress current native interpretation after mismatch. Mapping and original publication admission also check the durable fence under the brief shared device Web Lock. Mounted bounded adoption preview shows exact current native IDs/titles/missing records before explicit binding. Final source reread satisfactory; docs truthfully promise inspection/export only after detected mismatch, with no silent rekey. | Foreign/missing scope is rejected visibly without changing data. Changed instance/service identity requires reviewed migration. A detected mismatch cannot admit fresh current-catalogue IDs into the preserved old namespace across retry/reload/another tab or publication signing. Canonical original backups remain portable; native-only/hidden/unmapped entries are explicitly excluded from publication. |
| AUD-07 P1 | Previous `catalog/repository.rs:511` delegated chapter authority from membership rows without live nondeleted/canonically-visible group eligibility. Deleted or revoked groups retained editing/removal authority. | Catalogue. `repository.rs:511-513,535-549` now locks current eligible group/member rows through transaction and validates fresh canonical group visibility. Source reread satisfactory; fixture source being completed. | Deleted/revoked group leader/officer cannot claim/edit/remove via old delegation or attribute new uploads to ineligible group. Actual uploader and independently authorized staff retain their real authority. Concurrent membership/deletion serializes. |
| AUD-08 P1 | Client issuance advertises self.library.read, previously absent from default grants; private reads checked user/role rather than scope. Moderator-owned library token inherited other users' private draft authority; whoami/check exposed account email. | Audit author, with parent authorization; **independent catalogue source review accepted 2026-10-13 04:02 UTC, with added expected-account test review accepted 04:25 UTC; runtime acceptance pending**. `auth/principal.rs:26-86,153-254` now exposes narrow browser/library capabilities, current read-only bearer allowlist and no ambient roles. Private library readers, browser-only account/social reads and catalogue/upload guards were updated. `tests/auth_http.rs:439-619` adds actual issued-bearer and expected-account Router acceptance source. | Library bearer reads only its owner's private library. It cannot inspect account email/settings/client management, private drafts/covers/staged files or private votes, or mutate any resource, even when account is staff. Missing/narrowed scopes, disabled/restricted/unverified owner, rotation and revocation fail closed; browser and recovery behavior remains correct. Matching account constraint works; stale identity conflicts before a browser mutation and malformed/duplicate constraints are rejected. |
| AUD-09 P2 | Source original bodies previously returned with platform admission permits dropped at handler return, unlike native streaming ownership. | Bridge. `protocol/media.rs:19-38,95-104,133-135,163-184` now embeds platform permits in the streamed original body while original Snapshot independently owns its download budget. Source reread satisfactory; execution pending. | Slow original readers saturate intended bounded admissions, release on completion/cancel, and remain counted during graceful drain. HEAD/304/range/error paths release without leaks. |
| AUD-10 P2 | Historical committed upload fallback could expose saved chapter DTO after current source/local visibility was withdrawn. | Bridge/catalogue. `sessions.rs:248-312` now returns fresh visible DTO or a minimal explicitly pending uploader receipt only after the actual original current-authority predicate. Historical profile/counter/thread relationships are omitted. Source reread satisfactory; revocation execution pending. | Repeat a committed request after source revocation/local removal and never return historical readable page/metadata DTO. Pending-source receipt remains minimal and truthful. |
| AUD-11 P2 | `frontend/src/lib/hooks/persistent-store.svelte.ts` parses old localStorage unguarded and update bypasses persistence. `reader.svelte:34-42` injects unescaped JSON into HTML script data; a title containing `</script>` can terminate its element. Root image sanitizer permits direct Imgur, conflicting with same-origin CSP. | Frontend mounted minimal parse/persist/escape/proxy script repairs. Corrupt bytes are preserved until explicit edit; update persists and failed storage is visible; script-breaking characters are escaped; every external HTTP(S) image uses the bounded proxy. Source reread satisfactory; browser execution pending. | Malformed/unavailable old storage cannot kill app bootstrap or silently lose changes. Literal hostile title survives reader metadata parsing without added DOM/script. Supported inline external images use bounded same-origin proxy and display under enforced CSP. |
| AUD-12 P2 | New account import paginates live sessions without pinning selected account or bounded incremental accumulation; no-Web-Locks fallback reads state before later tx and can lose concurrent completed=true progress. | Local-library/frontend. Account preview/apply freezes account identity and bounds incremental records. Device series/progress/member state now reads and writes inside one IndexedDB records+meta transaction (`library.ts:60-104`), including without Web Locks. Source reread satisfactory; concurrent-account/multi-tab execution pending. | Account change during preview/import aborts without mixed records. Over-budget imports stop before unbounded accumulation. Multi-tab progress/history invariants hold or saving fails visibly when required locking is unavailable. |
| AUD-13 P2 verification harness | `scripts/run-auth-tests.sh:26,35` creates private loopback TCP, but `backend/tests/library_http.rs:94-98` still rejects every transport except Unix. | Catalogue corrected `library_http.rs:85-111` to exact 127.0.0.1 or owned fixture socket plus reserved DB and owned fixture receipt. Source reread satisfactory; no suite was executed. | Suite accepts only exact owned fixture runtime/reserved DB and permitted loopback/owned socket, rejects other targets, reaches real Router/DB checks and produces stopped-process receipt. |


| AUD-14 P2 | Original `UploadChapterPages.svelte:17-34,255-278,329-360` decompresses whole ZIP entries before size checks, decodes all images concurrently and recursively collects folders without aggregate admission. | Frontend mounted streamed output, sequential image decoding, image header/pixel admission, 500-page/500MiB aggregate limits, bounded folder traversal and object-URL cleanup in `frontend-overlays.mjs:623-653` and two upload fragments. Bounded EOCD/central-directory preflight now precedes JSZip, requires exact directory end and rejects later ambiguous markers, unsupported ZIP64/split-disk/trailing metadata. Actual installed JSZip parser source informed this consistency guard; no exploit execution is claimed. Source reread satisfactory; adversarial/ordinary browser execution pending. | Oversized compressed ZIP, excessive central-directory entries, highly compressed oversized image, excessive folder depth/count/bytes and malformed image fail visibly before unbounded allocation. Normal CBZ/folder uploads still produce genuine ordered images and release controls after errors. |
| AUD-15 P2 verification harness | Earlier browser acceptance ran startup deep links only and threw on every nonstartup phase. Source now has real form/image/PWA journey code in `scripts/browser-journeys.mjs`; remaining editorial/community/local-first/original-signer journeys and system checks are still pending. | Parent/test owner. Harness source is a preparation deliverable, never a pass. **Open testing gate after implementation freeze.** | Actual UI controls plus real Router/DB/original signed operations establish account, library, upload/recovery, reader, managed-group/moderation, public signed profile/social, installed-worker and shutdown journeys with owned fixture/process receipts. No fake success responses or screenshot counts substitute for outcomes. |
| AUD-16 P1 | Initial capability response omitted an automatically CSP-allowed original origin and reported publication available without browser CORS access; original routes alone were not usable cross-origin callers. | Bridge corrected effective exact approved connect origins and reports original native CORS/same-origin availability in `protocol/adapters.rs:38-72`. Source reread satisfactory; actual browser preflight/admission/signing pending. | Correct configured public original origin is allowed by the unchanged strict CSP and original CORS; unapproved origin fails visibly before signing. Browser-selected signer is distinct from password account/operator identity, with no secret exposed. |
| AUD-17 P1 | New native title Publish caller is real (`catalog/mutations.rs:601-603`), but `publication.rs:780-903` regenerates input from mutable title/selected cover/referenced creator names. Durable command reserves exact manifest hash and submitted version. `require_manga_edit` still allows edits after a prepared/source-accepted failure, poisoning immutable retry and source recovery. | Migration 0074 now snapshots canonical manifest and pins native originals; current native title metadata/relations/cover/lock/reject/delete writers share prepared-state parent fencing. Materialized retry now reads a freshly authorized real signed source event/receipt without replaying cleared original pins. Current browser/native-review admission plus actual bound original publisher authority permits only a minimal pending receipt. Mounted read-only editor boundary reloads it and refreshes current public DTO first when projection completes. Source reread satisfactory; real-cover disconnect/restart/revocation execution pending. | Lost response/restart/prepared failure retains the exact approved signed title input. Later local metadata/cover/creator edits cannot alter or strand the approved replay. Retry with changed input is visibly rejected; identical approved recovery materializes once, preserving native alias and submitted authority/version. |


| AUD-18 P2 | First signed public profile/social adapter lost exact event identity when POST succeeded but its response was lost; UI cleared the review before awaiting transport. A second comment attempt could create a new signed duplicate. | Local-library now saves bounded exact body/recipient intent in original IndexedDB meta before first delivery, per-event Web Locks, immutable same-body retry with fresh original NIP98 request proof, cancellation, signed receipts and current public counterpart reconciliation (`protocol-identity.ts:94-204`). UI-admitted template/read budgets are aligned. Source reread satisfactory; lost-response/reload/cancel execution pending. | Accept a comment but drop its response, reload, verify outcome-unknown retained exact event/recipient, reconcile or explicitly retry the same signed body and prove one stored comment. Changed signer/recipient must fail; cancellation cannot pretend to recall accepted copies. |

P1 denotes a known feature-completeness/authorization/privacy gate; P2 denotes a
known robustness/lifecycle or verification gap. A source fix is not a test pass.

## Actual original seams and identity boundary

The original crate is a path dependency, not a rewritten protocol implementation.
The inspected original interfaces include:

- `server/src/library_manifest.rs`: Chapters/Series/Lists/Members record families
  with canonical 32-byte identifiers and signed kind-11999 pointer.
- `library_client_bundle.rs`: complete manifest graph validation and explicit
  owner write-relay kind-10002 routing.
- `library_client.rs`, client outbox/recovery/discovery: signer interface, bounded
  explicit recipient delivery, first acceptance versus completion, per-recipient
  Accepted/Superseded/Rejected/Retryable states.
- `manga-protocol-lab/browser-client/src/store.js:7-75`, `publishing.js:34-117`:
  durable IndexedDB/local records, worker snapshot, explicit public consent,
  external signer, owner consistency, exact graph and reviewed recipients.
- Original `accounts.rs`, `relay.rs`, `db.rs`: current account permit plus signed
  profiles/contacts/relay lists, notes/reactions/comments and deletions, rather
  than a server-signed impersonation of password accounts.

Native catalogue IDs, compatibility aliases and canonical signed event IDs are
separate domains. The new adapter must publish only current visible canonical
mapping, preserving explicit origin/catalogue-namespace/service binding. Backups of the same original service on two distinct native catalogues are not interchangeable; native alias namespaces must be preserved on restore. Native-only editorial
objects remain device/account state. An instance password account, operator
publisher signer and external user signer confer different authorities.

Source media currently checks event, parent and exact descriptor on every read;
original bytes come from the user's owned original store or explicit self-hosted
same-origin original endpoint. Imported-peer metadata is not a reader-CDN grant.
Deletion is instance removal plus an actual signed request for original review;
a pending request is never claimed as approved physical/source deletion.

## Finite review coverage and known limits

Source traced: catalogue filters/DTOs/editorial mutation and version locks;
managed membership/claims/report review; persisted community/reply/vote/report
operations; native authentication and mail/recovery/client lifecycle; account
follows/history/lists/progress/preferences/import/export; upload staging,
publication preparation/recovery/cleanup; actual original event/media projections;
CSP/proxy/preview/ETag policy; service-worker authority; listener startup/readiness,
admission/body bounds/drain and original worker ownership; release/config/proxy
entrypoints and fixture target safety.

The retained 676 input files, 51 page routes, 475 anchors and 107 operations are
useful inventories. They do not establish working user paths. The reconstructed
missing announcement pages are additive recovery, not recovered original bytes.
Imported chapter and cover derivative source is now present; frozen source is reported by the bridge owner, while real browser acceptance remains required. The frontend owner reports final157 anchored repairs across54 paths and preservation of all676 originals, with49 composed/external Svelte components compiling. These are composition/preservation checks, not runtime evidence.

Still unproved: all browser-visible controls and mounted adapter behavior,
installed-worker revocation, cross-tab storage and quota failures, source-authority
races, accepted-write disconnect/restart/recovery, exact page/range/HEAD/ETag
behavior, real original account/group/relay admission, native listener/worker
failure and graceful shutdown, process cleanup, operator TLS/mail/DNS/secrets,
backup restore and deployment. Production operation cannot be signed off from
this source review. Previously recorded tests/builds do not cover these new bytes.

## Concrete codebase-design cleanup

Reviewed official source:
[Matt Pocock codebase-design](https://github.com/mattpocock/skills/tree/main/skills/engineering/codebase-design),
including its deepening and design-it-twice guidance.

Apply the small-interface/deep-behavior and deletion/locality principles to real
seams, avoiding speculative architecture:

1. Keep one explicit query encoder, replacing reliance on the global serializer
   quirk at the new adapter boundary. Tests and production callers cross it.
2. Keep one browser-versus-scoped-library authorization interface with the
   invariant documented at the principal boundary. Do not sprinkle independent
   role checks that accidentally widen bearer authority.
3. Let upload resume own one draft/order/version/immutable-publication contract;
   join file removal/sort/autosave/commit under its actual mutation boundary.
   Delete superseded automatic-session-reset and ignored-DELETE behavior.
4. Keep device/native namespace and original canonical namespace explicit with
   original storage/signing/outbox behavior reused. Do not create a second fake
   protocol implementation or signer hidden behind password Session.
5. Treat caches as authority contracts, not incidental speed hooks. One
   first-registered sensitive-network policy governs every cache candidate;
   remove historical unsafe entries before performance work.
6. Prefer tests of returned observable state/authority/receipts through the same
   Router/store/worker interface callers use. No success stubs, redundant shallow
   route-count tests, or private helper-only assertions as end-to-end proof.
7. Keep one authoritative replacement-body source for overlay fragments; remove or clearly generate duplicated inline/review copies so an edit cannot silently affect only unmounted code.
8. Bound and own live process/task/body resources. Retain existing original
   download/worker abstractions rather than inventing a parallel storage owner.

Reconsider broader refactoring only after functional and system checks identify
an actual repeated complexity/cost. Performance measurements and hill climbing
are last, against frozen correctness constraints and preserved UI/source hashes.

## Final reviewed seam hashes

These are SHA-256 pins, not test receipts:

| Source | SHA-256 |
| --- | --- |
| `backend/src/domains/auth/principal.rs` | `ba40b0fc95533e7cac7dbef4ba36d4884cf3284859b7b073763e48c93bbac95e` |
| `backend/tests/auth_http.rs` | `7838732e2b42cf65e6abd3ffcc0726eca3a94c21a012d0346b976394206d80e9` |
| `backend/src/domains/catalog/read.rs` | `e4751095635b5f03f744a5de1bb5ec2a484a2562170a1c952bc60cc60839ccf3` |
| `backend/src/protocol/publication.rs` | `b434baddc3ba358a4f72cb5c650f52d94e8e9433e849fde8628855f9f882d034` |
| `platform-integrations/frontend-overlays.mjs` | `c0eb6e3fd988d922c0ce6e60e4c43cdd0a80a324c1d09279900387b76d1a194a` |
| `platform-integrations/frontend-behavior/MangaEditBoundary.svelte` | `e246f4ad220de1c1b67c87006e208746bf0330569ce4ff82084746eb05db4ad5` |
| `platform-integrations/local-library/device-store.ts` | `8b563825b8e8b74beec871dbf9c7fec6ca6a0b28fb3d62c04ace9a57ced7e227` |
| `platform-integrations/local-library/library.ts` | `789b894cd493279daa7fe0c8e6a9cf4ea4359e9340ba45d475bedf4221b30316` |
| `platform-integrations/local-library/publication.ts` | `754c99ba955622a2ced7c7d3cfe0c276e83201ca565b95fd5fa87edeb8ca512e` |
| `platform-integrations/local-library/overlays.mjs` | `16605a09363ac334ec08ea756374316171da582412c7a0fe8550a2d540a2dac9` |

## Signoff boundary

The finite source-wiring signoff is issued for the scope above: owners reported
freeze, final source rereads resolved every known implementation finding,
self-authored authority/test changes received independent catalogue review, and
capability docs were reconciled with mounted behavior. The user additionally
requested a fresh independent reviewer squad after each substantive patch set;
this artifact is a handoff to that next review, not a reason to skip it. Fresh
compile/build, end-to-end/system testing and deployment outcomes remain separate
gates. No runtime-verified control or production-readiness claim is made.
