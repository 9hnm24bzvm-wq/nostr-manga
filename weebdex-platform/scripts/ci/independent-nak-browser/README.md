# Independent nak signer, actual native-account acceptance

**Source-only; not executed or accepted.** This is separate from the older
synthetic bunker/focus fixture. The independently downloaded official
`nak-nodb-v0.21.2-linux-amd64` is the sole NIP46 server. The relay never decrypts,
creates a response, or signs an event. No product code or native account/session
is substituted. The main fixture, ordinary signup/reauthentication, actual
Account page, native identity-link endpoint, and sole CUA owner remain separate.

Pinned binary SHA256:
`e2fbbdf24cda6fae99c777f11003e0b517b43ac1a40224c78fa87a051354e394`.
Its acquisition receipt is `.build/independent-signer-nak-0.21.2/acquisition.json`.
Do not redownload, compile, or install. Primary source reviewed:
<https://raw.githubusercontent.com/fiatjaf/nak/v0.21.2/bunker.go> and
<https://raw.githubusercontent.com/fiatjaf/nak/v0.21.2/main.go>.

## Boundaries

- Exactly one disposable signing identity is generated after explicit release.
  It is provided to the local nak child through its private environment, never
  argv, browser, output, or evidence. The browser's independent ephemeral NIP46
  transport key is not a second account/signing identity.
- Nak receives one literal loopback relay URL, no persisted clients, custom
  relays, inherited credentials/proxies, or default home/config. This is reviewed
  configuration confinement, **not an asserted OS network-namespace sandbox**.
  Its private Unix socket is inside a0700 temporary runtime. No IPC client is used.
- The forwarding relay binds127.0.0.1 only; exact Host/path/browser Origin,
  bounded NIP01 filters, verified kind24133 signatures, sender/recipient keys,
  frame/event/connection limits. No public application-event kind is accepted.
  It stores no events and sends no AUTH, redirects, or external addresses.
- Nak output includes secrets and decrypted requests. It stays in one bounded
  private0600 log under0700 `/tmp/weebdex-nak-owned-*`, never copied to evidence.
  Only public signing key, counters, and independently verified native27235
  proof event IDs can enter the final redacted receipt.
- One1200-second lifetime; `/usr/bin/timeout` also bounds the actual child.
  Cleanup terminates the owned process group/sockets, removes private URI/log/
  config files and clears owned key buffers. It does not claim guaranteed
  cryptographic erasure of immutable runtime strings.

## Release and secure handoff (not automatic)

1. Wait for current browser fixture cleanup and verifier source/hash release.
   Prefer a verifier-planned port1024..65535: the new main fixture can admit that
   exact WS origin before this helper exists. Finish secure native signup/email
   verification first, so the one identity's1200-second lifetime is not spent
   waiting for user setup. The helper binds that port once before key generation;
   an occupied port fails without a key or automatic replacement. Start only
   this helper, never a browser, once the native account is ready:
   ```sh
   WEEBDEX_INDEPENDENT_NAK_APPROVED=yes \
   WEEBDEX_INDEPENDENT_NAK_PORT=<verifier-planned-port> \
   WEEBDEX_BROWSER_DOM_NODE_MODULES="$PWD/.build/qualified-test-deps-20261028/node_modules" \
   node scripts/ci/independent-nak-browser/launch.mjs
   ```
   Keep its stdin open. Ready output contains only public relay origin and the
   private local file path, not the file's secret-bearing bunker URI.
2. The NEW main native fixture must be configured with exactly that WS
   origin. No production CSP or current listener is altered. Bind its resulting
   public HTTP origin once via helper stdin:
   `{"browserOrigin":"http://127.0.0.1:<actual-new-main-port>"}`.
3. The approved secure handoff uploads that exact local connection file to the
   user's private native Library (no shared directory or access grant), using
   the current Library skill. Do not read file contents into model context.
   Root delivers only the native private attachment. The user supplies the URI
   through supported browserAuth/manual secure input in the actual masked
   RemoteSignerPanel. No hidden DOM fill, clipboard injection, or tool-argument
   secret is authorized. If private ownership/input cannot be established,
   stop and report the exact blocker.
4. The user signs up/logs into the fresh mock account using normal secure UI;
   satisfy native verification/reauthentication through supported workflow.
   The actual Account UI reviews the relay and key before explicit connection.
   It may select the unlinked remote identity; then explicitly review and
   confirm the private native identity association. The server must verify
   the nak-produced27235 proof. Do not grant publisher admission, roles,
   permissions, or protocol identity binding. Do not perform a public action.
5. Verify selected public key/private link match, explicit signer disconnect,
   and native unlink/reload as approved. Record only visible outcomes and
   redacted native receipt fields, not raw bodies, URI, tokens or logs.
6. Send `{"stop":true}` or SIGINT. Require terminal redacted closed receipt,
   stopped child/listener, removed temporary runtime. Root removes the temporary
   native Library connection attachment under the approved cleanup scope.
   The expired/stopped signer cannot be revived or a second identity generated
   without further release. Preserve failures honestly; do not retry until pass.

This demonstrates only the exercised official nak version, local WS transport,
actual native-link proof and disconnect. It does not establish external relay,
production WSS/TLS, another signer's interoperability, or public publication.
