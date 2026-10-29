# Owned remote-signer browser focus fixture

**Isolated bundle and owned HTTP start/close smoke passed; actual browser/CUA acceptance remains pending.** The smoke opened no WebSocket and signed no application event. Evidence is `ux-evidence/six-persona-fixes-20261028/remote-browser-smoke/receipt.json` from the repository root. The operator must approve the browser window and choose an already-qualified sandboxed Chromium runner first. No sandbox override, production CSP change, external relay, credential connection or real account mutation is required.

`fixture.mjs` builds the **current** RemoteSignerPanel, RemoteSignerStatus, remote coordinator, bunker adapter and public-key display module, with the final frontend's actual Button/Input/Dialog wrappers and actual `bits-ui` primitives. It does not mock Dialog. The native-account reads, Session source and publication-selection registry are explicit finite synthetic boundaries in `boundaries.ts`. The shell is not the full deployed Account route, and fixture geometry CSS does not establish production visual parity.

The fixture binds a fresh `127.0.0.1` port. Its exact reviewed relay origin is `ws://127.0.0.1:<that-port>`, endpoint `/relay`; the page is `http://127.0.0.1:<that-port>`. Only the synthetic capability boundary returns that one origin. Its response CSP admits only self and that exact WS origin. HTTP Host/Origin and WS Host/Origin/path are checked; CONNECT and other endpoints/methods are refused. This uses the adapter's existing loopback-development rule and **does not broaden production policy**.

The server generates synthetic bunker/user keys only in process memory, verifies every actual browser transport signature, uses the pinned official NIP44 implementation and holds/replies to authenticated get_public_key requests. It never signs an application event. Public connection links remain ephemeral and must not be logged or screenshotted; there is no connection secret. Receipts contain only finite booleans/counts/source hashes, never event/key/token bytes. Cleanup terminates sockets, stops the server, removes the temporary bundle and clears owned private-key buffers.

## Supported CUA interactive invocation

After exact source/build pinning and browser-session approval, start the
fixture server (this command launches **no browser**):

```sh
WEEBDEX_FRONTEND_ROOT="$PWD/.build/frontend-ux-final" \
WEEBDEX_BROWSER_DOM_NODE_MODULES="$PWD/.build/qualified-test-deps-20261028/node_modules" \
node scripts/ci/remote-signer-browser/launch.mjs
```

Run from the new `weebdex-platform` product root. Its one ready line contains
only ephemeral public loopback `page`, `controls` and redacted `observations`
URLs. Open `page` and `controls` as separate tabs using the documented CUA
browser surface. There is no TLS substitution: this is explicitly HTTP/WS
loopback-development evidence, **not production WSS/TLS interoperability**.
The process shuts itself down after 15 minutes; stop it with SIGINT sooner and
verify terminal cleanup before another runtime process starts.

1. On the page, open the synthetic Account connection, then click **Fill owned
   synthetic public connection link**. This explicitly labeled fixture button
   places only the owned public bunker link into the actual panel's password
   input; it is not hidden auth injection into an ordinary site.
2. Click **Review remote signer**. The input must clear. In the controls tab,
   refresh redacted observations: `connections` must still be `0`. Check the
   consent checkbox; that alone must also leave `connections` at `0`.
3. Click **Connect reviewed remote signer**. The default holds the authenticated
   public-key reply. The actual top approval Dialog must receive focus; Tab and
   Shift+Tab remain inside it. The lower setup dialog must not take keyboard
   focus while the top one is open.
4. Click **Cancel signer request and disconnect**. The top Dialog closes, focus
   returns within the lower setup dialog, an error is shown and the page remains
   `Selected: no`. In the controls tab release the held reply and refresh: the
   actual socket must close (`activeConnections: 0`) and selection must remain
   absent. Reconnecting requires filling/reviewing/consenting again.
5. Repeat a held connection, then use the separate controls tab's **Switch
   synthetic native account**. The top Dialog closes, no identity is installed,
   and releasing the late reply cannot change that. Refresh observations to
   verify transport closure; the page's synthetic account label changes.
6. Repeat a held connection, choose **Change synthetic linked revision**, then
   **Release held reply**. The actual coordinator's final native read must refuse
   the stale link, close the socket and leave no selected identity. The allowed
   handshake may include `switch_relays` before that final read; no app event is
   signed.
7. For a successful narrow handshake, choose **Reply normally**, perform a fresh
   explicit review/connect and observe `Selected: yes`. Closing setup keeps the
   session selection. Switching the synthetic account then disconnects it.

Control actions affect only in-memory fixture state. They use same-origin,
exact-Origin JSON POSTs with a 256-byte body bound, never real native APIs.
`/fixture/public-link` is the sole public-link fill endpoint; `/fixture/state`
and `/fixture/observations` are read-only finite fixture endpoints. Do not
capture raw network/event/link bytes in receipts. Record visible outcomes,
counts and source hashes; leave each unexercised case explicitly pending.

## Optional operator-controlled Playwright invocation (separate evidence)

After runtime approval, use a fresh browser context from an **already-qualified, sandboxed** browser instance:

```js
import { runOwnedRemoteSignerBrowserChecks } from './scripts/ci/remote-signer-browser/checks.mjs';
const report = await runOwnedRemoteSignerBrowserChecks(browser, {
  frontend: process.cwd() + '/.build/frontend-ux-final',
  domDependencies: process.cwd() + '/.build/qualified-test-deps-20261028/node_modules'
});
// Save only report, plus the qualified browser identity/sandbox receipt.
// Do not serialize fixture objects, browser requests or bunker links.
```

The module does not launch a browser. The operator must close the browser/launcher after it returns. Each case creates/closes its own context and owned server. All nonfixture browser HTTP requests are blocked. The actual pages' CSP and WS server only permit the one owned socket; no relay redirect/failover occurs.

Cases cover: zero sockets during review/checkbox-only consent; actual nested-dialog focus trapping; Cancel and late held response; account switch and late response; link-revision switch and late response; successful selection surviving setup dismissal, then account switch disconnecting. The failed-case receipt records the finite last phase rather than unsafe raw diagnostic strings. A focus failure is a real acceptance failure to investigate, not a reason to weaken the assertion or replace Dialog with a mock.

This is a **narrow real-browser focus/transport acceptance**, not a real-user signer interoperability certificate, full deployed-account/backend journey, full production style regression, or external publication test. The existing real HTTP F05 and focused 41 tests remain separate evidence.
