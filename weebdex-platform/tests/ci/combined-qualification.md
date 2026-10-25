# Reproducible bounded local qualification

Run from a checkout with current Git metadata and already restored, pinned dependencies. Historical local-only ancestors are no longer required: exact counterfactual originals are retained as hash-pinned fixtures. Default mode still uses Git for current-source identity/inventory. The explicit finite-only export mode requires a separately reviewed manifest and expected SHA-256; strict/artifact/all refuse export mode. See `docs/verification/clean-export-mode-20261022.md` for its exact provenance contract and the still-pending complete exported-tree run. This driver installs nothing, never launches Chromium, Cargo, PostgreSQL, an application, a hosted job, or a benchmark. Existing finite tests may own short-lived numeric-loopback HTTP listeners and synthetic child processes. A denied Unix-socket witness stays explicitly skipped; this does not bypass the browser restriction.

## Inputs and command

Create a private local JSON configuration with existing absolute paths:

```json
{
  "frontend": "/absolute/frozen-frontend-with-complete-node_modules",
  "dom_node_modules": "/absolute/pinned-jsdom-fake-idb-svelte-typescript/node_modules",
  "noble_stage": "/absolute/local-library-noble-2.0.1",
  "scure_stage": "/absolute/local-signing-base-2.0.0",
  "bits_dialog_sources": "/absolute/official-bits-ui-2.16.5-dialog-sources",
  "playwright_core": "/absolute/playwright-core-1.62.1",
  "svelte_runtime_sources": "/absolute/svelte-5.55.1-Git-runtime-sources",
  "artifact_root": "/absolute/fresh-Vite-artifact",
  "artifact_manifest": "/absolute/fresh-artifact-manifest.json"
}
```

The full frontend must include its source, yarn.lock and exact dependencies, including Bits UI and Lucide, not just Svelte/TypeScript. DOM fixtures use JSDOM27.0.1/fake-IndexedDB6.2.4. The Svelte counterfactual requires the separately hash-pinned Git-source tree: npm5.55.1 has identical relevant runtime/compiler files but a different package.json and is not an interchangeable input. The exact official Bits UI source pair is checked by SHA256. Playwright is used only for source-extracted accessibility algorithms; no browser is started.

```sh
python3 weebdex-platform/tests/ci/run_combined_qualification.py \
  --config /absolute/local-inputs.json \
  --output /absolute/new-qualification-directory \
  --phase all
```

Use `finite`, `strict`, or `artifact` instead of `all` for separate resource slots. Every output directory and semantic generation must be new. Selected phases and every unselected phase appear in status.json. A finite pass is not a semantic or artifact pass. Run strict only after final product integration has frozen and the frontend-heavy slot is free.

## What executes

- Discovery covers every `test_*.mjs`, `*.test.mjs` and `test_*.py` directly in platform tests/ci. The two positional-argument semantic consumers and fresh generated-CSS consumer are explicitly classified, never fed blindly into node --test. Helper fixtures and live API probe drivers are not unit files. Original protocol/client suites, Rust suites, live probes, native browsers, actual application restore and heavy benchmarks are separate qualifications, not implied passes.
- Finite phase executes all discovered Node files sequentially with VM modules and Python modules through unittest discovery, including modules without a main block. Node skip lines and Python test/reason pairs are retained. A result with any permitted existing skips is named passed-with-explicit-skips, not an unqualified full pass.
- Setup materializes exact original counterfactual bytes from `tests/fixtures/qualification-baselines/`, verifies a fixed manifest SHA256 plus each source SHA256, byte count and Git blob identity, and retains provenance. The old device-restore fixture uses an exact 11-file historical closure, not current source. The six previous upload baselines are preserved and the previously omitted key-import ledger counterfactual is now required unconditionally. The native-user old-source control also uses its exact retained original. No baseline is reconstructed from current code or declared current. The separate eight title/history originals remain byte-unchanged. See `docs/verification/clean-export-dependencies-20261022.md` for the exact requirement inventory and bounded export evidence.
- Strict phase creates two fresh gate0/gate1 projections through the existing checker, then invokes both semantic consumers with their actual generation and matching outer qualification. It records the complete current product inventory, physical dependency root discovery (including symlink targets), exact byte/mode/nanosecond-mtime/symlink snapshots, Node read-only guard audits and bounded owned-process samples. A failed strict baseline prevents its positive/negative consumers; no historical pass is substituted.
- Artifact phase requires an explicit real build root/manifest, invokes the unchanged emitted-CSS consumer and verifies manifest-bound CSS bytes. It is an artifact property check, not evidence that this driver built the artifact or that every product input was used by that build; retain the producing build's independent source/lifecycle receipts.

Each command has a600-second total bound, including25 seconds reserved for cleanup. The qualification adapter reuses the reviewed gated-launch/pidfd/subreaper Owner from scripts/ci/run-sandbox-preflight.py as a library only; it never invokes the browser entrypoint. It captures exact process births, discovers reparented/new-session descendants, excludes preexisting children, and runs terminal cleanup in finally even after resource-read faults or interruption. Unreadable/malformed process census is an explicit uncertainty, not an empty process list. A zero-exit leader with forced descendant cleanup cannot pass.

Receipts retain50ms owned-birth/resource samples, a4GiB+256MiB launch reserve,4GiB+128MiB live disk floor,768MiB available-memory floor and4GiB owned-tree RSS cap. The qualification-specific ledger is capped at1024 births, retained stdout at4MiB, samples at8MiB, and sampled generated-evidence growth at256MiB. Both strict gates write through the bounded pipe, not unbounded per-gate log files. The owner requires actual terminal pipe EOF, preserves captured-handle cleanup despite census failure, and never converts unverified cleanup into an empty-survivor pass. These are finite qualification bounds, not increased browser-probe limits.

Source and physical-cache inventories must remain unchanged after the selected phases. These checks are observed snapshots plus an in-process Node mutation guard, not an OS-level sandbox claim. Earlier finite receipts produced by the preliminary group-only adapter retain their genuine test outcomes but do not establish this stronger lifecycle/cleanup contract; they must not be relabeled as fresh owner-qualified passes.

## Corrected stale contracts

The approved report/draft addition added exactly five immutable source identities: ReportDialog, the drafts route layout, title-drafts layout/page, and title detail loader. It added exactly three approved feature exceptions: signed report dialog, signed draft feed and truthful draft review. Registry expectations are therefore55/18, with the unchanged nine availability and three media exceptions, and42 entries after the existing13-source exclusion. All individual immutable hashes and exact exception-scope checks remain enforced.

The utility-source plugin follows the existing upload-refusal QA plugin in the same pre-compilation unshift. The updated assertion binds that actual order and its following frontend overlay rather than incorrectly requiring utility to be first.

The original integrated-read-fanout-v1.json and its dated evidence remain byte-unchanged. A separate current-source association binds the immutable recipe digest, all37 current source digests, and a reason for each difference. This is source-only recipe requalification, not a new load/browser/capacity result. Any later product integration that changes a pinned input requires review and an explicit association refresh before rerunning.

The late-verification stall test retains its10ms deadline, one consumed byte and timeout outcome. Its synchronous transport/clock seam drives the actual request middleware, accounting, verification, final deadline gate and cleanup deterministically. This avoids racing network arrival against a10ms host deadline; it also asserts verification ran exactly once, cannot report completion, and leaves no inflight request. Other tests still exercise actual finite loopback transport.
