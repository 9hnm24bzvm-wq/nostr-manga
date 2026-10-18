# Production recovery qualification and remaining prerequisites

Audit base: restored `be283b664f699e8c7175f6eac0200cd22e212c8a`, 2026-10-18.
This is a bounded inventory/plan, not a production-readiness certificate.

## What is established

- The private checkpoint script defaults to planning, binds the exact release
  manifest hash, dumps the whole database, copies all three disjoint storage
  roots, checks their membership/identity/hashes before and after copying, and
  writes completion last. Restore refuses occupied roots/nonempty databases and
  uses a transaction. It cannot prove that all remote writers are stopped;
  operator-asserted quiescence remains necessary.
- A retained historical real PostgreSQL 17 mechanics drill verifies actual
  custom-format dump/transactional restore of two tiny synthetic tables and
  three tiny roots. It does not start the application or establish recovery of
  account authority, signed publication, source visibility or pending commands.
- Eight real original nginx supervisor lifecycle methods previously passed on
  the unchanged current supervisor implementation, with exact source/binary
  pins, ownership/recovery/drain/Range checks and physical shutdown proof. This
  covers that original media reader, not the entire platform's shutdown.
- Current finite checks cover checkpoint refusal/integrity/byte-stability,
  wrapper action boundaries, and source ordering for SIGTERM/readiness/request
  drain plus the example systemd deadline. The wrapper uses a stand-in, and the
  graceful-platform check is source-only. Neither is a live application test.

The new exclusion guard refuses either input runtime configuration file inside
any copied storage root, before a tool/output is opened. This closes a concrete
counterexample to configuration exclusion. Unknown secrets among intended media
cannot be classified automatically; reviewed storage layout is still required.

## Environment inventory after workspace replacement

Observed 15:16–15:18 UTC, before concurrent dependency-recovery handoffs. This
is an audit-time observation, not a claim that other lanes remain unchanged.

Present: Python 3.12.14, Node 24.19.0, Linux pidfd/proc inspection, OpenSSL 3.5.7,
`dpkg-deb`, make/cc/g++, and official isolated nginx 1.30.6. The existing libpq
library reports 170011 (PostgreSQL 17.11); it supports the finite no-connection DSN
parser checks. Python aiohttp 3.14.4 and Pillow 12.3.0 are installed.

Missing at that audit time from the bounded project/system inventory: `psql`, `pg_dump`,
`pg_restore`, `postgres`, `initdb`, Rust/cargo, platform/original-node/strfry
executables, built integrated frontend/release manifest, and Python cbor2 and
coincurve. Old warm toolchains, runtime databases, build artifacts and local
backup archives disappeared with the old workspace. Public source checkpoints
and their historical receipts are not substitutes for those private artifacts.
No installation, database process, service deployment or deletion occurred in
this audit. Resource observations are in the associated evidence.

At 15:30 UTC a separate approved recovery lane reported cbor2/coincurve recovery
and its own finite results. Reuse only its verified physical target/version/hash
handoff; that later recovery does not revise this audit's historical observation
or establish the still-unrun 30k/500k or full-application restore cases.

## Full application restore: required next qualification

Before claiming recovery, obtain a matching trusted immutable release (or build
and qualify a new locked release once) containing the integrated frontend,
platform binary and release manifest, plus the matching original migration
path/tool and strfry. The publisher can embed its original listener, but a fresh
baseline still needs original-schema initialization through the matching CLI or
shared core. Rust packages declare minimum 1.90; frontend build requires Node 22.12+
and the unchanged lockfile's dependencies. Do not substitute static placeholder
HTML: startup checks for the real bounded SvelteKit bootstrap.

Any new official dependency installation/download needs the parent-approved
artifact/version/hash/dependency closure and bounded disk/time plan. Reuse a
single coordinated backend build/target; no competing warm builds. No such
installation/build/real restore is authorized by this inventory alone.

A fresh synthetic-only drill should then:

1. Pin every binary, migration, frontend artifact and release-manifest byte.
   Create a new isolated PostgreSQL baseline and three private local roots;
   initialize both schema sets. Keep configuration and test signing identities
   separate. No real account data or production key is required.
2. Through real signed/native interfaces, establish one visible series, eligible
   original cover, readable chapter/pages and known media hashes; retain original
   signed-event identity. Add one private account/library state and one durable
   pending upload/publication intent or outbox item to exercise reconciliation.
3. Gracefully stop every writer. Prove readiness withdrawal, admitted-request
   drain, original child stop, closed listeners and exact owned-process cleanup.
   Keep PostgreSQL alive; preserve immutable commands without guessing success.
4. Run real private backup and verification with the current script. Restore to
   a separately created empty database and absent roots with the exact retained
   release, identities, volume/group bindings and reviewed service grants.
