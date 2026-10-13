# Historical intermediate candidate

`relay-release-candidate.rs` preserves the release-before-socket-send candidate
from the two-slow-reader red/green slice. It predates the subsequent owned-SQL
cancellation fence and is not current production source. Current source is in
`server/src/relay.rs` and `server/src/relay_dispatch.rs`; current evidence is under
`results/rebuild-admission/`. This archive exists only to preserve provenance.
