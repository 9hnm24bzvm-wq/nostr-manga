# Rebuild plan and acceptance

Current source is reconstructed from October8 checkpoint26/schema52. Never inherit October10 results onto this tree. The original baseline is independently backed up at https://files.catbox.moe/9cclmj.zip (encrypted; recovery password was delivered privately to the user, not stored here).

## Slices
1. Browser durable operations: immutable publication inventory; metadata/bytes separation; stale cancellation; preference revision around signing; bounded batch upload; atomic staged streaming backups; atomic build promotion.
2. Library/projection lifecycle: prepared immutable references with transactional generation/health/authority rechecks; bounded per-author work; verified import acknowledgements independent of blocked retirements; locked retirement and grace; genuine public lifecycle regressions.
3. Read/download resources: admission before database access in HTTP and established Nostr messages; retryable outage responses; separate image byte/count capacity; body/deadline/cancellation ownership; upload independence.
4. Operator workflows: normalization/JSON ownership; private atomic artifacts; restore preflight; real CLI/process failure outcomes; safe source packaging.
5. Source hygiene: retire unused prototype/model/wrappers only after current behavior replacements pass. Format only with bounded, measured diffs. Keep current docs and historical evidence distinct.
6. Operational verification: exact final full command, real HTTP/PostgreSQL/strfry/nginx, codec vectors, large library/module interoperability, and real-image lower-load retention/recovery. Heavy-load acceptance remains open until measured independently; do not label a successful test process as achieved capacity.

## Checkpoints
Use immutable snapshot inputs, explicit file allowlist, no credentials/runtime databases, AES-256 authenticated ZIP STORE, manifest hashes/CRC/length, atomic receipts. POST once; reconcile any uncertain result before retry. Download and decrypt the returned URL, compare archive and every member. Deliver URL/password in the user's private chat. Backup completion is separate from software acceptance. Partial snapshots explicitly list which rebuilt slices they include.

## Agreed test seams
- Public browser library, backup and publication operations
- Browser flows (model versus actual-browser evidence labeled)
- HTTP/Nostr through real PostgreSQL/strfry
- Operator CLI and process lifecycle
- Public codecs using independent vectors
- Actual application-used Rust lifecycle/config/error interfaces

No private-helper tests, source-shape assertions, internal table/counter outcome oracles or test-only public wrappers. External fault setup is allowed. Red -> green one behavior at a time; do not bulk invent a test suite then implement it.
