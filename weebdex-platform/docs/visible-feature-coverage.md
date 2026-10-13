# Visible feature coverage and implementation gap register

Original feature-anchor inventory updated 2026-10-13T04:34:13.171826+00:00; mounted-source currentness updated 2026-10-13T09:46:13.322Z. Source implementation audit; **no tests executed by this auditor**. A handler, API wrapper, compile or build is not a complete visible-workflow pass. Final API/database/browser/security checks remain distinct gates.

The retained source has676 original files,51 unique page paths and475 non-generic action/binding/link anchors. The107 observed operations have94 actual wrapper or direct callers;13 are API-source-only. The current frozen source composition has534 exact SHA/count-anchored definitions (543 occurrences) across106 original paths. Build19/source534 passed; Build18/source533 and Build17/source516 are historical; finite repairs are documented in docs/round7-8-frontend-repairs.md and docs/round9-frontend-repairs.md and docs/round11-frontend-source-repairs.md and docs/round12-frontend-source-repair.md, docs/round13-frontend-source-repairs.md, docs/round14-frontend-source-repairs.md docs/round15-frontend-source-repairs.md and docs/round16-frontend-source-repairs.md. Current exact integration/external-component inventory is test-evidence/frontend/integration-source-manifest.json; historical35-repair evidence remains an earlier audit snapshot, not the final source. Original attachment files are never written. Exactly one explicitly approved GroupClaimDialog onclick JavaScript body exception retains the same DOM/layout/classes/styles.

## Auditable evidence

- control-anchors.json: every observed anchor has exact original path/line, family and user-visible acceptance; all runtime_verified values remain false until final evidence.
- api-ui-call-coverage.json: all107 operations plus actual caller paths/lines; direct multipart and export href are included, avoiding an API-wrapper-only false negative.
- script-repair-evidence.json: historical35 repairs; current534 repair definitions are recorded in integration-source-manifest.json. The historical entries include original SHA, exact source line and anchor count. Full before/after script definitions are in platform-integrations/frontend-overlays.mjs and docs/frontend-overlays.md.
- workflow-gap-register.json: distinct backend, source-publication, missing route, configuration and remaining integration gaps with exact evidence and acceptance.
- retained-route-evidence.json, query-key-evidence.json, static-link-evidence.json: route/query/link traces for final browser matrix.

## Control families and acceptance

### auth (24 observed anchors)
Complete browser login/register/logout/verification/recovery/account update using real persisted session. Hydrate only authoritative account preference scope/version; preserve local library. Invalid/stale/expired flows show useful errors; credential changes and API-client actions enforce fresh owner authority.

### catalog (101 observed anchors)
Search title/alternate/author/artist/year/original language/demographic/status/content rating/included/excluded tag AND/OR plus chapter language/uploader/group filters and every sort/order/page/limit/view. Default discovery hides unselected adult rating; explicit no ratings is empty. Create/edit draft/title/author/group/cover/relation workflows validate versions and permissions.

### community (11 observed anchors)
Read/paginate/deep-link discussions; submit/reply/quote/edit/remove/vote with real persisted versions and counters. Empty editor formatting is safe. Embedded previews use actual local eligible entities; no fake success, hidden-source leakage or external resource fetch.

### library (11 observed anchors)
Follow/status/bell/unfollow, library filters/views/pagination, updates/feed/history/read/unread, import/export default to a scoped browser-local device library without an account; optional explicit account mode/import preserves independent device state. Custom local lists, page progress, scoped/private backup review and explicit signed-publication/recovery panels are additive real UI extensions. Canonical records use the original protocol modules; native IDs require current verified mappings before any public signing. Failed import does not claim success. Automatic excluded tags do not remove followed titles from updates. No browser library is overwritten or auto-published at login.

### moderation (28 observed anchors)
Reasons/report submit/status/list/category/object/reason/status/order filters work with owner/staff permission. Group claim submission records review, not ownership; deliberate authorized approval grants membership atomically. Claim-code clipboard reports actual write result.

