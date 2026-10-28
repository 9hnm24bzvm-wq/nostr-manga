# Reader UX verification — 2026-10-28

Review candidate based on `1fbe4f4f9ca17463274023772a6c757427f0db5c`; not merged into main or production-deployed.

## Final source checks

- Applicable standalone suite: **5,254 passed, 0 failed, 6 skipped** (5,260 total).
- Final frontend build: **passed**, 465 files, 7,697,443 bytes; public catalogue UI enabled.
- Generated CSS: **1 passed**. Strict additive source association: **2 passed**, binding 55 reviewed sources.
- All final artifact and source bytes were reverified after the suite.
- [Receipt](verification-receipt.json) records exact artifact/source hashes. Raw logs, screenshots, runtime state, keys and personal data are excluded from this publication.

This is an applicable-suite result, not an all-authored-tests pass. [Exact exclusions](suite-exclusions.json) name historical source-bound qualifications and unavailable predecessor fixtures. Historical receipts remain historical; no inherited browser, release or production qualification is claimed. Positional historical qualification CLIs were not rerun. The final CSS test ran separately. Strict current-source restoration is tested against the immutable base without weakening prior pins.

## Browser evidence and follow-up

The disposable browser walkthrough checked fresh-reader navigation, one-follow Updates, reader idle controls, profile Groups, identity setup and task return, list presentation and language filters. These checks passed on earlier immutable fixture builds, **before the final language-selector/schedules build**. A mock rating of **10/10** was saved and read back. Recovery-key download and reimport passed. These are bounded synthetic checks, not real-user publication or production evidence.

### Final-build upload follow-up (15:02 UTC)

The previously pending mock upload was completed on the exact delivered frontend build. The named English language selector and collapsed Scheduled uploads disclosure were checked live. A session-only synthetic identity was created, its recovery key downloaded privately, and the fixture provisioned only its bounded publishing permissions. After explicit consent to the local Terms/Guidelines, one generated PNG was submitted as **Chapter 3 — Generated UX test** to **Synthetic signed upload group**.

There was exactly **one publication submission**. The initial response said the request was accepted but a readable chapter receipt was unavailable. The explicit **Check saved publication receipt** action safely recovered **Saved chapter restored** without a second commit. The title list then showed the chapter, and the reader rendered the actual generated page (640 × 960 pixels). This verifies the upload/recovery/read journey, not instant publication or an uninterrupted success dialog.

[Sanitized follow-up evidence](final-upload-browser-verification.json) records the exact temporary URLs and build identity. These loopback URLs are evidence only: the fixture has been cleanly stopped, all owned processes terminated, all loopback ports closed, and generated server keys removed. Both final teardown and full lifecycle were clean. An earlier disposable attempt hit a reserved synthetic-email collision during fixture provisioning; it stopped cleanly and was replaced without changing production code or relaxing permissions.

Earlier broad browser checks remain scoped to their earlier builds; this follow-up is not a complete final-build browser regression or production-readiness claim.


## Privacy and authority

Identity setup is centralized and explicit. Creating/downloading a recovery key does not itself acknowledge backup, select/activate an identity, admit a publisher or submit the interrupted task. Private-key custody stays client-side. Public source delivery excludes credentials, private keys, runtime databases, sessions and manga image datasets. Deliberate synthetic test fixtures are not production authority.

No main merge, force update, Actions dispatch, deployment or production-readiness claim is part of this delivery.
