# Denied CONNECT socket lifecycle

Hosted run [38502322656](https://github.com/9hnm24bzvm-wq/nostr-manga/actions/runs/38502322656)
used `0ee583e53a698975c17d19c679736146a1cea5f2`, tree
`6ab0b95f672175d3aecf7dfd255924c56f566537`. All ten startup journeys
passed their nineteen original assertions. Three executions still exited 1
with an uncaught exception during `closing-browser`, before report finalization:
ordinary account, device navigation, and the home-batch startup. These remain
failed; the artifact did not contain exception codes or stacks and therefore
does not uniquely establish their cause.

The existing external-deny proxy hands CONNECT sockets out of Node's HTTP
parser without taking socket-error ownership. With the pinned Node 24.19.0,
ten bounded loopback reproductions of the exact original handlers each exited
1 with `ECONNRESET` when a denied peer reset during browser-close stage. No
browser, application, database, credentials, or external connection was used.

The extracted proxy still returns the same HTTP/CONNECT 403 responses and never
opens an outbound connection. Only a socket that has received a CONNECT denial
and remains in this proxy's tracked accepted-socket set owns `ECONNRESET`.
Other codes, stale/unowned sockets, and server errors retain fatal behavior.
Shutdown destroys only tracked sockets, joins their close events and the server
close callback, and has a seven-second deadline. An unresolved close cannot
finalize a passing report. Passive fatal observations now copy only four fixed
error-code identities or `unknown`; no error text or stack is exported.

The public runner evidence requires a closed finite proxy lifecycle record as
well as the original zero-exit, finalized-report and cleanup gates. Missing,
malformed, open, or unresolved proxy evidence cannot support a pass. Expected
CONNECT reset counts are observations of owned denied sockets, not exemptions
from application, browser, runner-exit or cleanup assertions.

`node tests/ci/test_browser_deny_proxy.mjs` runs 24 bounded child-process probes:
normal HTTP/CONNECT denial, ten repeated resets, early and half-closed resets,
malformed HTTP, idle shutdown, eight separate-process peer resets, a concurrent
reset/shutdown race, and strict unexpected-error/deadline controls. Successful
children naturally exit after proxy sockets, peer processes and listener closure
are joined; a closed-port probe requires connection refusal. Unexpected EPIPE,
stale/non-CONNECT reset ownership, server error, repeated close and a stuck
socket double remain nonzero failures. No global exception handler or forced
success exit is added.

An earlier stronger same-process eight-client Node reset stress reached zero
proxy sockets and a closed listener but stalled during the Node client's native
process shutdown. Its failed diagnostic receipts are retained separately. The
passing multi-peer probe instead uses bounded Python TCP peers in a separately
joined process, matching Chromium's actual process boundary. That qualifies the
harness proof; it does not relabel the same-process stress as passed or establish
a genuine hosted result. Fresh hosted acceptance remains required.
