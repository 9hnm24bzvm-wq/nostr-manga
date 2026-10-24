# Signed chapter command uncertainty fence

## Scope and confirmed defect

This is a focused successor to the frozen signed-editor affordance repair
`c392b9d6c6165a474236a3d81267f00f5b1a3736`, not a change to its authorization
rules or native endpoints. The earlier component-local pending state was not a
persistent replay guard: a generic chapter reload cleared it and a remount lost
it. Actual mounted Svelte plus the actual controller reproduced the defect for
update, claim and removal, after both reload and remount. Native update version
checks, already-owner claim rejection and local removal-report deduplication do
not constitute an exact command receipt or universal replay safety. In
particular, the removal handler can invoke independent source deletion again
following an already-deduplicated local review request.

## Persistence and certainty boundary

The new chapter-only sessionStorage record is keyed by captured origin, service
key, catalogue namespace, signer, account and chapter UUID. It fences every
chapter action in that exact scope. Its closed record contains the action,
parent UUID, observed version, exact removal effect and a SHA-256 digest of the
captured intent and signed wire request. It contains no chapter title, volume,
number, group display text, signature, private key or raw request body. Existing
upload-publication and replacement recovery namespaces remain independent.

The record is validated and synchronously persisted before dispatch, with
checks before and after asynchronous hashing. Invalid or unavailable recovery
storage fails closed without overwriting an existing marker. Removal of the
marker requires the exact record installed by this in-flight owner; changed
records or failed storage operations do not permit replay. No automatic retry,
manual reset, endpoint, or generic-read reconciliation was added.

Only the transport's actual typed known-unsent error, a complete typed 4xx
refusal, or an exact validated direct success can clear this owner’s record.
A generic exception with an `outcome` property cannot manufacture certainty.
Unavailable response bodies, 5xx, malformed or contradictory replies, interrupted
views and changed captured scope remain fenced. A complete 4xx is not silently
retried; the user must deliberately submit another eligible action.

These are same-tab, same-origin browser-session safeguards. They survive
component remount and ordinary page reload, not an asserted cross-tab lock or a
browser-restart durability guarantee. They do not replace backend authorization,
server idempotency or user-controlled browser storage. Clearing browser storage
or ending its session is not a supported reconciliation operation.

## Exact responses, without invented attestations

- Update 200: exact chapter and unique parent, next version exactly observed
  version plus one, requested metadata after native Unicode whitespace trimming,
  and exact requested group identifiers.
- Claim 200: exact chapter and unique parent, version greater than the observed
  snapshot, plus the captured account as both uploader attribute and unique user
  relation. This is not a fabricated submitted-version compare-and-swap.
- Removal 202: exact signed route/account/effect context, closed existing body,
  matching local/source status headers, valid local request UUID, and a matching
  source request identifier header when the contract provides one.
- Removal 204: captured explicit moderator-hide intent and existing exact
  instance/source status headers, with no response body or source request ID.

All results remain bound to the original live owner and its service/account
checks. These public write responses do not carry a signed response-proof event;
this repair validates the direct response to the exact signed request, not a
new cryptographic receipt. Removal replies do not contain chapter/account/version
fields, and this repair does not invent them or claim atomic version proof.

A 202 whose sourceDeletionStatus is `unconfirmed` confirms only the local review
request. The UI shows that acknowledgement while retaining the command fence.
A fixed `local-review-accepted-source-unconfirmed` phase preserves this distinction
across reload/remount without retaining chapter text. Another removal cannot
re-enter the independent source operation. Known source statuses with complete,
matching existing receipts retain their previous behavior.

## Remaining backend limitations

1. Claim and removal accept expectedAccountId but no expectedVersion mutation
   precondition. Their observed frontend version is not an atomic transaction
   fence. A future focused backend change may add optional, transaction-enforced
   expectedVersion checks without changing this repair's claims.
2. Generic public chapter reads cannot reconcile an unknown command, including a
   locally accepted removal whose independent source outcome is unknown. Exact
   source-request receipt reconciliation is still needed for supported recovery
   from that partial outcome. No unsafe reset is offered here.
3. Session storage is not a server idempotency journal or a cross-tab lock.

## Verification and immutable source

Finite verification uses the actual compiled Svelte 5.55.1 component, TypeScript
5.9.3, Node 24.19.0 and JSdom 27.0.1, with explicit finite service/signer/HTTP and
browser-storage stand-ins. The dedicated pending suite covers 63 cases; the
broader editor/upload suite reports 478 pass, zero fail and two explicitly
optional older-source controls skipped. The exact c392b9d component and controller
fail all eight selected replay/partial-202 behavioral controls using this same
harness; they do not fail because of imports or unavailable methods.

The approved stored-uploader DTO source remains byte-identical to c392b9d. Its
nine Rust DTO unit tests passed offline under the unmodified reviewed process
owner, with verified clean termination and no survivors. A preceding warm compile
used a nonstandard wrapper and is explicitly not counted as the qualification
receipt. The subsequent reviewed-owner run used that exact compiled source cache.

This lane did not run a browser, service, live HTTP/database, publication or an
integrated strict/build gate. The 676 attachment files plus three preserved
frontend additions remain unchanged. Raw execution output stays private;
sanitized result summaries and hashes are recorded separately. This document
supersedes the earlier component-recovery description, not its frozen history.
