# Bounded synthetic application-data restore qualification

The new `scripts/ci/qualify-full-application-restore.py` is a qualification
**driver**, not a production restore tool. The checked-in finite tests do not
start PostgreSQL or the application. Compilation of the guarded provisioner and
its unit tests is also not an application-restore result. See the separate
finite evidence receipt for what actually ran. A successful runtime result means
application **data** restored with separately retained matching synthetic
credentials. It does not mean independent full-host or key recovery.

The 2026-10-25 authentication-fencing update has only finite, mocked boundary
tests. No real database restore, old-token rejection or fresh-login runtime
qualification has been run for this update; the sequence below remains a driver
contract, not an execution receipt.

## Scope and prerequisites

Reuse one independently reviewed matching build of the real platform, guarded
`browser_fixture_provisioner`, complete integrated frontend, original strfry and
PostgreSQL client/server tools. No dependency download, compiler invocation,
browser, external service, real account, production key, or public upload occurs
inside this driver. The existing native Chromium socket denial is irrelevant:
this driver neither launches a browser nor attempts a different browser route.

The provisioner must be compiled from this checkout: its existing compiled
`CARGO_MANIFEST_DIR` guard still restricts actions to this checkout's private
`test-runtime/browser-final-*` root and exact reserved password-free loopback
source database. No fixture, credential, database-name or source-overlap guard is
relaxed. The only added action stages a synthetic signed personal note using
real original account admission, account permit and durable staging APIs while
the listeners are joined. It requires explicit restore opt-in and the existing
joined-listener marker. It never fabricates or rewrites an outbox SQL row.

A candidate pin inventory records hashes and byte sizes for:

- Both Rust source/migration/example/test graphs and Cargo manifests/locks.
- The supplied frontend source/static files, root build configurations and
  post-build scripts; every platform integration overlay; platform build,
  projection, QA, fixture and regression scripts.
- Exact platform/provisioner/strfry/Python/PostgreSQL executable bytes.
- Exact release and complete frontend-artifact manifests. Release artifact
  membership must equal the platform plus every frontend file, with no extras;
  frontend directories/files must be physical, unlinked and bounded.

Inventory alone is not proof of build provenance. The coordinator must associate
these reviewed source bytes with its matching compiler/build receipts before
approving a runtime attempt. The original release-manifest format does not
attest that relation. Hashes preserve the reviewed handoff, not its authenticity.
No claim of deterministic compiler output or dependency-source reproducibility
is made. Installed Python/Pillow compatibility remains a runtime prerequisite.

## Command contract

Run from the matching checkout's `weebdex-platform` directory. All artifact paths
must be physical paths to the already-reviewed artifacts. `PLATFORM_TEST_PG_BIN`
selects the already-reviewed PostgreSQL tool root. Strfry remains the exact
existing fixture path `manga-protocol-lab/vendor/strfry/strfry`.

First produce a read-only candidate inventory, review it against the build
handoff, and retain it as a private operator input. This does not launch services:

```sh
python3 scripts/ci/qualify-full-application-restore.py \
  --binary PLATFORM_BINARY --protocol-provisioner FIXTURE_PROVISIONER \
  --ui-dir COMPLETE_FRONTEND --release-manifest RELEASE_MANIFEST \
  --frontend-artifact-manifest FRONTEND_MANIFEST --inventory
```

Use the same five artifact arguments for every following command. Supply the
reviewed inventory file and its independently retained SHA-256. The default is
still a read-only plan; it must neither create an evidence directory nor a
fixture, bundle, database, listener or key:

```sh
python3 scripts/ci/qualify-full-application-restore.py [same artifact arguments] \
  --pins REVIEWED_PINS --pins-sha256 REVIEWED_SHA256 \
  --output NEW_ABSOLUTE_DIRECTORY_UNDER_THIS_CHECKOUT_TEST_EVIDENCE
```

Only after the parent/coordinator's review and runtime authorization, add
`--execute-synthetic`. The default runtime observation window is 600 seconds;
`--seconds` permits 60–900 seconds. A short window may correctly refuse a run.
The fixture's reviewed graceful cleanup has its own finite per-process drain,
reap and fallback deadlines; cleanup is not claimed to fit inside the observation
window. A timeout, signal, forced lifecycle, uncertain process outcome or failed
cleanup can never emit a passing receipt.

