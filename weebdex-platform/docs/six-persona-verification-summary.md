# Current automated qualification — broader browser acceptance pending

The current source has complete passing automated gates. **It is not blanket acceptance of all 21 findings or 94 criteria, and is not a production release.** The preceding authenticated browser passed four targeted fixes and exposed a cold private-preview reload race; the current correction has not yet been retested in the actual browser.

## Current gates

| Gate | Result |
|---|---|
| Full applicable frontend suite | 6,028 passed; 0 failed; 6 unchanged historical skips; 1,742 inputs unchanged |
| Strict composed frontend | 0 errors; 21 retained warnings; 679 canonical files unchanged |
| Static frontend/CSS/source association | 488 files; 7,924,456 bytes; 1,182 build inputs unchanged |
| Normal Rust unit cases | 227 passed |
| Owned-database library cases | 3 explicitly executed and passed |
| HTTP-router/PostgreSQL fixtures | 50 successful invocations, including real publication-access CLI/TCP and projected-source deletion refusal |
| Optimized artifacts | Three platform binaries rebuilt; standalone node retained with exact source/binary continuity |

The backend matrix binds 392 current inputs to 54 successful invocations. It combines real loopback TCP and in-process routers backed by owned PostgreSQL; it is not browser evidence. Every owned fixture completed verified cleanup. Focused counts overlap the full suite and are not added as unique coverage.

## Scope of this successor

- **Private draft cold entry:** the universal loader returns a validated route intent without private data or a premature Session read. The actual preview performs current native authorization after Session initialization, clears on account/route changes and never falls back to public reader children. 32 focused actual-component/session/bootstrap cases passed; this is not an SSR-renderer claim because the application has SSR disabled.
- **Native publication-only access:** a supported reviewed offline CLI uses a separate delegation table, not `protocol_identity_bindings` or portable identity links. Exact original admission epoch, delegation revision, account lifecycle and command authority remain pinned. Current native clocks are checked before signing and after staging at original API handoff, not atomically inside the original module's subsequent transaction. No live account delegation has been applied.
- **Separate scheduling capability:** delegation supports publish-now only; future scheduling is refused before source/schedule side effects. The UI preserves a selected date and requires deliberate clearing, while existing schedule recovery remains available. The compact closed disclosure regression was fixed without weakening its original assertion. 118 focused cases passed, including the retained 11-case panel suite.
- **Independent signer preparation:** official nak 0.21.2 is checksum-pinned. A bounded forwarding-only loopback helper and exact optional browser-fixture origin are prepared; 4 pure helper cases and 14 fixture-origin/retained-PG-root cases passed. No nak execution, key generation, relay or external-signer browser acceptance is claimed.

The dedicated delegation grants no signer-to-native login, native role/permission, original admission, publishing-enabled group, source withdrawal/deletion or new scheduled-worker authority. A real projected-source HTTP test proves native delegation plus the local removal grant still reaches exact 403 source-deletion refusal without source/native mutation. See [the reviewed operator workflow](native-publication-access-cli.md).

## Exact artifacts and recovery

Source identity is the containing reviewed commit plus the exact source association in [the artifact receipt](six-persona-qualified-artifacts-20261029.json). The immutable delivery receipt records the resulting commit/tree; this document does not embed a circular self-commit hash. Last verified remote head: `ae5ea82a805a931cded217f80bb882c99c56dfc9`. The current successor is not yet published.

- Frontend manifest SHA-256: `11f186ef7f8c5fbfae8cc2f6adbbda4274258ba48a3f023a2503ef36903cb236`
- Frontend index SHA-256: `7b5164186c205362c02c253365c19da4bdffc01da8cdbe6fc512b9177b3522c1`

| Binary under `artifacts/bin` | Bytes | SHA-256 |
|---|---:|---|
| `weebdex-platform` | 55,963,896 | `a82ad08fe8da27b343fa8a7eb42bd2f9ec971ac8ad61f9a3a00f4285d29f4f64` |
| `weebdex-admin` | 9,545,104 | `ff2b2c840b39ee9d6e764587a73cd33b7813d34308e763f4a12bec780174a9b5` |
| `browser_fixture_provisioner` | 33,311,960 | `f28fcbf1d3be83469270ac267b8c98bc554cddfc2843c2e642d9956c2467b59f` |
| `manga-node` | 27,728,728 | `44112d36d6e7938e41d6f8de3203506f0b657367bdc0607e1da0314fe3903d7e` |

