# Local-first browser client

Owner instruction8October2026,14:00UTC supersedes any implication that every reader needs an account or relay-backed library. Local reading state is the default. Publishing is optional and public, to user-selected instances that permit it.

Implementation scope: reusable browser modules plus a functional browser library client. This is not the final MangaDex-style reader-site design. Use IndexedDB for durable local chapters/series/lists/members without a signing key or network request. Explicit export/import protects against browser eviction and supports local transfer. Browser persistence is requested only by user action and is not guaranteed.

Opt-in publication uses an external NIP07 signer; never request/store a private key. Preview public fields and selected recipients before signing. Build the existing canonical chunk/root graph, verify signed event identity and server response proofs, preserve signed snapshot and per-recipient staging/commit cursor in a durable outbox, retry only by explicit user action, and never turn a local save into network traffic. A relay rejection does not undo the local save. New publication requires fresh confirmation; background relay synchronization is off.

Recovery is explicit: fetch a signed public head/objects from a pinned chosen service, validate hashes/schema/buckets and preview before a revision-fenced local replacement. Nothing may silently overwrite local edits. Multi-tab transactions serialize local changes; stale import/restore fails rather than discarding newer changes.

Verification: JS codec/model tests, cross-language fixtures against the Rust codec, real browser IndexedDB/reload/offline/multiple-tab flows, no unsolicited network/signing, explicit publication through actual PostgreSQL/strfry, retry/rejection recovery and user-safe import/export.30k/500k functional data checks are not a throughput certificate. Capacity run is interrupted and paused again for this new feature request.

Dependencies: pinned noble-curves/hashes from the official npm registry for signature/hash verification; no new identity server,framework or hosted service. Sources: https://github.com/paulmillr/noble-curves and https://github.com/nostr-protocol/nips/blob/master/07.md . Browser setup requires HTTPS or loopback secure context,IndexedDB and a compatible optional signer for publication. Local reading does not require a signer.
