# Signed staged-page replacement recovery

This is a new implementation on recovered source, based on local checkpoint `456bcb03`. It does not restore any lost historical Git blobs. The original frontend attachments stay unchanged; the enabled public upload overlay supplies this behavior. The separate frozen browser03 worktree and its pending execution approval are unaffected.

## Public transaction contract

`POST /api/v1/nostr/uploads/{session}/files/{file}/replace` takes an exact signed raw multipart body: one plain `expectedVersion` field containing canonical positive decimal i32, followed by one `file`, then EOF. NIP-98 signs the actual complete body, method, configured public URL and service. No cookie or unsigned version header grants authority.

The route resolves the public session/file IDs and checks the bound account, management permission, ownership and immutable session before decoding. It discards that preflight actor. After decoding, a new account/service/binding/grant check and the existing native replacement core run in the same mutation transaction. The public path strictly checks the signed version even for identical bytes. The native cookie path retains its existing identical-response retry behavior.

A successful transaction keeps the file ID, native position and saved page order. It atomically swaps the original object and derivatives, updates the session version only for a changed image/name, and returns the actual public file resource plus `sessionVersion` read under the transaction lock. Rejection keeps the prior image. Existing real-image, individual/aggregate byte, expiry and prepared-publication guards remain authoritative.

## Browser-facing adapter and UI

The serializer reads the selected file once. It returns the exact original-image slice already copied into its multipart body; the adapter hashes that slice and sends those same serialized bytes. A normal result requires the stable ID, name, digest, size, MIME and actual session version, then an exact owner session read matching the prior target, draft, groups, page order and unchanged neighbors.

Before dispatch the adapter persists bounded replacement facts in a separate origin/account/service/catalogue-scoped local-storage fence. The one original request alone has a private allowance through its own fence. Queued writes, direct mutation helpers and a later editor reload remain fenced while that outcome is unresolved. Install/clear use a bounded, exclusive per-account Web Lock with `ifAvailable:true`; a busy or unavailable lock fails closed. No network operation runs under that lock. Clearing checks the exact attempt so an older cooperating adapter callback cannot erase a newer tab's fence.

A transport failure does not imply refusal. There is no automatic replacement replay. The explicit “Check replacement outcome” control performs read-only exact-session reconciliation. A matching new stable-file receipt at the exact expected version can resolve a changed replacement. An unchanged pre-version after an unknown request cannot prove that a delayed request will not still apply. A lost identical no-op remains unresolved unless the original request had a definitive valid success response. Provably unsent requests and valid explicit HTTP refusals can clear only their own attempt.

The UI retains the prior card and displayed preview until confirmation, then updates the same reactive card object so its keyed component and same-hash preview lease remain current. Confirmed adoption preserves unchanged neighbors and transfers or releases Blob URLs through the existing preview lease owner. The signed preview key includes the file hash, so a new image on the same file ID cannot adopt the previous lease's late Blob. There is no URL monkeypatch or synthetic page-order injection.

## Qualification status

- Source-extracted UI seams: 35 passed.
- Complete source-only recipe/stage provenance preflight: 12 passed, enabled and default-off. The old native replacement recipe is recorded as superseded; its presence is not execution coverage.
- Adapter fake-transport seams: 57 passed, including post-POST read/callback uncertainty, restored-authority races, and cross-tab fence admission.
- Exact-byte fake transport plus multipart/preview seams: 32 passed. These use fake signers/fetch and establish no genuine cryptographic, HTTP or browser result.
- Final source/mock scope: 136 checks across four explicit commands. The earlier UI/provenance predecessor runs are retained separately and are not added to this total.
- Backend `cargo test --locked --offline --manifest-path backend/Cargo.toml --lib --test public_uploads --no-run -j2` (tool/manifest paths abbreviated here; exact absolute invocation is saved in the receipt): passed in a separate target. All 308 captured Rust/source pins and the held browser03 inputs stayed unchanged.
- Four exact Rust pure cases: passed (native/public version policy, canonical version values, real in-memory Axum multipart parsing, exact route budget/private-cache selector).
- Genuine disposable PostgreSQL/HTTP replacement assertions: authored and compiled, not executed. They do not establish concurrent after-decode revocation; the production recheck is source-reviewed.
- New enabled frontend strict/build and actual replacement browser journey: pending. The older artifact still implements explicit replacement refusal.
- Public chapter editing remains unavailable. No full-40 or complete upload parity claim is made.

The existing browser startup results and all earlier failed checkpoints retain their original verdicts. This source-only branch creates no publication and performs no runtime database or browser action.
