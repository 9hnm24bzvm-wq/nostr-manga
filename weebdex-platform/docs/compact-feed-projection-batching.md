# Compact feed request-local projection reuse

## Evidence and current boundary

Verified hosted run38584846100 returned HTTP503 and the closed category
`query-canceled` in both `ui-feed` and `ui-reader`, after pagination seeding and
Spanish preference setup passed. That category establishes query cancellation;
it alone cannot distinguish the configured statement timeout from an explicit
cancellation. Repeated projection work is a source-established amplification,
not yet a measured sole cause of the hosted failure.

The former100-row mapped expression sharing one parent/group contains101 full
parent DTO evaluations and201 group DTO evaluations, including nested parent
relationships and the top-level map. These are structural counts for that
specific cardinality, not CPU, duration or database call-count measurements.

The candidate materializes only distinct parents and groups referenced by the
already selected page. It reuses each parent value in both chapter relationships
and the top-level map. Groups inside a full parent remain the unchanged complete
parent projection, including groups outside this selected language or page.
For that shared-parent/group shape, expensive parent construction is bounded to
one and group construction to two (one inside the parent and one for chapter
relationships). JSON copying for each returned chapter remains intentional.

No cross-request cache, application dependency, private-library authorization
change, extra request, background task, larger timeout or raised pool limit is
introduced. PostgreSQL/deadpool and all existing filter/ordering/count semantics
remain in place. MATERIALIZED CTEs and the original STABLE DTO helpers share one
statement snapshot, so source withdrawal cannot race across separate cache
queries. The original chapter visibility predicate remains in the overload too.

## Migration and restricted-runtime compatibility

The overload is explicit platform migration0087. All historical platform and
original protocol migration bytes remain unchanged. The existing one-argument
chapter projection is unchanged; an executable source-equivalence test pins
every field and predicate of the new overload to it, with only parent/group
relationship expressions replaced by same-statement projected arguments.

Pending PR28 already reserves0086. This branch carries only its exact unchanged
`0086_visibility_policy_preparation.sql` table migration and registration from
reviewed commit04e4edc, SHA-256
`04d617d8836d47d7ab572c03ac7dc2e3a5b362415bffe34f99d94d3ed07b87b6`.
This is a prerequisite, not integration or verification of PR28's runtime
visibility/attestation lifecycle. The ordered manifest ends85→86→87. Do not
apply87 using an85-only manifest, backfill/skip86, or replace PR28's approved86
bytes. Later PR28 integration must retain this same86 once and append87; its
migration manifest must not overwrite the added87 registration.

Run explicit owner-role platform migration before runtime startup, following the
restricted-runtime deployment procedure. The new overload is SECURITY INVOKER
(the PostgreSQL default); serving performs no DDL and grants no privileges.
After migration, reapply the reviewed function privilege policy so PUBLIC has no
function access and the application runtime can execute the new overload. The
new signature is `platform.catalog_chapter_json(text,jsonb,jsonb)`; the operator
may revoke PUBLIC EXECUTE on that signature and grant it to the existing runtime
role. No new credential, role, table-writing permission or elevated runtime role
is required. PR28's separate owner-prepared source-policy attestation still has
to be integrated and verified under its own procedure.

## Reproduction and regression

Use `bash scripts/run-feed-projection-tests.sh` with the repository's approved
Rust toolchain and pinned PostgreSQL prerequisites. It delegates to the existing
fresh disposable loopback PostgreSQL runner and its cleanup. No installed server
is silently substituted. The new native test has a finite180-second bound and
asserts the unchanged5s statement and1s lock budgets.

The test creates505 synthetic Spanish DTO rows,10000 metadata-only titles,
shared and multiple groups, a full-parent group outside the selected language,
and a second parent with no groups. It runs the frozen former query once and
records success or actual query cancellation, then requires genuine HTTP200
from the new path. If the former query succeeds, complete first-page JSON must
match. Independently, sampled full DTOs match the unchanged canonical projection
on every nonempty page; all505 IDs must be reached exactly once, the native
bilingual506 case is retained, maps equal nested parent relationships, and
mapped/unmapped, ascending, distinct-title updates, empty/zero-limit, missing
foreign-key/route, soft-deletion and source-withdrawal cases are checked.
Stale parent arguments must still fail the original current visibility predicate.
Only an actually executed native test emits duration measurements.

Local Rust/unit/type verification passed; real PostgreSQL execution remains
unrun. The unchanged pinned official bootstrap was attempted after authorization,
but its first official Debian build prerequisite returned HTTP403. A subsequent
poll reported cancelled approval review; no terminal supervisor receipt was
written and no continuing owned download/build process was observed. No mirror,
retry, alternate acquisition route or fabricated native result was used. Hosted
browser reproduction and before/after performance measurements remain required.
