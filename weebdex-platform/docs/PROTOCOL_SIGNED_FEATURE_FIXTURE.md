# Synthetic signed feature fixture

The disposable browser supervisor has a separate `--signed-feature-controls`
opt-in. It requires the combined protocol provisioner and generated synthetic
images, and does not enable or broaden public upload controls. The only admitted
browser command is the exact `public-signed-feature-recovery` phase, the matching
`BROWSER_PHASE`, and `scripts/browser-acceptance.mjs`, with a 15–300 second deadline.

Two loopback-only control endpoints accept exactly `{ "pubkey": "<64 lowercase hex>" }`:

- `POST /fixture/protocol/signed-feature-account`
- `POST /fixture/protocol/signed-feature-snapshot`

The same actions can use the existing private fixture-control transport. The
supervisor enforces its owned fixture lifetime, active phase, drain/rebuild state,
24-action maximum, 20-second child timeout, and stopped-child receipts. No user,
draft, report, reason, event, permission, secret, cookie, or chapter target may be
supplied. The provisioner additionally requires the reserved disposable database,
owned private root, synthetic-image marker, and signed-feature opt-in.

## Actual account and fixtures

The account action binds one fresh browser public key to fixed synthetic native
user `SgFeatUsr1`, verified with role `user` and exactly `report.create`,
`manga.create`, and `self.chapter.update`. It creates no native session, usable
password, group membership, operator binding, or publisher grant. A second
unbound synthetic user owns the foreign draft. Both draft titles start in `draft`.
A fixed enabled `manga` report reason requires details and has a positive version.
Only `uploader_id` on the first canonical seed chapter is associated with the
fixture actor. Canonical event, manifest, original page bytes, and upload grants
are not mutated.

The account response contains `fixture_only: true`,
`scenario: public-signed-features-v1`, `pubkey`, public/native identity pairs under
`user`, `draft`, `foreign_draft`, `manga`, and `chapter`, plus
`reason: { id, native_id, version, text }`, `catalog_namespace`, and `service_key`.
`chapter` additionally includes `initial_title` and `initial_version`.
Authority evidence includes `allowed: true`, `roles`, `native_permissions`,
`no_native_sessions: true`, and false flags `browser_private_key_received`,
`secret_received`, `browser_operator_binding`, and
`browser_publisher_admission_granted`.

The only submitted report body admitted to observation is
`Synthetic signed feature report details.`; the permitted edited chapter title is
`Synthetic signed feature saved chapter`. These strings are returned as
`report_body` and `chapter_saved_title`. Unexpected persisted text fails closed
rather than being copied to a receipt.

## Persisted observations

The snapshot requires the identical public key and rechecks actual native role,
permissions, account admission, no sessions, and no group membership. It returns:

- `report: { count, records }`, excluding removal-review requests, and
  `total_report_count`, including those requests.
- `draft: { id, native_id, state, version, deleted, owner_id, removal_request }`,
  where `removal_request` has `count` and `records`.
- `foreign_draft: { id, native_id, state, version, deleted, owner_id }`.
- `chapter: { id, native_id, title, version, uploader_id, deleted, is_unavailable }`.
- `original: { source_event_id, source_event_sha256, source_version,
  manifest_sha256, pages: [{ sha256, bytes }], fresh_original_reads, unchanged }`.

Report records are `{ id, native_id, submitter_id, entity_type, entity_id,
reason_id, body, status, target_version }`. Every reference except `native_id`
is a public alias. `entity_type` is `manga`; `status` must be `waiting`;
`target_version` is an integer or null. At most one exact normal report and one
exact owner-removal request are admitted. The removal body is the backend's fixed
`Authorized current owner requests instance title removal; no original deletion requested`.

The account response's `original` object is the baseline without `unchanged`.
Each observation independently queries the persisted signed source event and
reads its actual original-serving manifest and page bytes, bounded to 12 seconds,
12 pages, 4 MiB per page, and 16 MiB total. It never opens a second storage owner.
The original source signature, original identity, stored seed event, hosted state,
and served byte hashes must agree. The snapshot compares the freshly observed
fingerprints with the baseline, so a local chapter title save cannot be promoted
to success merely because a cached seed file remained unchanged.

## Finite tests

`python3 tests/ci/test_signed_feature_fixture.py` tests pure admission and actual
supervisor method guards without starting a browser or database. The provisioner
example contains `signed_features::tests` for the Rust opt-in, closed request,
reserved identities, and exact permissions. These tests and compilation alone do
not establish the real browser journeys or SQL outcomes; those require the
separate owned acceptance run.
