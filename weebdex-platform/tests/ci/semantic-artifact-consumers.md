# Completed strict-artifact consumers

These two tests validate an actual completed strict generation. They are not a replacement for running the strict checker, and need both explicit arguments:

```
node tests/ci/test_composed_semantic_projection.mjs GENERATION OUTER_QUALIFICATION
node tests/ci/test_semantic_validation_fail_closed.mjs GENERATION OUTER_QUALIFICATION
```

No stale generation default or fabricated legacy isolation JSON is accepted. The generation must match one of the outer `strict-{0,1}-summary.json` copies. The outer qualification also supplies current `strict-product-source-pins.json`, successful stopped `strict-resource-receipt.json`, exact `physical-dependencies-{before,after}.json.gz` snapshots and summaries, `physical-dependency-root-discovery.json`, and hash-bound `cache-audit-summary-physical.json` records for every actual `cache-audit/strict-physical` process receipt.

Mapped source identities are derived from the immutable679-file frontend inventory, actual reviewed added routes and reachable production graph. Every production snapshot, output, map, embedded source and validation record remains hash checked. The consumer independently reexecutes the actual reviewed pure five-stage transform chain at its observed gate and compares every intermediate/final output; it does not build or start the application. Actual import literals and all source-graph identities must agree. Both current gate profiles are validated independently;654/684 are observed generation results, not new hardcoded acceptance counts.

Dependency proofs cover actual physical targets behind symlinked package containers. Missing physical roots or missing per-process protections fail closed. The Python helper uses exact arbitrary-precision `mtimeNs` integers, checks byte/mode/mtime/symlink before/after equality, re-inventories current physical inputs and refuses symlink escapes. It does not write to dependency caches. The unchanged Node guard's denial controls use only a newly owned temporary fake cache, never real packages. This remains in-process mutation enforcement plus observed snapshots, not OS syscall sandboxing.

Negative controls require a passing real baseline first, then independently omit canonical/source-map/graph/cache evidence, remove actual primary-root protection, alter nanosecond metadata, and exercise missing/empty guard configuration and a genuine blocked cache write. Each refusal must match its intended assertion; an unrelated setup error cannot pass a negative control.

Historical generations whose outer guard omitted the physical primary cache remain incomplete evidence. A later corrected snapshot cannot retrospectively qualify them.
