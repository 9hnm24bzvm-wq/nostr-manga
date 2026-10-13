# Reader image derivatives and forum previews

Status: source implemented; compilation, SQL execution, API and browser tests deferred to the final implementation gate. Original protocol source and all attachment frontend files remain unchanged.

## Reader quality capability

New native uploads validate and retain exact original image bytes. During the same two-permit bounded blocking codec job, static images may receive a separately stored smaller derivative: JPEG quality72 for opaque images, lossless WebP for images with alpha. Aspect ratio is retained, no upscaling occurs, and derivative dimensions are bounded to1280×3000 (3.84M pixels). A derivative is retained only when its actual encoded byte count is strictly below the original; digest and size are verified again by the catalog-owned writer.

Original decode limits remain32MiB/file,20M original pixels and96MiB decoder allocation, with two concurrent jobs. Derivative thumbnails have a separate bounded allocation of at most3.84M pixels; source originals can remain larger. Existing GIF frame validation and original animation bytes remain unchanged. GIF optimized re-encoding is deliberately unsupported, so a chapter containing a GIF has no optimized capability.

The instance tables upload_file_derivatives and chapter_page_derivatives associate derived objects with actual original pages. They do not expand or change signed protocol original-media schemas. Editing a chapter without replacement copies original and existing derivative object associations; cancelling/deleting upload files cascades derivative references. Replacing pages discards the prior derivative set and installs only a complete new one. Orphan cleanup includes both derivative reference tables and retains the existing durable uncertain-COMMIT journals.

The catalog writer installs metadata.data_optimized only when every original page has a persisted derivative at the same position, with validated dimensions, distinct digest and strict byte savings. Partial sets are never advertised. The existing reader toggles then select actual original or optimized names/dimensions. If any page has no qualifying smaller encoding, data_optimized is absent and the original source truthfully disables the unavailable optimized control.

## Forum preview images

GET and HEAD /og-image/manga/{id}, /og-image/chapter/{id} and /og-image/group/{id} now generate local1200×630 SVG images from persisted public titles/metadata. The retained forum-thread page consumes these exact paths. Draft/deleted manga, deleted/unavailable chapters and deleted groups return404. An anonymous preview does not expose private records even to a logged-in owner.

SVG contains only fixed rectangles and escaped text: no script, foreignObject, embedded URL, image reference, external resource, remote fetch or raw HTML. Text and line counts are bounded. Responses use image/svg+xml, nosniff, restrictive sandbox CSP, SHA-256 ETags, matching HEAD Content-Length and public revalidation. Every read reacquires original protocol admission and a request permit, checks current visibility, and handles conditional304 only after that check. Entity edits produce a new content ETag.

## Final acceptance checks

- Upload a large static image; verify original byte digest remains unchanged and actual optimized bytes are smaller and decodable.
- Publish an all-static chapter; toggle Original/Optimized, retain exact page count/order and show expected dimensions.
- Include an unsupported animation or already smaller input; verify data_optimized is absent, never a partial or duplicated-original set.
- Copy/edit/reorder/remove/replace pages and verify matching original/derivative positions, version refresh and cleanup ownership.
- Interrupt stream, validation, SQL commit and download; verify durable guards and bounded permits recover.
- Fetch each visible forum preview URL, HEAD and If-None-Match; verify correct image body/type/length/304.
- Preview title containing XML/script-like text; verify escaped text, no executable or external resources.
- Delete/unpublish an entity after an initial preview; conditional requests must return404 and never cached private content.

Rendered SVG content alone is reused through the bounded shared Moka cache. Its key hashes the freshly authorized title/subtitle/description tuple and renderer version; every request performs current source-policy eligibility SQL before lookup, so authorization and entity DTOs are never cached.

Native signed upload integration sends exact ordered original descriptors through the original canonical encoder and publication runtime when explicitly configured. Actual original hash names are stored by chapter position, preserving repeated identical pages. Optional instance derivative names stay separate. Authenticated accepted-pending receipts are persisted with the native commit for truthful idempotent retry; public source reads remain fail-closed.
