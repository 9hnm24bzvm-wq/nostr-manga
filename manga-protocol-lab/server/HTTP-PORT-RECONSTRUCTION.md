# Reconstructed native original-image HTTP support

This code was reconstructed after the unpublished worktree was lost. It has not
been compiled or executed in the replacement environment, which lacks Rust.
Earlier test results do not apply to this reconstruction.

GET range responses use an already verified, budgeted snapshot. HEAD uses the
same policy and storage verification, ignores Range, releases the snapshot and
creates no body producer. If-Range falls back to the full response because this
route does not emit validators. Existing no-store and nosniff protection remains.
Compatibility image routes forward method and headers into the guarded route.

Tests cover bounded single ranges, ignored multipart, large decimal clamping,
unsatisfiable responses, HEAD equivalence and immediate lease release, If-Range
fallback and lease release after partial-response disconnection. They are defined
but unrun. Routed authorization and PostgreSQL-backed HTTP acceptance still need
execution, as do native Windows/macOS tests. This is not nginx throughput parity.
