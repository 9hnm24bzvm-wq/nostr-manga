# Reconstructed current signed public-list contract

The original signed-list reads and feed are implemented and qualified on the
isolated compatibility branch using actual HTTP, PostgreSQL and original signed
storage. Combined integration, browser and load qualification remain separate.

GET `/user/list` is the exact admitted Nostr signer's public graph. GET `/user/{id}/list` uses a typed ProtocolPublisher display UUID. GET `/list/{id}` and `/list/{id}/feed` use typed ProtocolList(owner64hex,list32hex) targets. Public paths require no proof; self path requires the service-bound exact request proof. Native account/private/public local lists never select a Nostr graph. Admission revocation and public content availability are distinct; original deletion, health, holds and visibility remain authoritative.

Migration91 adds derived aliases and a durable bounded cursor only. The existing background importer walks one owner and at most100 IDs per turn, backfilling old heads at startup and retrying current heads in full sweeps. It does not parse blobs in a SQL migration. `/api/v1` GET creates no aliases. A current head not fully projected returns503, rather than false empty data. Same-head completed readiness does not flap during rescan. A changing/unavailable owner advances the cursor so later owners progress, and later sweeps retry it.

The original owner:list UUID scheme and trusted preferred/legacy aliases are preserved. Equal raw list IDs under different owners remain distinct. Removed-list/deleted-head stale aliases return404 and cannot resurrect an old root or another owner's list.

List detail also proves its requested-to-canonical identity binding in both
`nostr.identity` and `data.nostr.identity`:

```json
{"kind":"list","requestedId":"normalized requested UUID","canonicalId":"returned data.id","source":"persisted-public-resource-ids"}
```

These fields come only from the persisted typed `ProtocolList` resolver, never
publisher metadata. Uppercase UUID input is normalized. The returned entity must
match that exact resolved canonical identity or assembly fails503. A client may
accept an alias redirect only after validating every proof field against its
captured request and returned entity; without a matching proof it must require
exact requested identity. This proves identity only, not ownership, permission,
account binding, source visibility or authority to follow. Original source
visibility and final signed-head checks still apply. Collection DTOs have no
request-specific alias proof.

Collections support limit0..100, default10, real offset and optional `head=<signed event id>` snapshot extension. List GETs declare no includes/privacy filters, so these fail400. Stale or changed heads fail409 after a final recheck following DTO mapping/feed rendering. Requested relationships are bounded5000 records, with5001 failing400, never truncating.

Members are original-visible series and currently visible local catalogue projections with canonical UUIDs. Preserve signed member ordering and raw IDs in provenance. List feed uses the existing chapter filter/include/render helper over an explicit bounded native scope. Empty membership means empty feed, never the whole catalogue.

Original lists have no MangaDex mutable version counter or creation date. Omit unavailable fields and describe this module schema exception; do not invent version1, timestamps or readDate. All original graph lists are published public data. Browser-local unpublished state stays local. List mutations require an explicit client-signed original publication; plain JSON is not signing permission.
