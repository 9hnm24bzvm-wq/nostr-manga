# Signed runtime follow-up after run 38615879103

## Observed boundary

Verified hosted run 38615879103 used public e94029800c1ffde6f1d7e90d36d121ef54c12651 / tree
83141fce1dba67dbd529cc1da53b065bba672133. All 23 common signed setup/security checks passed independently in all
four executed signed phases. No page error or runner fatal was recorded.

- signed-library: auth-preview-send passed; auth-exact-job-retained failed.
- signed-social: the first conversation-loaded check failed after selection.
- signed-source-controls: admission/identity passed; conversation-loaded failed.
- signed-recipients: review/publish controls passed; retained acceptance failed.
  Its later peer-policy mutation passed, then the public-head read failed.

These fixed receipts do not establish an HTTP/CORS, cryptographic, source or
backend defect. They also do not establish that a failed publication created a
library head needed by the later peer-policy assertion.

## Reproduced publication defect

LocalLibraryPanel held its publication review in Svelte `$state`. Its publish
callback handed that deeply reactive object directly to publishReviewed. The
original queueSnapshot retains graph.objects in its new job. The original
PublicationOutbox.create inserts those graph objects into IndexedDB jobObjects.
Svelte proxies cannot cross the structured-clone insertion boundary.

The new seam uses the installed exact-lock Svelte 5.55.1 proxy and snapshot
implementations, its actual compiler, original PublicationOutbox/indexeddb
modules, and fake-indexeddb 6.2.4. The adapter calls real structuredClone on
insertion rather than retaining references. The old reactive graph raises
DataCloneError and the transaction rolls back to zero jobs. This explains a
concrete source boundary after the snapshot signature and before delivery; the
hosted receipt does not expose the private exception itself.

The one-line product correction takes `$state.snapshot(publication!)` inside
the owned click callback, synchronously before clearing consent/review or any
await. All original source fingerprint, device/canonical revision, signer,
recipient revision and delivery guards remain unchanged. There is no later
snapshot that could capture a replacement review. The test pauses the downstream
operation, mutates the original reactive review and graph bytes, and proves the
captured intent, event JSON, both pinned recipients and stored bytes stay exact.
Original outbox revision/recipient-revision CAS negatives still reject.

This is local product-seam evidence, not fresh browser or real-publication
acceptance. A later source/proof/delivery error could remain after this fix.

### Other persistence/signing handoffs audited

- Retry/cancel/remove pass a primitive job id; retry reloads the original durable
  plain job and event, with original recipient and signing-identity checks.
- Recipient selection creates fresh relay entries from primitive origin/key/
  selected fields; verified remembering creates fresh proof-derived entries.
- Device import validates and normalizes records before insertion. Recovery
  normalizes each family into fresh canonical records before replacement/CAS.
- Canonical import passes the non-proxied File and primitive expected revision to
  the original staged import. Adoption persists primitive namespace/key fields.
- The identity panel already snapshots its reviewed public action before await.
  No additional persistence handoff was changed on speculation.

## Finite isolation of unresolved conversation reads

Only the first conversation load in signed-social and signed-source-controls
gets five additional nested status-only checks: exact native target controls,
actual scoped mapping response, genuine original root response, verified public
feed response, and the original canonical-source UI visibility assertion.

The passive observer matches the exact native POST body and original GET paths
at already approved fixture origins. Background/wrong-body/wrong-id responses
cannot satisfy it; missing, duplicate, unfinished, oversized, invalid-JSON or
failed-body observations fail. Root bytes retain the application's 16 KiB bound;
mapping/feed remain bounded at 1 MiB. Bodies are read only after requestfinished,
so no extra network-stream wait or fetch is introduced. All decode tasks are
immediately owned and joined when listeners are removed. Only fixed check ids
and statuses are published; URLs, bodies, keys and messages remain private.

The original 45-second post-click visibility wait is retained verbatim. Checks
inspect only the completed operation; no sequential timeout allowance is added.
Later conversation loads retain the original path and cannot replay these ids.
There is no CORS relaxation or speculative source/backend change.

Two-recipient acceptance additionally separates retained signed job, exact
primary receipt and exact secondary receipt. Explicit retained-job admission
and completed two-recipient publication prerequisites prevent later fixture
controls from running after their required setup failed. All original peer
revocation/restoration and separate public-read assertions remain required.

## Local validation and limits

Focused snapshot/observer/outcome/wiring tests passed (14 tests). They execute
the exact source callbacks with explicit driver/service/verifier stand-ins where
stated, and independently test the real proxy/outbox clone boundary. They do not
claim a browser, network, real signature/delivery or peer-policy acceptance.

The final browser source seams pass 257 tests when the sole stale current-plan
binding test is explicitly excluded. The preceding unfiltered browser glob ran
257 tests before the compiler-only test was added: 256 passed; its only failure
was the expected stale journey-diagnostics.py hash in
the current browser-actor-follow-plan source binding. Current combined plan and
composed-artifact hashes must refresh after integration; historical receipts are
unchanged. All 53 Python CI tests and all 676 canonical frontend hashes passed.

No hosted run, service startup, dependency installation, public write or timeout
increase was performed by this follow-up.