## Actual execution sequence

1. Initialize both full schemas using the existing guarded fixture's original
   setup and native migration commands. Use real signed series/chapters and
   generated local images, one owned PostgreSQL cluster, one real platform with
   its embedded original listener, and the local synthetic mail surface. This
   specialization enables PostgreSQL fsync and full-page writes; it does not
   inherit the browser fixture's non-durable optimization.
2. Register a synthetic native user through the actual CSRF-bound browser-flow
   API, then create a private list through the authenticated API. The generated
   password and issued session stay in memory. A narrow fixture SQL seam marks
   that synthetic user verified and grants only preference-update/library-read
   permissions. Seed a read-only API client and sent/unused verification and
   recovery token hashes bound to actual browser flows; confirm private-list API
   access and live flow reads. This does not qualify real email delivery,
   password-recovery completion, browser UI or signed-library behavior.
3. Close/join the mail/provisioning surface, observe readiness withdrawal, and
   gracefully stop/join the platform and embedded original writer. Stage exactly
   one real signed personal-note outbox item through the provisioner, join it,
   verify it is still pending, and leave only the owned PostgreSQL server live.
4. Preserve both existing safety boundaries: the fixture stores are inside its
   guarded private checkout root, while the private-checkpoint tool correctly
   refuses checkout storage. Therefore, while all synthetic writers are stopped,
   the driver makes a bounded, exact-membership/hash/identity-verified handoff of
   all three roots to a new private external directory. It verifies source
   stability before/after copying and checks process references/listeners with
   the unchanged checkpoint guard. Configuration and synthetic key files remain
   separate and are never in those copied roots. This extra handoff is explicit,
   not presented as a direct production-layout qualification. Both relocated
   configurations deliberately retain the source fixture operator/service key
   file paths and its group-key directory. The checkpoint does not restore them.
   Target configurations explicitly pause original autonomous roles and private
   discovery exceptions, retaining other role configuration and signer identity.
5. Run actual default backup plan, backup execute and bundle verification using
   the unchanged `private-checkpoint.py` API. Its real PostgreSQL commands run
   through the fixture's existing registered-birth process owner. The bundle
   contains the whole database plus native, original and projection storage.
   Then deliberately revoke the source fixture's old cookie/API credentials and
   consume its email tokens/flows after the backup. Pin the source database's new
   exact snapshot so later stability checks distinguish this intentional change.
6. Create two new databases from template0 on that disposable cluster. The
   occupied target contains a known sentinel; require the specific nonempty
   database refusal before any roots are created, and reread the unchanged
   sentinel. A different refusal is not credited. The genuine target is a
   separate empty database with absent roots; default restore plan must not
   create them. Actual dump restore must commit, followed by mandatory successful
   restored-authentication/autonomous-work fencing before a completion receipt.
7. Before starting the restored app, compare every public/platform table row and
   business column, all sequence state and the complete no-owner/no-ACL schema
   dump. A fixed allowlist projects out only the intentional restore-control
   columns in authentication, inventory, role, schedule and node-epoch ledgers;
   no table or business row is omitted. Independently require all restored
   capabilities invalid, schedules/roles paused, inventory sessions cleared and
   original restore/authentication epoch advanced. Immutable scheduled inputs,
   canonical users/policy, signer identity and pending publication/outbox rows
   retain exact parity. The only
   schema normalization removes PostgreSQL's random `\restrict`/`\unrestrict`
   safety nonce, not schema SQL. Verify every restored storage member/hash.
