# Genuine instance-account ratings in title controls

Backend dependency: `ef4f960aab273800ee39a1a0b60acafe06b493ab`.

The recovered frontend had no personal rating control. Public UUID mode now adds a small Rate action to the existing title row, using the native Button/Dialog components and a 1–10 score selector. It is shown only for published titles. Anonymous reading does not load an account or invoke a signer; an intentional rating action can direct the reader to existing public-identity management. No settings panel or technical accordion was added.

A rating action proves the configured service, authenticates fresh exact URI/method/service/nonce/body NIP-98 requests, reads the actual explicitly bound native account from `/user/me`, and verifies every personal-rating response's canonical account ID. A signing pubkey is never treated as the account ID or a native Session. POST/DELETE include the captured `expectedAccountId` and rating version inside the exact signed JSON. The backend transaction rejects a same-pubkey account rebind even when both accounts have identical versions. Scores, first-vote dates, current/tombstone revisions and no-op receipts are actual persisted observations; unrated is empty, never a fabricated zero.

Unconfirmed/malformed writes, CAS failures and account/service/target changes freeze further writes until an explicit fresh load; no vote is automatically replayed. Closing the dialog or switching signer/title discards late UI state. Only validated successful receipts display “saved” or “removed”. Accepted changes refresh the existing anonymous title-statistics read.

The old native star placeholder can now show the real instance-account mean from the backend's declared population, observed count and distribution. Missing or invalid statistics remain N/A. No global MangaDex population or Bayesian rating is invented. Existing unrelated comment/statistic visibility guards remain intact. The three tiny title markup additions have named, exact feature-exception checks; immutable canonical frontend files remain unchanged.

Verification: 19 rating-client/UI/summary finite cases pass; the broader library/publication/upload/bootstrap/rating batch passes 190/190; unchanged exact-byte NIP-98 transport cases pass 24/24. These execute actual production clients/component scripts/helpers with finite transport/lifecycle boundaries. Both strict composed modes pass (0 errors, 15 retained warnings; 84 native/97 public integration modules), all 679 canonical source files unchanged. Raw receipts are in `test-evidence/personal-ratings-ui-20261017/`.

The finite cases cover bound account versus pubkey, rebind/CAS collision, tombstones/no-ops, wrong-account/title/version data, uncertain outcomes with no replay, competing commands, signer/service/namespace/view change, delayed close/reset, honest aggregate provenance and exact native UI attachment. They do not establish genuine browser/DOM/layout/accessibility, real signer/account/relay, production or load acceptance. No real account or credential was used.

## Production preparation follow-on

The separate production HTML/CSS preservation verifier now enumerates the same three exact rating feature exceptions as the public overlay. Its build receipt exposes those approved exceptions. A real `prepareFrontendIntegration` regression runs against fresh copied canonical source in both modes, writes its receipt to a test-owned evidence directory, validates exact normalized title/action markup and verifies the canonical source inventory is untouched: 2/2 pass. This closes a production-preparation gap not exercised by strict type composition. It is not a full Vite/minifier or browser build claim.
