# Native completion transport: source-only, 2026-10-22

## Frozen bases and boundary

Source base: d122f2b70be9217a9198215d243accc07f788dc8.
Backend contract read directly from 478ba35e527366bc224fe80d48d93d75f373484e:
domains/library/reader_completion.rs, domains/library/validation.rs,
domains/catalog/reader_selector.rs and domains/auth/principal.rs.
The pre-change owned-json.ts is retained as an exact text fixture with SHA-256
c1ede7e37d425b730e8e804055114a1abe9f26a15e7bb66c90b6350787d04717.

Fresh separately admitted source qualification passed 178/178 controls against
2d2ddea2e9f2a90b2754386fdf8cccf796b70855: native completion 152/152 and unchanged
forum equivalence 26/26. The transport product remains the implementation from
fb1cca909297e2f79d6fb06920c924ede3030b26; the successor changes only the missing-body
test expectation. The closed sanitized result and exact source/tool/driver/map/
receipt hashes are in native-completion-transport-qualified-20261022.json.

The first admitted run is retained as a failure: 151/152 native controls passed,
then execution stopped without running the forum stage. Its missing-body test
incorrectly expected a release when no reader existed. Review corrected the test
to require zero reads, cancellations, releases and allocations for a missing body,
while retaining one release for every existing-body case. Production was unchanged.
A separately reviewed fresh run then passed both suites. The old failure receipts
remain preserved, not overwritten or reclassified as success.

Both fresh stages retained exact source/tool pins and completed without forced
cleanup, remaining processes, unknown births or tracking errors, with stdout EOF
observed. The peak sampled owned RSS was 170,135,552 bytes for native controls and
167,505,920 bytes for forum controls. Tests used only synthetic fetch/stream/timer
seams and actual source in Node VM modules. No compiler/build, dependency install,
browser, socket, service, hosted CI, integration, publication, cookie acceptance
or backend acceptance was performed. This documentation-only successor does not
change product/test bytes or rerun tests.

## Narrow authority and interface

createNativeCompletionJsonReader(endpoint, injected) returns an asynchronous read
function taking { mangaId, ids, accountId, accountEpoch, catalogNamespace,
serviceKey } and { signal, assertCurrent, consume }. The endpoint getter must
come from the build-configured ReaderEndpoint, never request or route input.
Only fetch/setTimer/clearTimer may be injected; injected origin, headers and
credentials are ignored. The result remains { status, body: unknown }. DTO
validation, account/mode/epoch fencing, ReaderBudget and UI work remain owned by
the separate reader lane.

The endpoint requires canonical HTTP(S) origin and native-aggregate-v1. Its base
is empty at the root or slash-delimited unreserved path segments without trailing
slash. Percent-encoded, normalizing, authority/query/fragment or backslash paths
are refused. This matches ordinary canonical ReaderEndpoint output. Unusual
encoded configured base paths fail closed rather than silently normalizing.
Captured origin/base/profile are rechecked at every existing currentness boundary,
including deferred dispatch, response/read waits, parse, and final delivery.

The transport constructs only GET {base}/user/history/manga/{nativeId}/completion.
It sets only Accept: application/json and X-WeebDex-Expected-Account, uses
credentials: include, cache: no-store, redirect: error, and no request body.
It cannot upload local completion records or select another path. IDs and account
IDs are exact case-preserving native ASCII identifiers [A-Za-z0-9_:-]{1,128}.
There must be 1..50 distinct chapter IDs, preserved in caller order. Sparse arrays
are rejected. Account epoch is a nonnegative safe integer (negative zero refused).
Namespace is canonical lowercase UUID and service key is lowercase 64-hex or
explicit null; null becomes the exact wire control serviceKey=null. All validation
occurs before dispatch; request values are copied before deferred execution.

## Shared settlement owner

The previously reviewed fixed-allocation settlement engine is private and shared.
The existing exported generic factory's supported signature, public/anonymous/
forum credentials, custom error classes/status, no-abort 404 path and healthy EOF
behavior are unchanged. Runtime attempts to use credentials: include on that
factory now fail before dispatch; extra headers cannot affect the request.
Only the native fixed-route factory supplies the private expected-account argument.

Native completion owns a module-wide slot across all factories, endpoint changes
and component disposal/remount. Caller cancellation or deadline does not release
ignored fetch/read/cancel work. The slot is retained through cleanup grace expiry
and released only when actual work settles and the reader lock is released.
Late rejected promises are observed. Native and device-IDB owners remain separate
bounded owners: mode changes fence/cancel old actions, but no synthetic claim is
made that the canceled operation settled or that the device owner must depend on
the native network owner. Account re-entry cannot start another native operation
while the first remains unsettled.

Limits: one 16 KiB byte buffer, 4096 stream iterations including EOF, 128 empty
fragments, 10-second deadline and 250-millisecond cleanup grace. Declared byte
length, MIME, raw bytes, fragments and UTF-8/JSON are checked. Non-200 bodies are
not returned; their streams remain owned and canceled. Healthy EOF does not abort.

## Qualified controls and execution boundary

- test_native_completion_transport.mjs executes actual old/current owner source
  through Node VM modules and built-in TypeScript stripping, with explicit fetch,
  stream and timer seams. It checks frozen source identity; old/current anonymous
  and same-origin behavior; native exact route/headers; no credential widening;
  invalid IDs, accounts, epochs, scope and endpoint before dispatch; endpoint drift
  at dispatch/fetch/read/consume/release; authority drift; redirects and failures;
  exact byte/fragment/empty bounds; healthy EOF; request-copy stability; ignored
  abort, late resolve/reject and remount/mode/endpoint changes; no unhandled work;
  separate anonymous owner; finite grace and retained read/lock ownership.
- test_bounded_reader_shared_owner_equivalence.mjs is unchanged and continues to
  compare the original forum implementation to the shared-owner wrapper, including
  exact custom error classes/status and no-abort 404 behavior.

Each suite ran separately and sequentially with --experimental-vm-modules --test
--test-concurrency=1, a reviewed 60-second owner, a 256 MiB owned-RSS ceiling,
unchanged normal resource floors and bounded receipts. The reviewed executor
requires exact positive TAP counts, source/tool continuity and clean ownership;
raised exceptions or forced cleanup cannot qualify. The new round retained 82,835
bytes of private evidence, plus 62,902 bytes from the preserved failed attempt.
No raw logs, request data, private paths or full inventories are included in the
sanitized repository summary. A synthetic pass does not qualify browser cookies,
HTTP/backend behavior, UI integration or production capacity.
