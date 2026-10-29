# Authenticated-browser findings — automated successor qualified, browser recheck pending

The actual authenticated predecessor walkthrough found five defects. All five now have scoped fixes and passing current automated qualification. **Targeted browser confirmation of the successor remains pending.** This is not blanket acceptance of all 21 findings or 94 criteria, and is not a production release.

## Current results

| Gate | Result |
|---|---|
| Full applicable frontend suite | 6,008 passed; 0 failed; 6 unchanged historical skips; 1,737 inputs unchanged |
| Strict composed frontend | 0 errors; 21 retained warnings; 679 canonical files unchanged |
| Static frontend and CSS/source association | 487 files; 7,920,832 bytes; 1,179 build inputs unchanged |
| Normal Rust unit cases | 227 passed |
| Owned-database library cases | 3 explicitly executed and passed |
| HTTP-router/PostgreSQL fixtures | 49 successful invocations, including exact logout/session and follow-preservation regressions |
| Optimized artifacts | Platform server, administrator CLI and browser provisioner rebuilt; standalone node retained only after exact unchanged-source/binary verification |

The backend matrix is an explicit current-source union across 53 successful invocations, not an inherited broad pass. HTTP coverage uses real PostgreSQL with both loopback TCP and in-process router targets. It does not substitute for browser testing.

## Five scoped corrections

- **Logout:** same-origin POST preserves the existing origin, cookie, flow-token and exact-session checks. The page settles on refusal and clears local account state only after authoritative signed-out confirmation. Automatic mutation replay is forbidden.
- **Chapter updates:** Follow/Unfollow toggles notifications through the existing revision-checked account-library API, preserving reading status, list placement, progress and original save time. Unfollow no longer deletes a library entry.
- **Private draft preview:** authorized private metadata, verified cover and relations use a separate preview. It does not mount public chapters, account-library or read-state consumers. Account/route changes clear private state; backend permissions are unchanged.
- **Upload guidance:** obsolete static format/size limits were removed. The embedded guidance links to current Publishing access rather than duplicating potentially conflicting numbers.
- **History wording:** the empty explanation accurately describes completed chapters, without changing completion/history semantics.

Focused proofs include 28 logout helper/mounted cases, 83 follow/client cases plus actual PostgreSQL preservation checks, 24 private-preview mounted cases, 99 upload-focused cases and 12 History cases. The final aggregate includes these current tests. Focused totals overlap other suites and are not summed as unique coverage.

## Exact artifacts

Source identity is the containing reviewed commit plus the exact source association in [the machine-readable artifact receipt](six-persona-qualified-artifacts-20261029.json). The immutable delivery receipt records the resulting commit and tree; this document does not embed a circular self-commit hash. The last verified remote head is `4410d118368a1747f4074809580e43668aed3333`, covering the preceding 14-path payload, not these newer changes.

- Frontend manifest SHA-256: `b6b5ae4d5738e1cc3eb1b7859cd4e989949579f4187960304c370248c4e69e04`
- Frontend index SHA-256: `02564a3fe81987b1555c559b08071a0012ff3b2d4f271d5eb3dcc89647bc47ff`

| Binary under `artifacts/bin` | Bytes | SHA-256 |
|---|---:|---|
| `weebdex-platform` | 55,792,256 | `7e954a522b2df2ff5c972178501e7cae89a9727b06f017c933e29bd5212d890e` |
| `weebdex-admin` | 8,926,536 | `a928058b55a8067e0d19ef59a614b64d7ddd307944b76675984f7a0765b9793f` |
| `browser_fixture_provisioner` | 33,210,200 | `9da00de54f799a9e8c7bfa3e0522ba7e2c9f8d91517b6b0a43ec9ecd5d574405` |
| `manga-node` | 27,728,728 | `44112d36d6e7938e41d6f8de3203506f0b657367bdc0607e1da0314fe3903d7e` |

These are Linux x86-64 artifacts. Use the documented PostgreSQL/configuration prerequisites. For `scripts/run-platform.sh`, explicitly set `PLATFORM_BIN` to `artifacts/bin/weebdex-platform`; do not silently select an older `artifacts/backend` binary. Migrations remain a separate explicit operation.

Dependency versions and lockfiles are unchanged. The previously approved standalone packages were checked against the existing lockfile; remote-signing dependencies retain separate archive/file provenance and licenses.

### Disposable browser-fixture recovery prerequisites

The source/artifact archive does not bundle the PostgreSQL toolchain, original strfry dependency or Python dependency environment. Before starting the disposable browser fixture:

- Set `PLATFORM_TEST_PG_BIN` to the absolute directory containing the previously qualified `postgres`, `pg_ctl` and `initdb` executables when `manga-protocol-lab/vendor/pgsql/bin` is absent. This is the fixture's supported override; it does not select a different platform binary.
- Use the qualified Python environment with Pillow available (`PIL.Image` and `PIL.ImageDraw`). Reuse its dependency root rather than silently installing new versions during acceptance.
- Materialize `manga-protocol-lab/vendor/strfry/strfry` using the reviewed executable whose SHA-256 is `883e2c4db57425a9fa3a8e9453aa84e0cb6c1552d3affa3bc2adfb8f897d994a`. A symlink to that exact existing executable is supported. Verify its resolved path and hash first; do not overwrite an existing, different dependency.
- The fixture resolves strfry relative to its source checkout, while the compiled provisioner also checks the canonical path beneath its **compile-time workspace**. Both paths must resolve to the same reviewed executable. A relocated archive alone does not satisfy this guard: restore the expected dependency path, or separately rebuild and requalify a provisioner for the new location. Do not weaken the guard or substitute the platform's `manga-node` binary.

The 2026-10-29 startup recovery reused the qualified PostgreSQL/Python roots and added only this verified strfry dependency symlink. No product sources, guards or final artifact hashes changed. Two failed startup attempts were cleanly stopped and retained before the successful retry. These prerequisites are for an isolated test fixture, not production deployment or authorization to start it without the agreed user-readiness handoff.

## Browser evidence and remaining limits

The actual predecessor authenticated run covered organic signup and reauthentication, then explicitly simulated email verification and approved disposable administrator setup. It exercised scoped library/profile, team, draft-cover, reporting, actual filtered CSV download, reading-resume and persistence behaviors. Email verification was not organic. Its five failed findings are preserved, including logout 403 and Follow/Unfollow data loss; fixture teardown is not logout success.

The 94-criterion reconciliation records 41 directly exercised behavior subsets, 15 inspection-only entries, 37 core behaviors not run and 1 guidance conflict. These are not 94 passes. Earlier nine guest regressions and the separate owned remote-signer dialog checks retain their own artifact and scope; their success is not silently inherited by changed code. All recorded fixtures stopped cleanly.

Two first-aggregate test integration failures were corrected narrowly: the historical account stage now selects its exact predecessor, and a VM loader resolves imports relative to the importing module. All assertions and exclusions remain. A synthetic expired logout-flow fixture was corrected to respect the existing timestamp constraint. A separate held-throttle authentication test exceeded its unchanged lock timeout; one quiet all-auth rerun passed. Failed attempts remain preserved and no production timeout/security assertion was weakened.

Account-library CSV/XML import/export remains unavailable. Staff history CSV is a distinct reviewed private export capped at 250 rows/256 KiB. Unknown team/credit outcomes stay fenced without automatic replay. Advanced identity and remote signing are optional. Six historical skips, 21 frontend warnings and retained Rust warnings remain disclosed.

No production deployment, main merge, automated CI run, real-user privilege change, external credential use or real publication is implied.
