# Instance upload and media boundary

This module persists instance chapters and calls the original canonical publication
service for new uploads only when the explicitly configured signer/store and
authenticated operator identity binding authorize it. Disabled publication returns
truthful instance-local status. Original protocol chapters remain immutable;
metadata edits are explicit instance annotations and signed page replacement
requires the original deletion/new-upload workflow.
The chapter reader never retrieves third-party originals or uses the image proxy.

API mutations require a verified persisted session/API principal, the source's
permission grant, same-origin browser mutation validation and owner-scoped upload
session rows. CatalogRepository is the sole catalog writer; a caller-owned SQL
transaction atomically publishes catalog metadata, ordered page associations and
session state. The exact ordered file set is checked before publication. A repeated
commit succeeds only with the identical canonical payload digest.

Originals are streamed to an exclusive temporary file while hashing. Header,
extension, declared MIME, real dimensions and complete bounded decode are checked.
JPEG/PNG/WebP and GIF pages are supported. GIF frames are decoded individually with
a 100-frame/20M-aggregate-pixel ceiling. GIF covers are explicitly unsupported.
Covers get 256/512 lossless WebP variants at upload time. Static chapter pages may receive real smaller bounded derivatives. The catalog
writer advertises optimized variants only as a validated complete ordered set.
Unsupported GIF animation is never flattened or partially optimized. Empty
optimized arrays are omitted because the retained JS reader treats arrays as truthy.

Bounds: 32MiB/file, 500MiB/session, 500 files/session, 5 files/request,
20M decoded pixels, 96MiB decoder allocations and two decode workers. File byte
streams have a 120-second deadline; multipart field/metadata waits are bounded.
User names never form object paths; storage keys are generated 64-character hex.

A drop guard unlinks cancelled/uncommitted files. Before COMMIT a durable key-only
journal is written. An ambiguous COMMIT retains the original until recovery can
verify the database row while holding the same object advisory lock. Durable SQL
cleanup records retry object unlink. `recover_storage` runs after startup schema
verification; ordinary upload mutations also retry recovery. There is no extra
worker lifecycle. Soft-removed chapters retain recoverable originals and their
public media join rejects removed records.

GET/HEAD media delivery checks object size, sets actual MIME/length and strong
content-digest ETag, supports one byte range, conditionals and If-Range. Request and
read-admission permits remain owned by the streaming body and release on completion
or disconnect. Session previews and unpublished covers remain authenticated and
private/no-store; public source-linked media revalidates current original authority before serving.

The separate inline discussion-image proxy vets every redirect, all DNS addresses
and port, pins DNS results, disables ambient proxies and credentials, bounds
redirects/network time/8MiB response bytes/concurrency, validates actual decoded
image content and unlinks its temporary file when the response body drops. It is
not a chapter origin, federation path or lasting storage authority.

Verification status is recorded separately. Source implementation and a passing
compile do not establish real multipart/PostgreSQL/HTTP round trips, ownership
regressions, signed protocol publication or unchanged-frontend UI behavior.

Signed-publication commands use durable original timestamps and input hashes,
retaining canonical IDs across uncertain native commit outcomes. Accepted-pending
receipts are owner-scoped and persisted with the upload session; replay of an
identical commit returns the truthful receipt or its subsequently visible chapter.
Repeated identical original hashes remain separate chapter page positions.
