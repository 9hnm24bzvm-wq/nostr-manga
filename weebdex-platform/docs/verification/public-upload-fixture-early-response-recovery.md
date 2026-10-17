# Reconstructed browser fixture: early HTTP response

Browser01 failed before submitting any browser command. The genuine original
fixture seed reached readiness and `start()` printed `FIXTURE_READY`; the later
operational HTTP checks then raised `URLError` with Broken pipe. The retained
`local-mail-sink` stage was stale. No failing request name or HTTP status was
saved, so the oversized JSON POST remains a source-supported candidate cause.
This result is not reclassified as an observed HTTP 413 or a browser result.

The original supervisor source, sanitized owner receipt/independent review and
two filtered public log entries are retained in
`test-evidence/public-api/upload-product-recovery/fixture-early-response-source/`.
Its nested physical teardown was clean, with 41 saved births absent and four
nested listener closures; the outer exact-port observation was missing and its
terminal diagnostic remains preserved. Lifecycle success remained false.

The test-only successor records closed name/method/local-path/body-count/expected
status metadata before every operational HTTP call. Transport failures retain
the actual parsed status when available, otherwise null, and a fixed exception
class. Raw error text, request headers and request/response bodies are not copied
into that diagnostic. Other operational checks keep their existing transports.

Only the exact oversized POST uses a standard-library `HTTPConnection` owner.
It still attempts the real 1,048,577-byte body with the matching Content-Length.
A send-side BrokenPipeError or ConnectionResetError is retained separately;
the client attempts to parse the response from that same connection. There is
no replay or inferred status. Actual HTTP 413 and complete bounded response framing
are required by the operational assertion. Completed send-call bytes are local
client observations, not claimed server-received body bytes.

One five-second absolute watchdog interrupts the captured original socket even
during header parsing; remaining-budget I/O checks share that deadline. Response
and connection close are always attempted. Timer cancellation/join has a separate
one-second cleanup bound; expired, incomplete or unsettled work cannot pass.
Primary failures remain primary when cleanup also fails.

The initial 25 AST/fake-I/O cases passed on their first run. They exercise the real
helper/check bodies with fake connections, sockets and timers; no real thread,
socket, fixture, PostgreSQL or browser was started. The completion association
then added 3 source-extracted publication cases; all 28 successor cases passed.
The unchanged helper and prior 25 source/log are retained separately.

The operational receipt now includes the exact run directory. Only after that
receipt is atomically saved does the supervisor publish its saved path/hash in
`operational_http`, before final ready. Listener startup clears any old marker.
The outer owner must verify the fixed receipt path, matching saved hash and exact
current run; readiness alone or a stale global receipt cannot authorize a browser.

A fresh genuine owner run
must still verify the actual HTTP 413, complete body and settled watchdog record before
submitting its browser command. Product, frontend artifact, backend binaries and
browser journey source remain unchanged by this correction.