### navigation (168 observed anchors)
Activate each retained route/link and UI-only disclosure/search/theme/sidebar/view/tab control. Verify correct state, back/forward and fresh deep links. External links remain deliberate user navigation; tracking is blocked by deployment policy. A source anchor alone is not browser proof.

### preferences (11 observed anchors)
Save/reset all visible language/rating/tag/uploader/group/items preferences and optimistic version. Four unchecked ratings produce sole __none__, yielding empty public discovery; default selections/reset keep normal defaults. Anonymous preferences remain local. Reader quality uses actual available capability.

### reader (86 observed anchors)
Open actual original pages, navigate first/last/previous/next and chapter selection in natural order. Exercise paged/scroll/layout/direction/double/offset/width/gap/preload/parallel and input switches. Real failures expose Retry; cancel/retry/quality/exit release requests, permits and URLs. Optimized is enabled only for a complete actual smaller derivative set; animation retains original timing.

### upload (35 observed anchors)
No current session returns204, existing owned session returns a full owner-only versioned resume DTO with saved draft/order/files and current authority/can-mutate/can-cancel/retry flags. Begin/resume/cancel/add archive/images/folder/reorder/remove/replace/commit exact pages. Reject invalid media visibly without stuck mutex. Refresh committed versions. Signed source status and pending processing are truthful; identical commit retries preserve event/receipt/page positions.

## Concrete repairs and remaining gates

### BE-001: Upload entry was unreachable when no session returned200{}
Status: implemented source. Evidence: `weebdex-platform/backend/src/domains/uploads/sessions.rs:171`.
Acceptance: No current session returns204, existing owned session returns a full owner-only versioned resume DTO with saved draft/order/files and current authority/can-mutate/can-cancel/retry flags. Begin/resume/cancel/add archive/images/folder/reorder/remove/replace/commit exact pages. Reject invalid media visibly without stuck mutex. Refresh committed versions. Signed source status and pending processing are truthful; identical commit retries preserve event/receipt/page positions.

### BE-002: Optimized quality previously had no real pipeline
Status: implemented source; bridge media integration separately owned. Evidence: `weebdex-platform/backend/src/domains/uploads/derivatives.rs:7`.
Acceptance: Open actual original pages, navigate first/last/previous/next and chapter selection in natural order. Exercise paged/scroll/layout/direction/double/offset/width/gap/preload/parallel and input switches. Real failures expose Retry; cancel/retry/quality/exit release requests, permits and URLs. Optimized is enabled only for a complete actual smaller derivative set; animation retains original timing.

### BE-003: Derivative metadata must never advertise partial or duplicated originals
Status: implemented catalog-owned complete-set writer. Evidence: `weebdex-platform/backend/src/domains/catalog/derivatives.rs:8`.
Acceptance: Open actual original pages, navigate first/last/previous/next and chapter selection in natural order. Exercise paged/scroll/layout/direction/double/offset/width/gap/preload/parallel and input switches. Real failures expose Retry; cancel/retry/quality/exit release requests, permits and URLs. Optimized is enabled only for a complete actual smaller derivative set; animation retains original timing.

### BE-004: Forum entity OG paths previously had placeholder/unhandled image
Status: implemented source with local SVG rendering and fresh canonical visibility. Evidence: `weebdex-platform/backend/src/domains/previews/mod.rs:19`.
Acceptance: Read/paginate/deep-link discussions; submit/reply/quote/edit/remove/vote with real persisted versions and counters. Empty editor formatting is safe. Embedded previews use actual local eligible entities; no fake success, hidden-source leakage or external resource fetch.

### BE-005: Native upload previously stopped at local catalog rather than configured source publication
Status: implemented actual original publication hook; authenticated pending receipt durable. Evidence: `weebdex-platform/backend/src/domains/uploads/sessions.rs:513`.
Acceptance: No current session returns204, existing owned session returns a full owner-only versioned resume DTO with saved draft/order/files and current authority/can-mutate/can-cancel/retry flags. Begin/resume/cancel/add archive/images/folder/reorder/remove/replace/commit exact pages. Reject invalid media visibly without stuck mutex. Refresh committed versions. Signed source status and pending processing are truthful; identical commit retries preserve event/receipt/page positions.

