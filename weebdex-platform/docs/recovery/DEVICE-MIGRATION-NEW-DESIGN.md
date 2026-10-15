# New atomic device-ID migration replacement

The original large migration module was lost. `device-id-state.ts` and `device-id-migration.ts` are newly designed recovery implementations, not byte-identical recoveries. `savedLibraryLinkUpdatesReady` remains hard-false and no UI automatically calls these migration functions. Product activation also requires compatible library commands and the complete HTTP/browser acceptance.

The additive IndexedDB schema2 keeps the original database name, records/meta stores and native data. It adds immutable public aliases, SHA256 original-row archives, explicit issues, and deletion fences. Upgrade does not rewrite original records, revision, scope or an owner marker. Unconverted libraries still export/read version1 backups and remain usable. Legacy32-hex namespace backups remain inspectable without being silently adopted as UUID scopes.

A dedicated stable random device-owner UUID is created only by the explicit core seam. `prepareDeviceIdMigration` captures that owner, the exact verified origin/service/namespace, current revision, a complete synchronous SHA256 state fingerprint and a deeply frozen pre-update library backup. Network lookups run outside the transaction: bounded server aliases, typed visibility proof and actual unique chapter-parent proof. No browser-derived resource UUIDs exist. Unknown/hidden/wrong-parent rows stay retained with exact exportable original-row archives. Lists retain their original local identities; all manga/chapter/member reference keys are updated consistently.

`commitDeviceIdMigration` accepts only the exact privately registered review, its original database, explicit confirmation and current context checks. It rereads scope/owner/revision/all sidecars and fingerprints in one readwrite transaction. Rows, marker, count/revision and every sidecar commit together or roll back. An optional live context watcher aborts an interrupted transaction; a product hook must supply that watcher before activation. Reusing, fabricating or moving a review to another database is rejected.

Alias bindings cannot change. Newer timestamps win record collisions; canonical local state wins equal-clock native-alias ties; any remaining source-key tie is deterministic. Deletion fences win equal clocks and expand only through known scoped aliases. Removed/superseded originals remain archived. Repeating an unchanged migration creates neither duplicate archives nor another revision.

`preparePublicDeviceRestore` validates a complete version2 backup and prepares a separate explicit merge/replace review. Foreign device-owner backups need explicit admission; the target's stable owner remains unchanged, while archive source owners remain intact. Catalogue scope changes, alias rebinding and bypassing a populated unconverted legacy library are rejected. Existing deletion fences survive restores, so an older backup cannot resurrect a deleted row. Legacy import does not silently invoke this dedicated flow.

The retained device store now reads complete v2 snapshots. Its legacy saving/import/hydration/publication paths reject owner-marked v2 data with a visible compatibility explanation. The original signed public graph publisher rejects v2 before source mapping, worker work or signing. Nostr principals, keys, original events, signatures and protocol storage are unchanged.

Current executed recovery checks:21 newly written owned fake-IDB migration/store tests,19 typed client/composed API seams, and13 original-management/publication-fence tests pass. The source compiler passed with both catalogue gates. A real composed gate-on semantic check passed0errors/15warnings with679original files unchanged; the final post-fence semantic rerun also passed those same bounds. These are finite seams, not a product migration/browser activation claim.

## Exact native and existing-UUID alias proof

Every saved manga/chapter reference, including a syntactically valid UUID, is
submitted to the namespace-scoped typed `/api/v1/legacy/resolve` contract. Its
`legacy_id` is the exact requested input and `id` is the preferred public UUID.
The client rejects omitted, duplicate, wrong-kind and unrequested responses.
Only these proved preferred IDs reach the unchanged exact visibility and parent
proof fences. A collection returning a preferred ID for an unproved alias is
still rejected.

Non-identity native/UUID bindings are retained in the same immutable kind-scoped
alias ledger. Original rows remain archived; deletion fences retain both the
original alias and the preferred identity. These changes do not rewrite signed
Nostr roots or events. Missing proofs retain unresolved exportable source rows.
The product migration switch remains independently disabled.
