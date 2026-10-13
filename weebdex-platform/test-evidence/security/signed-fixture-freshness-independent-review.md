# Independent signed-fixture freshness review

Reviewed 2026-10-13 UTC. Verdict: PASS for the final frozen023 test-only correction. Compilation and real-runtime success remain the compiler/runtime owner's responsibility; this reviewer ran no Cargo, browser, database or runtime commands and made no implementation edits.

## Scope and exact delta

Target: backend/tests/protocol_bridge_http.rs, final SHA-256 f223ac92d986dfca4a182a641079b95e97ad338662b3ec20a94188f91c713bfa.

The prior022 source was reconstructed by removing exactly the two explanatory comments, bounded receive-second wait and validator assertion. Its resulting SHA-256 is 40e3fa63864e68efe6a3a4f734a8c7bbf172b4643c368f7c98454cf9aa8f8dee, exactly the prior compiled candidate's pinned source hash. All other bytes, including every drain/disconnect assertion, are identical.

The final correction keeps the unchanged public auth::sign, waits at most two seconds while auth::now() <= upload_proof.created_at using ten-millisecond sleeps, and calls the actual original auth::blossom with the same origin, upload action and manifest hash before the real HTTP request. This fixes a synthetic current-second fixture's violation of the existing strict receiver freshness contract. The wait fails explicitly on clock/scheduling anomalies; it does not accept a403, retry around authorization, or change the verifier.

## Original contract and bindings

- Original server/src/auth.rs:159-179 rejects kind !=24242, created_at >= now(), expiry <= now(), lifetime >300 seconds, empty content, incorrect action/domain or absent/invalid hash scope. Signing uses current whole-second time. Original auth.rs SHA-256 ae9e2e6e86c14c25d53fdd5f22c53b285061f7502996bf463b9a1c721de8dc46 matches the frozen source baseline.
- The unchanged proof has t=upload, server=the actual listener's configured origin, x=SHA-256 of the exact encoded drain manifest, expiration=now()+120 and nonempty content. The real request remains PUT to the original /upload/{hash} route, Nostr base64 authorization, exact manifest body, and expected200.
- Original node.rs binds this route to PUT. blob_actor parses auth::token, verifies event ID/signature, applies original Blossom scope, restoration clock and publisher admission. upload_blob applies authorization again and rejects empty or hash-mismatched bodies before original owned storage work. No injected acceptance, authority bypass or fixture-only production path was introduced. Original node.rs SHA-256 9e2d09fda7e05a8bbc740879b9ed5622a189e853eec66ceda8c1c7daec4024a3.
- Subsequent signed1002 publication, POST URL/method/body-bound HTTP proof, actual source-verdict advisory lock, observed blocked original mutation, client abort, shutdown, assertion that listener is unfinished while blocked, barrier release, bounded successful listener join, durable hosted-event query and retained peer-byte checks remain byte-identical to022.

## Integrity and resolved finding

Candidate023 receipt SHA-256 d94878d178e3b614d72c9ca3ac7f2cf66ea48426d19e0f4ea91ec0870b2bcf37 at weebdex-platform-build-environment/synchronous-restore/results/bridge-authorized-023/candidate-source.json. Candidate022 to023 has no changed original-source records and only the targeted test hash changes in platform_files. Independently verified117 current production source, migration and Cargo paths match candidate023 hashes, with zero mismatches. Backend Cargo.toml SHA-256 220c6eaf5e0630f17a02d66506d5102f322213a1dbcbf8814fac8f2dc059725d.

The provisional correction used auth::sign_at, which is pub(crate) in the original library and unavailable from this external integration test. This reviewer reported the visibility blocker promptly; the owner replaced it before any Cargo/runtime receipt with the bounded wait above. Provisional source89b78e35549c38aec741a85b514a94e4e88db8b8de296cc3df3ceb12f40c7cd0 and its candidate receipt are superseded. No original export or dependency changes were needed.