5. Run matching `check-config` and `verify` before serving, with no automatic
   migrations. Start only the isolated loopback runtime. Verify original/source
   visibility, exact raw event/media bytes, Range/conditional reads, reader pages,
   local metadata/ownership and the private account state. Reconcile the pending
   intent/outbox exactly once without duplicate original publication.
6. Stop/restart once and repeat those checks; retain source/hash/outcome and
   physical-cleanup evidence. No raw database, archive, keys, account content or
   connection values belong in the public report.

Proposed small-drill envelope **after** tools/build are ready: one serial
loopback runtime, at most 1 GiB new fixture/bundle/restored storage, 1.5 GiB combined
owned RSS, at least 2 GiB host available memory, 300s test plus 45s physical cleanup,
and 4 GiB +256MiB start/4GiB +128MiB live free-space reserves. These are proposed
refusal ceilings, not measured capacity or permission to start. A build/toolchain
budget is separate and must be coordinated first. A production-data drill
additionally needs explicit operator authorization for the exact isolated target,
private backups and existing secret provisioning; never ask for credentials in
chat or expose that data as a test artifact.

## PostgreSQL 18 diagnostics: twelve held methods

`bench/library-node/test_pg_diagnostics.py` has five finite methods and twelve
real PostgreSQL methods. Fresh finite execution passes five and explicitly skips
twelve. Its statistics validator accepts only server versions 180000..189999;
PostgreSQL 17 is not equivalent. No Python database driver is required: the real
path executes `psql`. Original migrations and two tiny fixture tables/queues are
sufficient; no application or strfry binary is required for these twelve methods.

The local bootstrap source pins PostgreSQL 18.6/archive SHA256
`555610c24d53e4316da5b7d3fc25c279d96856d5e0e23ee308c328c5fa881d9f`.
That is an inventory of the checked-in pin, not fresh registry/signature
verification or approval to install it. Its compiler fallback requires bison,
flex and m4, currently absent; a build must not be launched blindly. Prefer a
parent-reviewed official package/tool closure if available, with fresh provenance.

The existing shell fixture fixes 127.0.0.1:55437, empty Unix-socket path,
shared_buffers 16 MiB and max_wal_size 512/1024MiB, initializes only a new cluster,
and validates its owned data path. Real cases intentionally arrange locks,
reset that isolated cluster's statistics and terminate only named fixture
sessions; the sampler itself is read-only. The wrapper also automatically
removes its runtime, so it must **not** be run under the current no-delete
instruction. Use an explicitly owned retaining driver instead.

Current tests require a `/tmp/mixed-diagnostic-*` parent. Here `/tmp` is tmpfs
(5,225,717,760 bytes total), while workspace storage is overlay. Account tmpfs/WAL
bytes against memory as well as filesystem reserve; never use this diagnostic
fixture as a disk-durability/performance claim. Proposed next-test limits:
256 MiB fixture (stop if exceeded despite configured max WAL),768 MiB owned RSS,
at least 2 GiB available memory, 120s tests +30s cleanup, one closed/available fixed
port and no competing runtime. Retain data/logs and enforce birth/pidfd cleanup.
Any installed package/build budget is separate. Stop/refuse if these bounds do
not fit; do not weaken statistics-version or timeout assertions.

## 30k/500k compaction versus actual-node benchmarks

`bench/library-node/test_compact.py` executes both histories **at import time**;
never include it in blanket discovery. It creates two complete Library objects
per size, compacts one, and compares exact roots plus five updates. It is an
in-memory codec equivalence test, not an application/SQL/load measurement.
`run.py` import requires `NODE_RESULTS`, aiohttp, cbor2 and coincurve and imports
the original migration codecs. Configure a new output directory first. Original
requirements pin cbor2==5.6.5 and coincurve==21.0.0; the local aiohttp 3.14.4
observation differs from the historical requirements pin 3.13.5 and must remain
explicit in any result. No absent dependency may be silently mocked.

Proposed offline-only envelope after the approved dependency environment exists:
one process, serial 30k then 500k, 1.5 GiB RSS, 2 GiB available-memory reserve,
64 MiB filesystem output and 120s plus 30s cleanup, with no HTTP request or database.
This is a proposed safe ceiling, not measured runtime/memory. Stop on the cap;
keep incomplete output and do not report a partial history as a full pass.

Actual-node serial/load fixtures are a different scope: they additionally need
PostgreSQL, built matching release manga-node, strfry, synthetic admission/keys,
loopback ports 58200/55441, migrations, storage and an explicit traffic/resource
budget. Their success cleanup deletes fixtures, so the old wrappers cannot run
as-is under current instructions. Coordinate with the mixed-load owner; neither
this inventory nor the offline equivalence case qualifies WAN, full connection
memory, a 100 TB corpus, eight-billion-image traffic or a production SLO.
