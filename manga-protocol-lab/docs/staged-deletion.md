# Authenticated staging deletion (schema40)

Status: domain, real HTTP and combined functional regressions pass. This is the reference API, not a claim of complete standard Blossom interoperability.

## Routes and outcomes

- DELETE `/blobs/{hash}` requires a valid hash/action/server-scoped blob authorization signed by an admitted publisher. The body must be empty. A non-admin publisher needs a grant for that exact generation and current permission epoch.
- POST `/groups/{group}/delete-upload/{hash}` takes `{}` and body-bound NIP-98 member authorization. The managed-group remove/admin role is required, rechecked at the final policy transaction.
- Success200 reports `deleted:true` only after physical cleanup and accounting settlement.
-404 means the staging object is absent.403 means authorization/grant failure.409 means retained content or another active interest prevents deletion.429 means the storage work budget is occupied.503 is an unconfirmed outcome; durable cleanup may still be pending, and is never represented as confirmed deletion.

The conservative shared-file policy is to refuse409 without releasing any interest. Clients can retry after the other staging interests expire. This operation does not issue a chapter/series deletion request or bypass moderator approval.

## Shared-file protections

All ordinary uploaders, including the administrator, now receive explicit temporary staging grants. This is necessary to distinguish one user's delete request from another user's recently uploaded copy sharing the same physical hash. Existing unretained owned data receives conservative upgrade grace; the migration does not infer that a shared original is abandoned immediately.

Deletion refuses any retained publication/library object, active other publisher/group/account interest, active peer-transfer hold or undrained external reader. The common blob staging timestamp is shortened only after these checks. Authority, blob generation, owned interest and the durable reclaim marker are committed atomically under the physical hash mutation guard. A late upload cannot add a new grant between authorization and cleanup.

The same receipt-backed reclaim/refund implementation handles explicit deletion and expiry. Disconnecting the HTTP caller does not cancel owned cleanup. A failed unlink retains charges and a durable claim; subsequent cleanup or an authorized retry can complete it without double-refund. Formerly hosted objects belong to their existing moderation/retirement lifecycle, even after their current strong references are gone.

## Evidence

- `results/audit-direct-delete-domain.log`: shared administrator/publisher interests, protected content/transfers, group deletion, unlink failure/retry, request cancellation behind a PostgreSQL barrier and revoked permission epochs.
- `results/audit-direct-delete-http.log` and `results/http-staged-deletion.json`: real signed HTTP/group routes; wrong actor/action/server, absent authorization, nonempty body, absence404, shared409 and protected publication409.

Production-scale metadata cost, standard Blossom routes and CORS remain separate work. No new throughput test was run.

Combined verification: results/audit-direct-delete-full-regression.log and audit-direct-delete-python.log. This includes existing group/peer/storage/serving regressions, Clippy and PostgreSQL process/restart checks, plus109 Python tests.
