# Single-account source checkpoint — 28 October 2026

This branch preserves ongoing single-account implementation work based on `e3a8ca84eef38fee4168cbde690696c4174e08e5`. **It is not a finished validated release.** Earlier reader-UX build/browser receipts describe their exact earlier source and do not qualify this checkpoint.

Ordinary site login now connects the account library, social/profile controls and contribution adapters to canonical native account storage. Cookie-only adapters remain distinct from optional advanced signed workflows. Migration 106 adds native library revision metadata; existing privacy, explicit confirmation, account-switch fences and authorization checks remain essential.

Focused tests passed during implementation. **The final aggregate suite, release/build gates and complete end-to-end browser validation are still pending.** Do not infer production readiness, complete coverage, successful deployment or migration qualification from this source backup. Later verification must identify the exact source/build tested and any failures or exclusions.

Included: reviewed implementation, migration, tests, strict source-association helpers and normal technical documentation. Excluded: generated build artifacts, runtime databases, keys, credentials, sessions, personal data, screenshots, raw logs, downloads, dependency caches and internal coordination reports. Public dummy fixtures are test data, not production authority.

No main merge, force update or GitHub Actions run is part of this checkpoint. The previous encrypted artifact checkpoint still represents its earlier tested build; it is not an artifact release of this implementation.
