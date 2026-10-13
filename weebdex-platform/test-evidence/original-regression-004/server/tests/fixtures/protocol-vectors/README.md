# Independent public codec vectors

These literal fixtures are consumed through application-used Rust interfaces:
`series_metadata::parse`, `manifest::{series,chapter}`, Nostr event verification/signing,
and compatibility UUID/calendar conversion. The input bytes and expected outcomes do
not call the Rust implementation to construct their oracle.

`generate.py` uses pinned Python `cbor2`, `coincurve`, and standard-library
`json`, `hashlib`, `uuid`, and `datetime`. Run it from the project root with
`PYTHONPATH=vendor/pydeps python3 server/tests/fixtures/protocol-vectors/generate.py`.
The Nostr secret integer 1 is a deliberately public synthetic fixture, never a
deployment key. No application credentials or private histories are included.

All CBOR map keys here are text. For this set, the independent encoder's canonical
ordering agrees with the protocol's lexicographic encoded-key ordering. Mixed-key
profiles would need a separate conformance vector.

The gzip response test contains independently generated Python `gzip.compress`
bytes directly in its test. It checks the caller-used response interface, headers,
exact wire bytes and consumption of an externally supplied lease. It does not
claim that a compressed-response round trip proves real HTTP deadline behavior.
