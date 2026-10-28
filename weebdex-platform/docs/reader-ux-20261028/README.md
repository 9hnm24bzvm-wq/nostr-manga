# Reader UX verification — 2026-10-28

Review candidate based on `1fbe4f4f9ca17463274023772a6c757427f0db5c`; not merged into main or production-deployed.

## Final source checks

- Applicable standalone suite: **5,254 passed, 0 failed, 6 skipped** (5,260 total).
- Final frontend build: **passed**, 465 files, 7,697,443 bytes; public catalogue UI enabled.
- Generated CSS: **1 passed**. Strict additive source association: **2 passed**, binding 55 reviewed sources.
- All final artifact and source bytes were reverified after the suite.
- [Receipt](verification-receipt.json) records exact artifact/source hashes. Raw logs, screenshots, runtime state, keys and personal data are excluded from this publication.

This is an applicable-suite result, not an all-authored-tests pass. [Exact exclusions](suite-exclusions.json) name historical source-bound qualifications and unavailable predecessor fixtures. Historical receipts remain historical; no inherited browser, release or production qualification is claimed. Positional historical qualification CLIs were not rerun. The final CSS test ran separately. Strict current-source restoration is tested against the immutable base without weakening prior pins.

## Browser evidence and remaining limit

The disposable browser walkthrough checked fresh-reader navigation, one-follow Updates, reader idle controls, profile Groups, identity setup and task return, list presentation and language filters. These checks passed on earlier immutable fixture builds, **before the final language-selector/schedules build**. A mock rating of **10/10** was saved and read back. Recovery-key download and reimport passed. These are bounded synthetic checks, not real-user publication or production evidence.

**Mock chapter upload was not verified:** approval for the final CUA submission remains pending and the live fixture has expired. The final build's named language selector and compact schedules UI passed component/contract tests but were not rechecked in the live browser. Do not describe the upload journey or final-build browser suite as complete.

## Privacy and authority

Identity setup is centralized and explicit. Creating/downloading a recovery key does not itself acknowledge backup, select/activate an identity, admit a publisher or submit the interrupted task. Private-key custody stays client-side. Public source delivery excludes credentials, private keys, runtime databases, sessions and manga image datasets. Deliberate synthetic test fixtures are not production authority.

No main merge, force update, Actions dispatch, deployment or production-readiness claim is part of this delivery.