These are Linux x86-64 artifacts. Set `PLATFORM_BIN` explicitly to `artifacts/bin/weebdex-platform` for `scripts/run-platform.sh`; do not silently select an older `artifacts/backend` binary. Migrations, deployment and live delegation are separate explicit operations. Product dependency lockfiles are unchanged. Remote-signing dependencies retain their archive/file provenance and licenses; the independent nak executable is a separate test-only tool, not a bundled credential or automatic runtime requirement.

### Disposable browser-fixture recovery prerequisites

The source/artifact archive does not bundle the PostgreSQL toolchain, original strfry dependency or Python dependency environment. Before starting the disposable browser fixture:

- Set `PLATFORM_TEST_PG_BIN` to the absolute directory containing the previously qualified `postgres`, `pg_ctl` and `initdb` executables when `manga-protocol-lab/vendor/pgsql/bin` is absent. This is the fixture's supported override; it does not select a different platform binary.
- Use the qualified Python environment with Pillow available (`PIL.Image` and `PIL.ImageDraw`). Reuse its dependency root rather than silently installing new versions during acceptance.
- Materialize `manga-protocol-lab/vendor/strfry/strfry` using the reviewed executable whose SHA-256 is `883e2c4db57425a9fa3a8e9453aa84e0cb6c1552d3affa3bc2adfb8f897d994a`. A symlink to that exact existing executable is supported. Verify its resolved path and hash first; do not overwrite an existing, different dependency.
- The fixture resolves strfry relative to its source checkout, while the compiled provisioner also checks the canonical path beneath its **compile-time workspace**. Both paths must resolve to the same reviewed executable. A relocated archive alone does not satisfy this guard: restore the expected dependency path, or separately rebuild and requalify a provisioner for the new location. Do not weaken the guard or substitute the platform's `manga-node` binary.

The 2026-10-29 startup recovery reused the qualified PostgreSQL/Python roots and added only this verified strfry dependency symlink. No product sources, guards or final artifact hashes changed. Two failed startup attempts were cleanly stopped and retained before the successful retry. These prerequisites are for an isolated test fixture, not production deployment or authorization to start it without the agreed user-readiness handoff.


For a separately authorized independent-signer test, the new fixture option accepts exactly one `ws://127.0.0.1:PORT` origin on an unprivileged port. It grants no account/publication authority, contains no key/URI handoff and is not permission to start a relay. Reserve the planned port before fixture allocation, wait for the native account/user-ready step, then use the bounded separate signer supervisor and user-mediated private input workflow. Never add a wildcard or external relay to work around the capability boundary.

## Actual browser evidence and remaining limits

The preceding authenticated successor actually passed title Follow/Unfollow status/progress preservation, completion-only History copy, current upload guidance and logout with protected-page reload denial. Internal draft-preview navigation worked, but direct reload failed before the shell. Its failure remains separate from the current 32-case repair. Cumulative tracker scope is 42 directly exercised behavior subsets, 15 inspection-only and 37 core behaviors unrun—not 94 complete passes. Earlier actual staff CSV delivery and guest/remote-dialog checks retain their exact artifact and scope, not a blanket pass for changed code.

The broader current browser cases still pending are cold draft reload, native publication with separately authorized setup, content-report correction, genuine user-mediated email verification and independent nak/native-link signing/disconnect. The previous verification handoff was blocked; no agent retrieved a one-time code and no organic verification pass is claimed. Prior simulated verification is explicitly a test precondition, not organic delivery/verification evidence.

The first new compile exposed an outdated library-test caller. The first new publication fixture formatted database-local EDT as documented UTC and unintentionally requested a past time; explicit UTC plus a future preassertion corrected that fixture while retaining 403/no-effects. The first frontend aggregate found a real compact-disclosure regression; its original assertion was preserved and the explanation moved inside expansion. Those failures, and earlier histories, remain preserved. A runtime reset also interrupted an optimized release attempt without a terminal result; after the old managed session disappeared and source/artifacts were verified unchanged, the same offline command completed in separate successor evidence. No test exclusion, source pin, authority fence or original timeout was weakened.

Account-library CSV/XML import/export remains unavailable. Staff history CSV is a distinct private export capped at 250 rows/256 KiB. Unknown team/credit results remain fenced without automatic replay. Advanced identity is optional. Six historical skips, 21 retained frontend warnings and retained Rust warnings remain disclosed. No production deployment, main merge, CI run, real-user privilege change or external publication is implied.
