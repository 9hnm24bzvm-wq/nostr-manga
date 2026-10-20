# Signed preview abort lifecycle controls

## Observed hosted evidence

Run `39427501479`, attempt 1, source tree `478081458afa761a27eaad69286b2b04ddb63632`, ran the upload journey independently after feature cleanup. Its first two upload checks passed. The third check failed with the fixed substep `mutation-count` and category `assertion`. The observer retained three mutations and one signed preview GET with HTTP 200 followed by a native `aborted` terminal. The producer capture was structurally valid but empty, so correlation was incomplete. No committed-original, replacement or reload-preview success was established.

These facts do not establish a mutation-count mismatch, a bad response body, an application defect or a browser defect. A status of 200 proves receipt of response headers, not successful body consumption. The observer is deliberately strict about aborted signed requests.

## New local controls

`tests/ci/test_public_upload_preview_abort_lifecycle.mjs` executes the current transport, current preview consumer and current lease with finite in-memory primitive/stream adapters. Observer events and signature verification are explicit stand-ins. It does not execute Svelte reactivity, real signing, network, native HTTP, a browser or image decoding.

Eight controls establish:

1. Lease disposal while the signed read is pending does not abort its transport. A late successful Blob allocates no URL.
2. Clearing the lease for dragging does not abort its transport. A late successful Blob allocates no URL.
3. A same-key metadata-only update reuses the pending lease operation, without extra request or abort.
4. A native failure can be observed before the transport catch records its refusal. The empty immediate producer snapshot is transient in this controlled ordering, not proof that this happened in the hosted run.
5. A transport body-read refusal records before its final cleanup abort. Both event orderings remain fatal to the observer, preserve known HTTP 200 and clean up their timers.
6. The exact current `completeWrites` and `until` functions can fail with `signed_native_failed` under the label `mutation-count` even when three actual observed mutations satisfy the requested count. The health guard throws before that count comparison.
7. An explicit unconditional-abort counterfactual is detected on an otherwise successful read. Current successful full-body consumption does not abort.
8. The exact historical pre-diagnostic transport (SHA-256 `9418845c867fd7fd0706756197900221e3cd73cd87a48133d5ed3ff6ef0d3056`) also keeps successful completed reads un-aborted and aborts an unfinished read after owner invalidation. This is not evidence that owner invalidation occurred in the hosted run.

The existing capture already waits up to sixteen 25 ms intervals for expected private event IDs, with a 1,000 ms outer deadline. These new ordering controls do not justify increasing that wait or treating an uncorrelated abort as success. Lease disposal does not pass an AbortSignal through the component loader, so a direct lease-driven cancellation is not supported by the inspected implementation.

## Scope and remaining uncertainty

Only tests and this document are added. No production behavior, reporting schema, request count, lifecycle allowance, privacy limit or success criterion changes. No abort is ignored; no retry is introduced.

The hosted initiating cause is still unknown. Source-backed possibilities include an incomplete transport failure (header/media/size/body/owner/identity/deadline), browser/network cancellation independent of the JavaScript controller, or unavailable diagnostic correlation. The report cannot select among them. A further proposal should separate observer-health failure from write-count progress with closed bounded diagnostic facts, and investigate native-terminal versus producer-record ordering without raw URLs, headers, IDs or error text. This is a diagnostic proposal, not a proven product correction.

## Qualification

With the exact historical transport supplied through `WEEBDEX_UPLOAD_REFUSAL_BASELINE_SOURCE`, running Node with `--experimental-vm-modules --test` on these four files passed 137 tests, zero failures or skips:

- `test_public_upload_preview_abort_lifecycle.mjs`
- `test_public_upload_request_recovery.mjs`
- `test_public_upload_refusal_qa.mjs`
- `test_public_upload_observer_boundaries.mjs`

The new tests alone passed 8/8, including the historical-source control. No hosted run was launched for this work.
