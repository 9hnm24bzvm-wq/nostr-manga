# Compact feed correlation planning boundaries

## Reproduced failure and evidence limits

Run38626003249 returned compact-feed503/query-canceled in both browser batches,
although its SQL-only native gate passed. That gate left the migration's trivial
SQL source-visibility stub installed. Real startup installs the canonical
PL/pgSQL source policy. The505 compact chapters are native media publications,
with NULL protocol source IDs; non-NULL event-ID lookup indexing is therefore
not an established cause of this compact failure and is unchanged here.

A separate owned PostgreSQL18.6 fixture reproduced the cancellation under the
unchanged5s statement/1s lock limits, after installing the real policy. EXPLAIN
showed a stale-statistics nested-loop plan scanning10,001 eligible parent titles
505 times, discarding5,050,000 join pairs. The matched stage took about4.72–4.78s
with the real policy, versus about0.90s with the SQL stub. The complete old feed
then received PostgreSQL's actual “due to statement timeout” cancellation.
These local errors distinguish that reproduction from an explicit cancellation;
the original hosted closed category alone still does not distinguish them.

Related amplification existed in the chapter overload's parent-visibility
EXISTS and in the chapter branch of canonical thread visibility. The former
could discard10,000 unrelated parents per row; the latter could scan/hash the
whole chapter catalogue for each point visibility check.

Background statistics subsequently changed an old full-query observation to
about47ms. That is why a warm or automatically analyzed rerun is insufficient
acceptance evidence. The regression explicitly keeps only its fresh disposable
parent/chapter tables unanalyzed. No production statistics policy is changed.

## Narrow implementation

The matched query uses a correlated LATERAL parent point lookup with OFFSET0
before applying its unchanged public filters. The zero offset is a deliberate
query-planning boundary, not pagination or an omitted result.

Additive migration0088 changes only two existing SQL function bodies:

- The three-argument chapter projection retains every field and every authority
  predicate, with OFFSET0 preserving its correlated parent EXISTS.
- Canonical thread visibility retains all four entity branches and all their
  predicates. Only the chapter branch gains a correlated parent lookup and an
  EXISTS boundary preventing an all-chapter hashed subplan.

Source-equivalence tests normalize only these planning boundaries and require
exact equality to the previous canonical bodies. All existing migrations,
including reviewed prerequisite86 and overload87, remain byte-identical. The
manifest appends88. Existing SECURITY INVOKER behavior, function signatures and
privileges are preserved; use the explicit owner migration procedure before
runtime schema verification. No runtime DDL, new grant, dependency, cross-request
cache, pool adjustment or timeout change is introduced.

## Verification

The retained native runner is scripts/run-feed-projection-tests.sh. The test now
applies the previous87 schema, all52 original migrations and the actual installed
runtime policy before measuring the frozen previous query. It then applies88
and calls the real HTTP router. It requires an actual parent-PK EXPLAIN, no
statistics rows for the two deliberately unanalyzed fixture tables, and complete
canonical100-row first-page plus map equality even if the old aggregate cancels.
The existing505-ID pagination, bilingual506, duplicate/shared parents/groups,
empty/zero-limit, unavailable rows, missing parents, soft-deletion and source
withdrawal cases remain. Original versus updated thread visibility is compared
for manga, chapter, group, user and missing identities across those transitions.

Independent SQL-only candidate experiments observed complete100-row execution
at about124–141ms with the real policy and exact full first-page JSON equality.
These are bounded fixture observations, not a general throughput or production
latency claim. Genuine browser-native-media reproduction, independent review,
and the executable full native regression are separately recorded acceptance
gates; no hosted rerun or live-preview fault injection is part of this patch.

### Local acceptance result

The strengthened real PostgreSQL regression passed on three fresh disposable
fixtures. The frozen previous query received actual statement-timeout cancellation
at5001ms in each; candidate real HTTP first pages took190ms,172ms and187ms.
The final run completed in18.97s, with zero statistics rows, a parent primary-key
lookup, and maximum per-node reported discarded pairs of1 (PostgreSQL's per-loop
average multiplied by Actual Loops). A deterministic helper test pins the
10,000×505=5,050,000 counting case. Full canonical100-row/map equality and every
retained505/506/visibility assertion passed in each complete run.

Owned supervisor receipts and fixture cleanup receipts confirm exit0, no deadline
or forced-cleanup signals, no surviving owned processes and PostgreSQL stopped. The
first development attempt exposed an existing user-insert trigger creating its
thread before the test's duplicate insert; setup was corrected to use that real
trigger-created thread, without production changes. One earlier temporary SQL
profiling helper had a syntax error and stopped with owned cleanup; it is not
accepted as a successful measurement.

Rust74 library tests, the deterministic plan-counter test, all-target checking,
Python56 CI tests, compact Node46 cases plus511 classifications, library-wiring61
cases,676 original frontend source hashes, formatting and whitespace gates passed. Original source files and the
unchanged native diagnostic projection remain separately bound in the receipt.
A genuine browser-native-media candidate run and hosted validation are still
unrun here. No live preview was queried or changed.
