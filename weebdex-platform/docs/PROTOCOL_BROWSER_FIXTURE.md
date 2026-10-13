# Combined original-backed browser fixture specification

Status: source specification only. No provisioner or combined fixture was executed.
The existing supervisor is `tests/runtime/browser_fixture.py`. Its current native
fixture cannot establish successful signed protocol library/profile/social delivery.
One assigned fixture owner should extend it and add the guarded Rust provisioner
together, then hand the compiled artifacts to the sole compiler/test coordinator.

## Ownership and isolation

Use one supervisor process namespace for a fresh private loopback PostgreSQL cluster,
the real platform process with its embedded original listener, the reserved-address
mail sink and Chromium. Disable Unix sockets and bind every service to loopback.
Use a new bounded directory under `weebdex-platform/test-runtime`; reject arbitrary
database names, hosts, directories and symlinks. A finite observation deadline and
verified child cleanup belong to that supervisor. Do not launch another unsupervised
original node or mount an original UploadStore from two mutation owners.

The existing production `main.rs` owns both listeners, source importer, cover worker,
native storage maintenance and graceful drain. Run that real executable, not a
successful-response adapter or a separate source subprocess masquerading as module
integration. The installed real strfry backend is an original-module projection
dependency, not a replacement protocol implementation.

## Guarded Rust test provisioner

An additive test-only example can use actual exported original module services:

1. Verify the reserved disposable DSN and fixture root. Open `Repository` with the
   fixture's loopback pool and apply its actual 52 migrations with `Repository::migrate`.
   Keep native migrations in the existing platform migrate command. Never copy
   canonical admission or publication SQL into Python.
2. Generate ephemeral synthetic operator/service identities only inside this isolated
   test fixture. Write required server key files with mode0600 under the private
   fixture root; they are not production keys or evidence. Use distinct operator and
   service keys to cover federation-holder identity wiring. Remove them on cleanup.
3. Admit the synthetic operator through original `set_publisher_admission`, and admit
   only explicitly supplied synthetic browser public keys through original account
   admission. No password account becomes a signer or gains account admission by
   logging in. The browser NIP07 private key stays in browser-runner memory; the
   supervisor/provisioner receives only its public key.
4. Emit nonsecret `OperatorRuntimeConfig`/original `NodeConfig` JSON referencing those
   fixture paths. Use separate native uploads and original storage roots; existing
   holder/service key, `read_policy:"public"`, no automatic publication deletion
   approval, disabled unrelated sync worker, actual backend binary/data directory,
   matching group-key roots, and a distinct reserved original loopback listener.
5. If signed seed resources are needed, open `PublicationRuntime` as the sole original
   store owner and publish generated fixture series/chapter images through real
   `Publications` and typed canonical1002/1001 inputs. Release the provisioner's store
   owner before starting the platform, or seed via the actual authenticated native
   publish/upload pathways after startup. Wait for genuine backend projection and
   importer aliases; do not invent event IDs or treat pending acceptance as public.

Original standalone CLI migration can be an operator setup fallback; the intended
browser seam uses the Rust module provisioner so source admission and seed publication
are source-owned interfaces rather than arbitrary fixture table writes.

## Exact browser transport

Configure `public_origin` as the actual UI loopback origin. Original
`native_node.base_url` must be its exact external browser-visible loopback origin,
with `cors_origins` containing the UI origin. Add only exact reviewed local HTTP/WS
origins through the native configured base plus `protocol_connect_origins`; use the
same effective approved origin list in CSP and public capabilities. No wildcard,
external provider, analytics or generic wss allowance is needed.

Extend `scripts/browser-acceptance.mjs`'s exact-origin routing guard only for the
nonsecret supervisor-declared original service origin. Continue blocking all other
requests through the owned deny proxy. Do not log signed-request bodies, proof
headers, tokens, private keys or mail recovery links. NIP98 proofs bind the exact
original URL/method/body; preserve original root paths without proxy prefix rewriting.

Required genuine endpoints are `/identity?nonce=…`, `/account-metadata`,
`/accounts/{pubkey}/metadata/{kind}`, `/social`, `/account-deletions`, `/libraries`,
`/libraries/{pubkey}/entries`, `/library/batches`, `/library/read`,
`/library/blobs/{hash}`, `/library/objects/{kind}`, `/library/staging/{hash}` and the
original root Nostr WebSocket. Browser recipients carry `{origin,serviceKey}` and
must pass real service proof; password session APIs are a separate capability.

## Acceptance and terminal evidence

Exercise native-only device persistence before consent, actual current64hex mappings
bound to the persisted catalogue namespace, explicit external NIP07 signer selection,
explicit reviewed relay/service recipients, real11999 delivery/import and restart
recovery, profile0/3/10002 publish/current read, comment1111/reaction7 signed events,
and ordinary author kind5 withdrawal. Observe actual original acknowledgments,
stored verified events and backend completion; queued/pending is not success.

Include denied signer prompt, denied proof, unadmitted account, wrong service key,
unapproved recipient, rebuilt catalogue namespace and revoked source mappings.
Check no unsigned native IDs enter public signed library content, no signer is
silently generated, and failed consent creates no accepted-publication claim.

Evidence may contain public synthetic IDs, origin/namespace, event kind/digest,
status/acknowledgment, artifact hashes and test outcomes. Exclude credentials,
signer secrets, proof/request bodies, personal data and runtime databases. Stop
Chromium first, signal the platform so both listeners drain, verify all children and
ports terminate, then stop the private PostgreSQL/mail service and remove fixture
key files. Report force-kill or incomplete drain explicitly.
