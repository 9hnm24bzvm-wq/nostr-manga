# Real-image functional acceptance

This fixture is a complete workflow with the four committed Metropolitan Museum public-domain Hokusai JPEG originals in `server/tests/fixtures/real-images`. It is not a throughput, cold-storage, 100 TB, large-network or production-capacity test. No generated image bytes or transcoded substitutes are used.

## Run

Use an isolated disposable PostgreSQL cluster only. From the repository root, run `bash results/rebuild-real-images/run-fixture.sh`. `results/rebuild-real-images/run-fixture.sh` starts the vendored PostgreSQL on loopback port 55439, creates `manga_test`, uses the shared `prototype/target` build directory and runs:

```
cargo test --offline --locked --manifest-path server/Cargo.toml -j2 \
  --test http_real_images -- --include-ignored --nocapture
```

The test uses the actual `manga-node migrate` and `serve` commands, real strfry and the configured nginx serving supervisor. It creates a second isolated database through PostgreSQL's CLI. It never reads application SQL tables or private storage counters as an oracle. Failures retain the temporary node runtime for diagnosis; successful execution removes its owned node runtime after verifying listeners have closed. The runner stops its PostgreSQL instance and reports the retained cluster path.

## Observable workflow

1. Verify each original's committed SHA256, byte count, public-domain metadata and decoded JPEG dimensions. Preserve its exact bytes.
2. Upload all originals through signed HTTP. Check that staged originals are not public. Publish a signed series and four-page chapter and retrieve the exact signed events.
3. Reject synchronization before source approval. Approve both peers explicitly and synchronize the series/chapter into independently configured receiver storage.
4. Stop and reap the source API process and confirm its listener is closed. The source has no static helper. Retrieve every original byte-for-byte through the receiver API and its actual nginx `.jpg` path, with JPEG content types.
5. Save a signed reading-progress library. Restart the receiver, keeping the source stopped, and reread progress plus exact nginx originals.
6. Reject in-place chapter mutation. A signed deletion request alone preserves visibility; local operator approval withdraws the event. Replaying the deleted event cannot restore it.
7. Reupload the same real originals and publish a new manifest with reversed page order. Verify a new event identity, exact manifest/original bytes, the old event still hidden and saved progress still attached to its original chapter ID.

## Current execution

The final full functional execution passed on 2026-10-10: one test, zero failures, 55.63 seconds. Evidence: `results/rebuild-real-images/006-final-run.log`, its `.exit` file (0), and `acceptance.json`. All four source originals total 4,646,237 bytes. Clippy with warnings denied passed in `005-clippy-green.log`; the exact test source's format check passed in `008-final-format-check.log`.

The earlier first execution also passed (55.71 seconds), and its evidence is preserved as `001-first-run.log` and `001-acceptance.json`. The first lint attempt (`003`) found one test-only collapsible-if style issue; `005` and the final functional rerun follow that correction. No runtime source fix was required.

The final PostgreSQL shutdown check is recorded in `007-postgres-stop-check.log`; exit 3 from `pg_ctl status` means the fixture server is no longer running. Both owned node listeners and the receiver nginx listener were confirmed closed before the test reported success.

## Evidence boundary

`server/tests/http_real_images.rs` is an additional acceptance fixture against the reconstructed runtime; writing it is not itself proof that the workflow passed. A successful run writes `results/rebuild-real-images/acceptance.json` with original descriptors/source links and the actual node-binary hash. Consult its current execution log and final exit status. Earlier logs or a partial run must not be promoted to current acceptance.

This does not prove source-side deletion propagation, every peer failure/revocation interleaving, actual reader browser rendering, storage pressure, all image formats or capacity targets. Those require their own seam-specific checks.
