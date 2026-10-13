# Account replacement foundation

The repository and authenticated HTTP metadata endpoints are implemented and regression-tested. Dedicated library admission, ordinary author deletion, approved account replication and reader compatibility are now implemented in their separate workflows. See [API index](API-INDEX.md) for current entry points. Enrollment beyond explicit operator admission remains a pending policy choice.

## Authority

Local account admission is explicit and default-deny. Receiving an author's events from a peer does not admit that author to post locally. An operator may change admission; each change increments an epoch, including disable/re-enable, so an earlier permit cannot revive after an ABA transition. HTTP integration must bind the NIP-98 signer to the author and separately authenticate admission administration.

The repository serializes head updates and admission changes on the author's permission row. Unrelated authors do not share that row. Signature, bounded serialization and profile JSON validation happen before the short transaction. No filesystem or network call occurs inside it.

## Supported events

This path currently admits profile kind 0, follow-list kind 3 and relay-list kind 10002. Relay hints do not trigger fetches or approve federation partners. Experimental library kind11999 is deliberately rejected on this metadata path; its dedicated library workflow verifies local object durability and grants. Publication kinds retain their own lifecycle.

The head winner uses Nostr ordering: later created_at, then lexicographically lower event ID at equal timestamps. Repeating the current event is idempotent; an older losing event has no effect. A retained winner watermark is separate from the nullable current pointer so ordinary deletion can prevent resurrection by stale replay.

## Projection

The transaction creates the new hosted event and import outbox entry, hides the previous event, and queues an explicit predecessor deletion. Cached immutable bodies still require primary-database visibility. New import precedes predecessor deletion. Obsolete versioned jobs are skipped. Backend batches separate two events at the same replaceable address so a scan is not expected to find two mutually replacing events.

A newly accepted database head is not a promise that strfry has finished. Projection completion remains an explicit boundary. A crashed process can leave work pending for retry. The separate library workflow retains predecessor objects until backend retirement and its configured read grace complete.

## Tests and remaining gates

The account fixture exercises 32 competing same-timestamp changes, deterministic tie selection, a warmed predecessor cache, exact retry, stale replay, admission revocation/re-enable, author mismatch, invalid profile content and rejection of the unverified library path. It drains the outbox through the actual installed strfry and independently scans for exactly the winning profile.

Separate fixtures now cover ordinary deletion, approved account imports, reference-retaining library admission and process/database restart. Configurable self-registration remains an owner choice. Broader external conformance and target deployment capacity are unqualified; do not equate passing reference fixtures with universal production support.


## Initial HTTP interface

- POST `/admin/accounts/{pubkey}` with `{"allowed":true|false}`: operator NIP-98, body binding and persistent replay protection. No implicit open registration.
- POST `/account-metadata`: the admitted personal key authenticates with NIP-98 and signs the event itself. A successful new or duplicate current event returns its ID and whether projection is complete. HTTP 202 means durable admission with projection still pending. HTTP 409 means the event lost replacement ordering.
- GET `/accounts/{pubkey}/metadata/{kind}`: public current head plus projection status under the instance read policy; no private library interpretation. Ordinary event retrieval remains at `/events/{id}` and preserves its projection boundary.

Revoking posting permission does not delete already-public metadata. Library kind11999 remains unsupported at these metadata endpoints. Routes and experimental kinds are not a finalized interoperability standard.

Source rules: [NIP-01](https://github.com/nostr-protocol/nips/blob/master/01.md), [NIP-65 relay lists](https://github.com/nostr-protocol/nips/blob/master/65.md), [NIP-98 HTTP authentication](https://github.com/nostr-protocol/nips/blob/master/98.md).
