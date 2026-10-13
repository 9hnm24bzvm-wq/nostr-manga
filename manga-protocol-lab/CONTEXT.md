# Reconstruction continuation: October 10, 2026

The active project is reconstructed from October 8 checkpoint26/schema52. Later October10 source was lost from the temporary workspace; do not apply its historical test results to this tree. Recovery is assistant-owned and is not waiting on a missing Library attachment.

Read docs/rebuild/STATUS.md, ACCEPTANCE-MATRIX.md and PLAN.md. Current narrow proof is in results/rebuild-*; old release documents are historical. The final combined suite, exact release build, rendered-browser completion and sustained traffic target remain separate gates.

Keep PostgreSQL/deadpool-postgres/tokio-postgres/Moka and strfry; no SQLx, Kratos or SQLite runtime. The browser saves locally, with explicit public publication only. Source must stay clean and seam-tested. No test-only public wrappers or private SQL/counter outcome assertions; external fault arrangement is allowed.

Encrypted checkpoint URLs and verified hashes are in docs/rebuild/CHECKPOINTS.json. Passwords are delivered in private recovery messages and must not be committed. Every checkpoint is read back, decrypted and member-verified. Never call an upload alone a completed backup. Never place real keys, credentials, personal data or runtime databases in a checkpoint.

No target-capacity claim is established. Feature completion precedes new heavy-load work; realistic images, measured workload proportions, durable accepted saves, backlog, resource use and failure behavior must be reported distinctly.
