# Native browser publication and recovery qualification

**Passed: a small four-family functional flow in actual dot cloud Chrome.**
Production browser build `e4f4862bd75c26bb` and node
`82c52b35720e623a303ec49fad73e65e08e033baf83b4fcc4349890af174c491`
were unchanged. All 57 served production asset hashes were rechecked after the
flow. This is additional browser acceptance, not a new application build or a
capacity result.

## Observed public behavior

1. A fresh loopback origin saved one chapter, series, named list and membership
   through the rendered forms. An actual downloaded JSONL backup contained all
   four records.
2. A wrong service-identity pin was refused. Correct identity was remembered only
   after its confirmation. Publication without the public-consent checkbox was
   refused, with no queued job. Signed public readback returned no library head.
3. The owned real node was stopped while its static client and PostgreSQL remained
   available. Explicit publication created one signed, durable retryable snapshot.
   Actual browser reload preserved its exact event ID and original revision.
4. Restarting the same node/backend did not publish automatically: another signed
   public read still returned HTTP 404/no_library_head. Explicit retry accepted
   the original event. Independent verification checked the response proof,
   publisher signature and projected=true.
5. A later local chapter edit to completed/page 9 did not change that public head.
   Recovery displayed the verified owner, service key, instance, event and four
   records. Declining replacement preserved the edit, confirmed by a real export.
6. Approving the same recovery restored the original page 7 state and every field
   of all four records, including their timestamps. A second actual browser
   startup and export retained exact record equality. Export envelope timestamps
   and local revision numbers are intentionally not required to remain identical.

Accepted event:
`8a50954633314357b562b33ca3454c2ce1ada894d1ea448e8c69a63f97b12201`.
The outbox keeps its original “Accepted; projection pending” receipt; later signed
HTTP readback independently established that projection had finished.

## Evidence and reproduction boundary

`acceptance.json` records 17 checked conditions, identities and export hashes.
The four `.jsonl` files are actual browser downloads. The `.txt` observations are
rendered UI/dialog text, and `head-*.json` are signed public HTTP outcomes verified
against the fixture service identity and request/body/status scope. No private
IndexedDB, SQL tables or internal counters were used as outcome oracles.

The client ran at `http://127.0.0.1:18768`; the disposable node ran at port 18769,
with PostgreSQL at 55434 and the real pinned strfry binary. Existing origin 18767
and its large local library were left untouched. The fixture injected only a
clearly labeled synthetic NIP07-compatible signer interface into the otherwise
unchanged application iframe. It did not inject records or call private client
state. The direct same-origin production page captured downloads because the
browser tool did not report iframe download events. Keyboard activation was used
where iframe pointer positioning did not activate a visible control.

An initial executor-local listener was unreachable from the cloud browser. The
successful fixture was launched through the supported native Terminal in the
browser host. Its standard-library controller exchanged bounded synthetic signed
HTTP requests with a fixture helper in the shared workspace. These setup changes
were confined to test orchestration. No application source, network security
setting or extension was changed.

All owned services were stopped and reaped; `stopped.json` and the terminal log
record shutdown. The fixture enforced a 2 GiB disk floor and a 90-minute maximum
lifetime. The successful run used only four records and completed well within
those limits.

## Limits

This does not qualify installed NIP07 extension prompts, remote signers, real
accounts, mobile hardware, browser eviction/disk-full behavior, physical power
loss or large-library native publication throughput. The signer is synthetic;
actual Chrome Worker, IndexedDB, Web Locks and the real server transport were
exercised. No full reader-website scope is inferred from this reference-client
qualification.

## Safe checkpoint evidence

Use `safe-evidence-files.json` plus `evidence.sha256`. The listed reports, public
responses and synthetic backups are safe evidence. Exclude all runtime trees,
passwords, service-key/configuration files and key-bearing fixture scripts.
Screenshots are sanitized local captures but are excluded from the checkpoint
allowlist unless specifically requested. The stopped runtime is private recovery
material, not part of the source/evidence checkpoint.
