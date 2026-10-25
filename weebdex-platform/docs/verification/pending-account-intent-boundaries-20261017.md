# Pending captured-account intent audit

Source pin: `e47b33aeb371dc08f4486ca86217e52cde7e1227`. This is a source-only matrix. No new compile or runtime probe has executed. Shared disk headroom is constrained, and further heavy work is held until the resource boundary is resolved. Production sources remain unchanged.

## Scope and existing proof

All listed routes already require a genuine exact NIP-98 external URL, method, service, nonce and raw-body proof. `auth::bound_account_write` rechecks the original admission capability and locks the current explicit binding and active native account inside the mutation transaction. Native permissions, visible typed targets and current ownership/group authority still apply. The concern below is **captured native-account intent**, not an authentication bypass: a previously reviewed account A can become a different independently eligible account B while the selected signing pubkey stays unchanged.

The corrected title/editorial helper `editorial::expected_account` resolves an optional typed native User UUID and requires equality with that final transaction's bound actor. Its dedicated clients always send the actual captured account UUID. Cover's separately qualified successor requires that field and also compares the upload's early/final actor. This follow-on will reuse the shared helper, without changing `auth.rs`, adding migrations or promoting a public resource into native authority.

| Affected persistent route | Current signed request | Existing mutation guard | Captured-account precondition at this source |
| --- | --- | --- | --- |
| PUT `/api/v1/chapter/{uuid}` | JSON version and partial metadata/group UUIDs | Native chapter CAS, owner/group/staff authority, source/parent visibility | None; current binding determines audit actor |
| POST `/api/v1/chapter/{uuid}/claim` | Empty body | Current native group-leader authority and unclaimed eligibility under chapter lock | None; current binding becomes uploader |
| DELETE `/api/v1/chapter/{uuid}` | Empty body | Native removal authority; explicit moderator hide or durable local review, then separate original-source guard | None; local report actor and later source guard must each preserve captured intent |
| POST `/api/v1/upload/begin` | JSON manga/group UUIDs | `chapter.create`, visible published title, unique open session per owner | None; current binding becomes session owner |
| POST `/api/v1/upload/begin/{chapterUuid}` | JSON chapter version | Native edit authority/version and unique open session per owner | None; current binding becomes edit-session owner |
| POST `/api/v1/report` | JSON category/reason/object UUIDs/details | `report.create`, actual reason, typed visible native object, private report/audit core | None; current binding becomes report submitter |

The nonpersistent `/upload/check-approval-required` decision is outside the first repair batch. Existing upload-session mutations keep their persisted `session.owner_id` checks, upload/version/source invariants and pending-command fences. A direct attempt to apply account B to a remembered account-A session is already refused by ownership; that guard must not be weakened.

### Discovery sequence that also needs qualification

`upload-client.ts` captures the displayed native account when loading signed account context. Its later transport refresh verifies the signer generation and service/catalogue scope, but does not compare the server's current native binding. A fresh GET `/upload` returns a session resource without a native-account identifier. The client can consequently discover a now-bound B session while still presenting captured A, then pass the server's correct B-session ownership check. This source sequence needs an actual disposable HTTP repro before selecting the smallest correction. It is not yet a runtime finding or a reason to remove session-owner fences.

## Proposed bounded repro

`test-evidence/public-api/account-intent-audit-20261017/uncompiled-baseline-probe.patch` is an **uncompiled diagnostic**, not a production patch or passing test. It reuses the existing physical PostgreSQL/NIP-98/strfry fixture and its normal owned guard. It prepares two active native accounts with colliding grants and two separate genuine leader memberships, so both are independently eligible without creating an impossible double-leader group.

The probe captures actual account-A context and exact legacy first-party request bytes/proof, rebinds that same pubkey to eligible B, sends each of the six requests, and checks the actual native actor in the resulting row/audit. It separately exercises current-session discovery followed by cancellation. Any observation, refusal or fixture error must be recorded as returned; no result is claimed yet. Once verified, only confirmed drift gets a request-bound account precondition plus a focused deny/same-account regression. Existing compatibility callers and native cookie contracts require an explicit documented decision, not accidental schema tightening.

The later compile must remain offline, use existing verified dependencies, one Cargo job and the owned bounded runner. Runtime/build must stay above the shared 4 GiB floor. A missing dependency, insufficient headroom or denied action pauses that exact operation. No browser or external publication is part of this audit.

## Qualification record

- Nine source files were byte-checked against the pinned commit and hashed in `source-pins.json`.
- The live backend test was restored to its exact pinned bytes after archiving the diagnostic patch.
- No Rust compile, database fixture, listener, browser or publication was launched for this batch.
- The earlier cover/title/editorial independent HTTP runs are separately completed and source-pinned; their passing counts are not reused as proof of this pending account-intent batch.

## 20:06 UTC baseline diagnostic executed

After the parent released the approved resource hold, the preserved diagnostic compiled offline with one Cargo job and no incremental cache copies. The live4GiB floor guard remained satisfied and its owned process group stopped cleanly. The exact executable SHA-256 is `819d0fc5d44593902eddb1bec5d63f84f0e99b44bd1c63c610279191e2275d0b`.

One fresh actual PostgreSQL/NIP-98/strfry fixture observed all seven sequences executing as eligible account B after exact legacy request bytes/proof were captured under actual account-A context. The six persistent routes returned their normal success statuses; fresh discovery then cancelled B's session while the captured label was A. Native rows/audit verified the actual actor, not just HTTP status. The diagnostic Rust case intentionally verifies the observed legacy behavior and completed normally; the independent captured-account acceptance invariant **fails7/7**. This is a reproduced defect, not a passing feature test.

All420 source inputs were unchanged during the run. Guard `auth-50th3ez1` verified PostgreSQL stopped, port closed, zero owned survivors, unknown births or cleanup errors. The exact source, compile/HTTP raw logs, executable pin, invariant failure result and cleanup receipt are preserved in `test-evidence/public-api/account-intent-baseline-20261017/`. No source repair is claimed at this checkpoint.
