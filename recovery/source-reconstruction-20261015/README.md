# Source reconstruction checkpoint

These namespaced source fragments were reconstructed after the execution workspace was replaced on 2026-10-15 at about 04:35 UTC. They are preserved as engineering recovery material, not installed into product modules.

The branch starts at the independently recovered public source commit `82290c732cc19b393b128fec0879885937f41054`, tree `3f724cd2150f9049a732847d98195c551b4374bc`. That source and its history were cloned and all 2,710 tracked file blobs were recomputed successfully after the replacement.

## Current snapshot

- Foundation: producer-confirmed stable reconstructed files and integration requirements.
- These reconstructed fragments are **unverified**: no byte-for-byte match to lost original commits, compile result, runtime result or endpoint parity is claimed. Historical observations are clearly identified in the producer's context file.
- Missing components and integration constraints remain documented. Other worker reconstructions will be added in separate small commits.
- No credentials, private keys, runtime databases, private logs or screenshots are included.

The complete earlier 13-commit onboarding/build/evidence checkpoint was not published. Only the first 10 commits are reachable from the base above. More recent onboarding policy changes and MangaDex implementation are not represented as restored production code by these fragments.

## Additional preserved source groups

The namespaced recovery history also preserves independently frozen public UUID persistence, native catalogue mutation core, public reports, public statistics, Nostr auth/list/profile/read-marker adapters, bound-account helpers, frontend adapters and browser CI correction patches/tests. Per-component manifests and recovery limitations accompany each group. They are source reconstruction material, not a representation that all API operations are implemented or accepted.

Normal code snapshots are also backed up on the repository branches:

- `rebuild/mangadex-contract-foundation`
- `rebuild/mangadex-frontend-source`
- `rebuild/nostr-public-adapters`
- `rebuild/onboarding-source-recovery-20261015`
- `rebuild/browser-ci-source-recovery`

Each source checkpoint records its local source origin and is separate from deployment or full browser acceptance. The diagnostic branch was not advanced by these recovery backups.
