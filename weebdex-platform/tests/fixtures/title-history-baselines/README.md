# Retained title/history counterfactual sources

These eight small files are exact, unmodified bytes from the source commits and paths recorded in manifest.json. Each entry includes full commit provenance, Git blob identity, byte count and SHA-256. The commits are provenance only: tests never fetch them or require a local Git object database.

The shared tests/ci/retained_title_history_source.mjs loader pins the complete manifest SHA-256 in source, validates the exact eight-entry set and provenance schema, then checks byte count and SHA-256 before returning a source. Corruption controls reject changed manifests, changed source bytes, coordinated manifest/source substitutions and unknown entry names without editing the tracked fixtures. Tests still execute the same historical functions/modules with their existing explicitly bounded dependency fixtures; this is not a reproduction of an entire historical application or build.

The retained boundaries cover the original eager title Search, initial full-index-only title pager, lexical followedAt pager, native history/title query serialization, millisecond-only history pager, pre-extraction exact timestamp helper and erased follow-response TypeScript contract. Do not regenerate these fixtures from current source to make a counterfactual pass. Any baseline change requires an explicit reviewed provenance update.
