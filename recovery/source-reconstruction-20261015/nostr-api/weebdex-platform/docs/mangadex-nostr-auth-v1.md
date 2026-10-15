# Reconstructed Nostr API authorization contract

RECONSTRUCTED UNVERIFIED after environment replacement. These are intended invariants and must be tested again against integrated source.

Protected `/api/v1` requests use original Nostr account admission and replay protection, not MangaDex OAuth/password authentication. The exact external platform public origin is trusted deployment configuration. Host, Forwarded, X-Forwarded-Host and X-Forwarded-Proto never select the signed origin, instance or principal. Preserve the OriginalUri including `/api/v1` and query bytes; rewriting to a native upstream URL invalidates the signature and is not an authentication mechanism.

A proof is an original signature-verified kind27235 NIP98 event in the Authorization header. It binds exact external URL, HTTP method and original body bytes, including empty mutation bodies, with the original60-second freshness rule and durable one-use replay ledger. The new `/api/v1` extension requires exactly one signed `["service", "<selected original service pubkey>"]` tag. Missing, duplicate or mismatched tags fail. The selected service key must equal the durable original node identity. Old original native endpoints keep their existing contract.

The service identity comes from the configured original publication storage holder and native-node configuration. Protected adapters require protocol_publication.native_node plus its matching storage_holder, the same original Repository and an attached original runtime. This is a protected-adapter deployment requirement, not a global requirement for unauthenticated public catalogue reads. The browser's original service challenge is proved at the selected native service origin; the platform request proof still signs the exact platform URL. A domain change does not silently replace the selected original service key.

Only a private, non-Clone AdmittedHttpCapability can carry a verified request principal. It contains exact principal, service, URL, method, body hash, authorization ID and original AccountPermit. Public UUIDs, display publisher aliases and native cookies cannot construct it. Protected transactions set statement and lock timeouts before authority locks, then recheck original service and account admission/version.

`GET /auth/check` truthfully returns the admitted Nostr identity and empty roles/permissions. Any draft/native-resource account ownership requires a separate explicit operator binding, active/verified native account, existing banned/restricted policy and exact native permission checks in the SAME transaction as protected data. It never creates a native account, fake session or signing key. Binding helpers must be recovered/reviewed separately by the foundation owner.

## Signed library scope

Read-marker GETs read only this principal's current original signed public graph, using the shared original LibraryObjects and UploadStore owner. Browser-local state remains the default; opting into public publication must remain explicit. Public publisher/list presentation UUIDs have no signing or account authority. Completed=false, absent and partial-page progress do not become completed read markers.

Plain MangaDex read/status mutation JSON cannot authorize server signing. The planned versioned client-signed commit extension was approved in design but NOT implemented before the reset. No success stub should replace the truthful unsupported response. `/user/history` cannot fabricate actual readDate from generic progress updated_at.
