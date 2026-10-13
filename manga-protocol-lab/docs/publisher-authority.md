# Independent publisher authority (schema 36)

This implements audit B1/R1: the identity allowed to upload and publish is distinct from the instance's physical storage owner. Subsequent schemas37–40 add abandoned staging cleanup and reference-API direct deletion; see staging-reclamation.md and staged-deletion.md. Standard Blossom routing remains separate.

## Local policy and API

An operator uses signed NIP-98 POST `/admin/publishers/{pubkey}` with `{"allowed":true}` or false. This is a separate permission from `/admin/accounts/{pubkey}`. Account, group and peer admission do not implicitly create independent publishing rights. The physical administrator's existing permission is initialized by instance binding.

An admitted publisher signs an upload token for PUT `/upload/{hash}`. The existing body/hash/size/slot limits and original filesystem receipt path still apply. The node records a generation-specific staging grant for the authenticated publisher only after complete verified storage and a second permission-epoch check. Shared exact bytes remain one physical object; knowing a hash alone does not create a staging grant. The publisher must upload the bytes to obtain its grant.

POST `/publications` accepts the author's signed event with body-bound NIP-98 authorization from the same key. The operation retains the permission epoch obtained before receiving the body. Group publication and approved-peer import retain their own existing authority paths. NIP-42 authenticated WebSocket EVENT uses the same domain publication preparation and commit checks.

GET `/blobs/{hash}` remains an authenticated staging route. Non-admin publishers require a current grant. Public hosted originals use `/originals/{hash}` under instance read policy.

## Safety boundaries

- The storage owner, volume and generation remain authoritative for physical mutation. No user key is substituted for that owner.
- Publisher authorization is rechecked after upload I/O and at publication commit. Revocation followed by re-admission does not revive an old permit or old grant.
- The manifest and every unique original need a current publisher grant before non-admin publication. Final commit rechecks grants against current generations under the existing blob locks.
- Grants use the existing five-minute staging lease. Re-uploading identical verified bytes renews that publisher's grant. This is an implementation staging policy, not a new protocol-wide retention requirement.
- Active grants inhibit existing retained-object cleanup. An upload cancelled/revoked before grant commit can leave only staged bytes; audit B2 remains open until abandoned staging is collected.
- Since schema40, ordinary administrator uploads also receive explicit staging grants so another uploader cannot erase their shared staged interest. The physical administrator retains its legacy local ownership path for publication and still needs a current permission epoch.
- No HTTP API returns or receives private publishing keys. Publishers sign locally; synthetic tests use synthetic fixture keys only.

## Verification

`server/tests/http_publishers.rs` covers real HTTP/strfry publication, authenticated WebSocket publication, distinct account/publisher permission, operator-only policy changes, request-author binding, staged read and publication isolation, physical deduplication, unchanged storage ownership, permission revocation/re-admission, expired grants at final commit and exact-byte renewal. Focused acceptance passes in `results/audit-publisher-regression-2.log`; structured observations are in `results/publisher-authority.json`.

The combined regression passes in `results/audit-publisher-full-regression.log`, including the existing group/peer/publication/storage/serving fixtures, Clippy, separate-process behavior and PostgreSQL immediate-stop/restart checks. This is not a claim of complete backend coverage or deployment capacity. The schema migration is additive and checksum-versioned; the old migrations are unchanged.

## Durable duplicate acknowledgment

A currently authorized publisher may retry the exact already-hosted signed event even after its temporary upload grants have expired or were invalidated by an earlier permission epoch. Preparation records only the existing event generation; commit rechecks current authority, local review verdict, generation and hosted lifecycle before its idempotent branch. This adds no new hosted transition, storage charge or private attribution. It does not revive old grants for a different event or resurrect a locally removed publication. Acknowledgment is not a fresh physical-integrity check. The native publisher fixture explicitly separates duplicate acknowledgment from fresh publication admission.
