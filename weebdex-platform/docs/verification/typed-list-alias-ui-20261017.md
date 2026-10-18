# Typed public-list alias acceptance

This small successor qualifies nonpreferred public-list UUID URLs against backend `cad7441bbd129799a188984c7585ba138c059154`. It extends the independently reviewed exact-request boundary in `45e7888`; it does not weaken target validation or confer ownership/follow authority.

When either source envelope or entity supplies `nostr.identity`, both copies must be exact four-field records identifying `kind:list`, the normalized captured request UUID, the actual returned canonical list UUID, and `source:persisted-public-resource-ids`. Every field and both copies must agree before a differing returned UUID can be accepted. Missing, malformed, wrong-kind/source/request/canonical, extra or inconsistent proof data fails closed. Older canonical responses without this extension remain exact-ID compatible. A differing UUID without the qualified proof is still refused.

Membership hydration and the final head/root/owner/name/order check use the proven canonical list. Follow intent subsequently uses that canonical target with the separately captured real native account and original preview head; the alias proof itself never supplies account authority.

The focused 28-client plus 11-component cases pass, including the original unrelated-target regression and a composed alias→member hydration/head recheck→canonical follow-intent case. The combined 319-case source/seam/preparation batch and both strict gates pass, with zero strict errors and the existing 15 warnings. All 679 original frontend files remain unchanged. The backend owner independently confirmed the adapter matches the qualified server contract. No browser, real account, new dependency or deployed-service qualification is claimed.

Evidence is retained under `test-evidence/frontend/typed-list-alias-ui-20261017/`.
