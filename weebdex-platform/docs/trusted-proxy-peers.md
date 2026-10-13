# Client identity behind an explicitly trusted reverse proxy

Default `trusted_proxy_peers:[]` uses only the actual socket peer. Untrusted or
missing transport never gains identity from X-Forwarded-For, Forwarded, X-Real-IP,
cookies or a caller-supplied request extension. Direct standalone behavior is
unchanged. IPv4-mapped IPv6 is normalized to the same IPv4 limiter identity.

For the supplied single-edge nginx template, use the reviewed runtime examples:

```json
"trusted_proxy_peers": ["127.0.0.1"],
"trusted_proxy_hops": 1
```

Nginx must overwrite X-Forwarded-For with `$remote_addr`; it must not append an
arbitrary client-supplied chain. Only an actual transport peer in the explicit
list can supply identity. Its header must be one field with one valid unicast IP
for this policy. Duplicate headers, invalid/empty IPs, hostnames, ports, bracketed
IPs, unspecified/multicast addresses and extra hops fail400 before a handler.
A trusted local probe with no forwarded header retains its actual socket identity.

Trusting loopback assumes only the reviewed proxy/local administrator can reach
the private listener. Other local processes sharing that address are within this
trust boundary. Keep private listener/network access restricted. This source
implementation does not change OS/network permissions or install a proxy.

For a deliberately reviewed multi-proxy chain, set an exact hop count1–8 and
list all actual immediate/intermediate proxy IPs (at most32unique addresses).
The single XFF field must be `[client, upstream_proxy...]`, with exactly that
many entries. Every entry after the client must be a listed proxy, and the actual
socket must be a listed final proxy. All proxy stages must independently strip
untrusted headers and build that exact policy. The supplied nginx template is
single-hop only; do not use it unchanged for a multi-hop deployment. No CIDR,
wildcard, inferred public/private network or arbitrary leftmost-XFF trust exists.

Central middleware installs ClientPeer after host/origin validation. Analytics
uses its rate identity, so distinct validated clients behind nginx do not share
the proxy's120/minute bucket. Visitor-hour deduplication remains separate.
Global request admission and original bounded read pools remain independent.
Native auth inspection found intentional global, browser-cookie, identifier and
email budgets, not a ConnectInfo-derived peer limiter; those global budgets are
preserved and are not reinterpreted as per-client quotas. Original-module policies
are unchanged by this platform-only middleware.

Authored acceptance source covers the real middleware (trusted/untrusted,
spoofed/duplicate/malformed headers, missing transport, IPv6 normalization and
exact multi-hop checks) and the public Router/PG analytics seam (one exhausted
client does not exhaust another, untrusted forged headers cannot rotate identity).
No runtime or compiler acceptance is claimed until the fresh-audit gate clears.
Actual nginx request forwarding and operational rate behavior still need E2E.