### BE-006: Canonical original hashes and repeated identical pages conflicted with native name primary key
Status: implemented migration23 canonical names plus position key. Evidence: `weebdex-platform/backend/migrations/0023_chapter_derivatives.sql:11`.
Acceptance: No current session returns204, existing owned session returns a full owner-only versioned resume DTO with saved draft/order/files and current authority/can-mutate/can-cancel/retry flags. Begin/resume/cancel/add archive/images/folder/reorder/remove/replace/commit exact pages. Reject invalid media visibly without stuck mutex. Refresh committed versions. Signed source status and pending processing are truthful; identical commit retries preserve event/receipt/page positions.

### CAT-001: Absent discovery ratings admitted adult results despite retained default preferences
Status: catalog owner corrected source defaults and explicit __none__ sentinel. Evidence: `weebdex-platform/backend/src/domains/catalog/query.rs:71`.
Acceptance: Search title/alternate/author/artist/year/original language/demographic/status/content rating/included/excluded tag AND/OR plus chapter language/uploader/group filters and every sort/order/page/limit/view. Default discovery hides unselected adult rating; explicit no ratings is empty. Create/edit draft/title/author/group/cover/relation workflows validate versions and permissions.

### CAT-002: Reader aggregate direction reversed first/last/next/previous and Start Read
Status: catalog owner corrected descending natural order. Evidence: `weebdex-platform/frontend/src/lib/components/reader/context.svelte.ts:27`.
Acceptance: Open actual original pages, navigate first/last/previous/next and chapter selection in natural order. Exercise paged/scroll/layout/direction/double/offset/width/gap/preload/parallel and input switches. Real failures expose Retry; cancel/retry/quality/exit release requests, permits and URLs. Optimized is enabled only for a complete actual smaller derivative set; animation retains original timing.

### ADD-001: Missing homepage announcement components and linked route
Status: frontend owner separately reconstructed real additive announcement routes/components. Evidence: `weebdex-platform/docs/announcements-reconstruction.md:7`.
Acceptance: Load pinned public home items, public feed/detail, authorized manage/draft/edit/publish/pin/delete with versions; anonymous callers never see drafts. Empty real data is allowed; missing components were reconstructed additively.

### OPEN-001: Imported protocol covers require actual256/512 reader thumbnails
Status: implemented source; imported cover bytes, materialization and revocation runtime suites remain unrun. Evidence: `weebdex-platform/backend/src/protocol/media.rs:31`.
Acceptance: Import a verified locally hosted series cover and load original,256 and512 exact supplied cover URLs; dimensions, MIME and smaller bounded bytes must match, source revocation fails closed before conditional delivery.

### OPEN-002: Native protocol-linked reader derivatives require authorized instance media routing
Status: implemented source; original/public optimized routing, current-authority and revocation runtime suites remain unrun. Evidence: `weebdex-platform/backend/src/domains/uploads/media.rs:26`.
Acceptance: Upload via configured signed runtime, select actual optimized complete pages, confirm allowed local associations plus current original chapter/series authority; revocation prevents fresh200/304, no unrestricted source/raw storage fallback.

### GATE-001: Fresh deep-link query startup ordering requires actual browser evidence
Status: not proven by static source. Evidence: `weebdex-platform/frontend/src/routes/+layout.ts:10`.
Acceptance: Open fresh title/random and search deep links under final same-origin build; verify each URL has normal route?query syntax after root initialization. No speculative server URI rewrite.

### CONFIG-001: Verification/recovery delivery needs actual operator-provided trusted email transport
Status: implementation capability exists; disabled transport truthfully unavailable. Evidence: `weebdex-platform/backend/src/domains/auth/README.md:21`.
Acceptance: With isolated reserved-domain sink verify delivery+one-time redemption; disabled transport rejects usefully without fake verification. Production credential provisioning remains deliberate operator action.