8. Before any restored child starts, require the exact separately retained
   operator/service key paths, private physical files, bounded sizes and matching
   in-memory-only byte fingerprints. Never emit those fingerprints or key bytes.
   Require the existing group-key directory to remain empty: this corpus has no
   managed-group signer and does not qualify group-key recovery. Missing/replaced
   keys, redirected paths, or missing/nonempty group-key directories are refused
   by the driver before `check-config`, `verify` or `serve` can generate or use a
   different authority. The finite negative controls exercise this driver guard
   with non-key placeholder bytes, not real application credential failures.
   Then use the matching binary's `check-config`, then `verify`, then `serve`, never
   `migrate`. Require actual readiness, same catalogue namespace and service
   authority, exact original signed-event bytes, exact original media hashes,
   native and original-backed reader media Range/ETag behavior, authenticated
   private-list equality, anonymous refusal, and terminal reconciliation of the
   pending original note. The note retains one event and one completed outbox
   row with the same immutable event fingerprint. This does not prove exactly
   one physical backend invocation under arbitrary crashes.
   Before private reads, require the old cookie to fail with 401, the old API
   client to lose private-list access, both old browser flows to return 410 and
   both email tokens to return 400. A new CSRF-bound password login must issue a
   different working session; never reuse the backup session for success checks.
9. Gracefully stop and restart once more with the same restored database/roots
   and identities; repeat old-capability rejection and a new password login as
   well as the data/media checks. Verify the post-revocation source database, sequences,
   original roots, external stopped roots and all pinned source/tool inputs are
   unchanged. Finally require the existing physical closed-port/no-survivor,
   no-forced-lifecycle cleanup receipt and removal of synthetic server keys.

Native and original media GET hashes are checked separately. Range/conditional
checks are made through the production reader adapter, which supports them;
the original module's `/originals/{hash}` endpoint is hash-checked without
pretending it implements Range/conditional semantics it does not provide. Runtime
receipts expose `native_reader_media_hash_range_etag_verified` and
`original_endpoint_media_hashes_verified` separately, and always retain
`original_endpoint_range_qualified: false`. Native-reader coverage includes both
native stored pages and imported original-backed pages via the reader adapter.

## Bounds and evidence

The resource watchdog samples every 0.5 seconds: no more than 1 GiB new allocated
fixture/private storage, 100,000 entries, 1.5 GiB owned RSS, at least 2 GiB host
available memory, and 4 GiB +128 MiB live free space. Startup additionally requires
space for the full 1 GiB cap plus 4 GiB +256 MiB reserve. Each checkpoint is capped
at 256 MiB and 90 seconds or the remaining observation window. These are sampled
refusal ceilings, not hard kernel quotas or throughput/capacity claims.

A successful status is explicitly
`passed-synthetic-data-restore-with-retained-credentials`. Every outcome retains
`separately_retained_matching_credentials_required: true`,
`independent_full_host_recovery_qualified: false`, `credential_recovery_qualified:
false` and `managed_group_key_recovery_qualified: false`. Runtime success requires
`retained_credentials_match: true` and `synthetic_server_keys_absent: true` after
cleanup. These limitations cannot be removed while retaining a passing outcome.
Success additionally requires exact business-row/schema/sequence parity,
`restored_authentication_fenced`, `autonomous_roles_paused`,
`old_cookie_api_email_and_flows_rejected`, `fresh_login_verified` and
`source_credentials_revoked_after_backup`. Retained credentials here mean
separately retained operator/service signing keys, not reusable restored sessions.

The driver prints and writes only a bounded fixed-field JSON outcome with
booleans, stage/error-family labels, counts and hashes. Child output, raw SQL,
credentials, session tokens, signed bodies, configuration, runtime roots, key
files, dump and bundle remain private. Do not copy the private runtime or bundle
into public checkpoint/evidence archives, even though this corpus is synthetic.
The driver removes only its generated service/operator keys through the existing
fixture cleanup. It retains other private state/partial outputs for inspection;
it does not blindly delete an uncertain restore or retry one.

## Explicitly still open

Even a genuine passing synthetic runtime receipt is not production DR. It does
not qualify pending native title/upload commands, arbitrary account/library
histories, signed-library synchronization, active accepted-request drain under
load, remote writers/services, cross-host/role/grant recovery, production secret
provisioning, independent full-host/key recovery, managed-group signing recovery,
large databases/volumes, browser persistence or production capacity.
Those remain separate work. The current finite-test receipt must never be cited
as a passed restored application run.
