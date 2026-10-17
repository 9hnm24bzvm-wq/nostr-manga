# Public signed upload recovery

The later [signed chapter interface](public-signed-chapter-writes.md) supersedes
the metadata/claim/removal limitation below. Published image revision remains
unavailable; this document retains the earlier upload recovery boundary.

This is a new implementation on recovered source `e7588f7d`. It is not a byte
restoration of the lost late upload adapter or a claim of MangaDex parity.
The earlier blank-begin/autosave recovery is retained in commit `3e46ddcb`.

## Activation and authority

`PUBLIC_MANGADEX_CATALOGUE` is the existing selected-instance build flag. Unset or
`0` leaves this new upload overlay disabled; only exact `1` enables it. The Vite
integration runs the public upload transform after the immutable-source repairs
and public catalogue transform. Original frontend files are never rewritten.
For an intended instance, explicitly build with `PUBLIC_MANGADEX_CATALOGUE=1`
through `scripts/vite.platform.config.mjs`, then configure that instance's
`public_origin` / `PLATFORM_PUBLIC_ORIGIN` to the exact browser-visible origin.
The same-origin public transport base is `/api/v1`; it is not the original
protocol relay origin advertised in publication capabilities.

The user explicitly selects a signing identity in Settings. The adapter captures
its generation, proves the configured publication service, and loads the signed
bound account context. It does not create or bind an account and does not promote
native-cookie authority. Admission, native identity binding, native grants and
operator/authorized-group publication authority remain separate checks.
Selecting a group does not grant publication authority. Current bound user roles
control locked-group UI affordances; the backend is authoritative.

## Normal recovered path

- Signed account context: `GET /api/v1/nostr/account-context`.
- Official public begin/current/image/delete/approval/commit routes remain under
  `/api/v1/upload`. Their actual success status is preserved as HTTP200.
- Exact owner snapshot and optimistic staging:
  `GET` / `PATCH /api/v1/nostr/uploads/{session}`.
- Exact owner-only original preview:
  `GET /api/v1/nostr/uploads/{session}/files/{file}/preview`.

Every new public request signs the configured exact absolute URL, method, service
and immutable raw-byte digest using NIP-98. No cookie, retry, redirect or original
relay-response-proof header is substituted. Original multipart bytes are serialized
before signing. Ordinary public HTTP responses are streamed under explicit limits.
The backend retains the existing private admission capability and same-transaction
binding/grant checks over PostgreSQL/deadpool/tokio-postgres.

A blank begin acknowledges only the returned server draft and file order. Retained
local language remains dirty and is saved after it is valid. Autosave captures the
actual draft/order before awaiting and acknowledges only that captured value;
newer edits remain dirty after success or failure. Exact public UUID receipts gate
page acceptance. Resume hydrates current original files, private metadata/order and
actual group views, without replaying an image POST.

Private preview reads verify actual size, MIME, digest and decoded dimensions
against the exact owner snapshot. A component-scoped Blob URL lease keeps local
previews until validated replacement, rejects stale owners, and releases URLs on
removal/unmount. The original ImageLoader is instantiated only for an actual Blob
URL, keyed when that URL changes; missing previews display a loading/error state.

Commit checks the real terms checkbox, actual approval decision and acknowledged
private metadata/order. A receipt is not a readable chapter: reader navigation is
available only after a current public catalogue view has matching actual provenance,
version, target and page data. An uncertain dispatched commit or accepted-but-
unavailable presentation persists an exact-session read-only fence. The UI clears
stale chapter fields and exposes an explicit saved-receipt check. It never replays
that commit or fabricates publication identity. The recovered backend currently
uses HTTP200; synthetic known202 tests qualify response honesty only.

## Explicit limitations and next proof

The signed recovered UI disables existing-chapter metadata edits, claims, chapter
deletion and existing-chapter replacement. Single-page replacement has no public
command and is explicitly refused before dispatch. Remove then attach is available
as two explicit actions. This is a functional boundary, not an equivalence claim.

The frontend applies conservative 25MiB-per-image and existing importer limits;
the backend also applies native dimensions, session and publication limits. A
staged image can still be rejected at commit by original publication MIME/blob or
authority rules. Private previews retain both outer request admission and inner
stream permits; very small configured concurrency may return busy.

Source seams use explicit fake signers/transports and real SHA-256, never fabricated
results described as end-to-end evidence. Rust compile checks are compile-only.
Genuine disposable PostgreSQL/HTTP tests, frontend strict checks/build and a natural
browser upload through commit/resume remain separately required. No prior lost
runtime artifact or acceptance result qualifies this recovered source.