## API source without an observed corresponding control

These are implemented service capabilities, not proof of a complete visible feature. No new buttons are invented merely to increase API coverage. The original static call scan missed dynamic method selection: Group.svelte has real Follow/Unfollow controls. Client.Update has no rename/scope editor; only creation/rotation/deletion/copy are promised. User/chapter/group statistics APIs have no caller. MangaEdit.svelte has real Lock/Unlock controls through dynamic API selection. Cover.Get is not a direct UI wrapper caller, although cover metadata and actual image URLs are visible.

- Client.Update: `src/lib/API/Client.ts:16`
- Cover.Get: `src/lib/API/Cover.ts:9`
- Group.Follows: `src/lib/API/Group.ts:64`
- Statistics.User: `src/lib/API/Statistics.ts:4`
- Statistics.Chapter: `src/lib/API/Statistics.ts:14`
- Statistics.Group: `src/lib/API/Statistics.ts:19`
- User.Follows: `src/lib/API/User.ts:29`
- User.Follow: `src/lib/API/User.ts:35`
- User.Unfollow: `src/lib/API/User.ts:41`

## Deliberate boundaries

- Optional account follows/history/lists/preferences and forum/report/analytics state remain instance-only. Device library is the anonymous default and independent of password account; public sync is an explicit reviewed signer/recipient operation. No login signs or publishes a Nostr event, overwrites the original local browser library, or generates signer credentials.
- Native new uploads invoke the real original canonical encoder/publication service only when explicitly configured and bound to actual admitted operator/group authority. Accepted-pending processing is distinct from public reader availability. Protocol chapter originals/events remain immutable; optional derivatives are instance capabilities.
- Unsupported animation omits optimized capability, never flattens timing. Any missing derivative makes the whole optimized set absent. Existing local covers generate actual256/512 WebP; imported source cover completeness remains independently tracked.
- Email and source signing/store setup require real operator configuration. Disabled configuration must be a truthful error/capability, not fake successful delivery or publication.
- The supplied original UI had no custom-list/page-progress API controls. The current additive device panel supplies real list membership/backup/protocol controls, and the external reader adapter persists actual current page and completion separately from a visit. Exact supplied source parity is preserved, globally identical rendered HTML is not claimed.
- Original external analytics are blocked by same-origin deployment CSP. Do not send final test traffic to upstream API/auth/analytics services.

## Final end-to-end gate

Use isolated synthetic accounts and owned disposable databases. First apply/compile all frozen implementation changes through the single compiler owner, then test API persistence/authorization and actual unchanged-source browser controls. Cover every inventoried family and reconstructed announcements; include failure, cancellation, stale version, reload, direct deep-link and source revocation paths. No production credentials, private keys, personal data, external sends or runtime databases belong in evidence/checkpoints.

A final completion claim requires receipts for those user-visible acceptances, including OPEN-001/OPEN-002. This audit does not replace that evidence.

## Exact denominator qualification

Two of475 observed anchors are directly tied to the known source-linked media integration gate: reader-settings.svelte:158 (Original) and:170 (Optimized). Imported source covers256/512 are another visible media workflow, outside this action/binding/href denominator. The remaining473 anchors are source-traced with explicit family acceptance; this inventory alone does not establish that each full workflow is individually source-reviewed implemented. Runtime-qualified anchors in this audit:0/475. Do not convert absence of another demonstrated gap into a473-control completion claim.

Round5 owner repair/compile evidence is `test-evidence/frontend/round5-owner-source-repairs/summary.json`; finite independent findings are `docs/round5-ui-source-audit.md`. The four signed browser phases are mounted from the independent source module. This does not change runtime_verified0/475.

Restricted recovery is mounted on existing /auth/recovery through an approved boundary, with byte-identical retained anonymous form. Capability is not a Session. Actual mail/password reset journey source is prepared; runtime remains unrun.
