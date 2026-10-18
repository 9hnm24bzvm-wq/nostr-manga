# Native-account following collection tabs

Titles, Groups and Users now appear beside Lists in the existing following navigation, reachable from the library’s followed-lists link. The three additive routes reuse normal title cards and site tab/button styles. Anonymous visits do not request an account or signature. A connected visitor deliberately entering a following view uses the actual bound native account; the anonymous device library remains separate and unchanged.

## Supported reads and management

The private read adapter uses the qualified `265804862294a9fa82dcaeabdd3843779be047b2` backend contract: `/user/follows/{manga|group|user}` with only `limit=20`, `offset` and the declared Manga cover/author/artist includes. Every page requires the exact account, scope, persisted-library source and request-proof provenance. Canonical resource types, IDs, collection bounds and resource revisions are checked. Results retain server order and known adult titles, without importing discovery filters. Resource revisions and dates are never presented as follow revisions or follow times. Hidden or disabled resources are excluded by the actual visible collection total; no unavailable-follow count is fabricated.

Each result’s Manage follow action captures its source account, then reads a fresh per-target membership status. The source account must still match before that lookup; only actual membership status can provide mutation CAS. Confirmed commands reread the same captured collection rather than optimistically changing totals or silently capturing a different native binding. Signer generation, destruction, pending requests and view changes invalidate stale assignments.

## Independent review correction

The API owner found that a failed later-page request originally cleared the reader while leaving a Previous button available, allowing a new bound account to be captured without Refresh. The final source retains its captured reader after failure and disables both pagination buttons while an error requires Refresh. An actual component-script regression exercises page20, account rebind, page40 failure and a direct Previous-style retry: neither retry adopts the new account; explicit Refresh does. The reviewer rechecked the exact final hashes and found no remaining collection contract or account-authority blocker.

## Qualification

The final corrected source passes all 370 finite cases in the documented combined command, including 12 collection-reader cases, 12 actual following-component-script cases, 12 follow-control cases and the two real production-preparation cases. Both composed strict production-source gates pass with zero errors and 15 existing warnings each. All 679 supplied frontend files remain unchanged. Source pins, exact commands, raw finite output and strict summaries are under `test-evidence/frontend/account-following-tabs-20261017/`.

The checks reused existing warm dependency inputs through symlinks, with the 4GiB floor plus100MiB margin checked before each strict generation. No installs, dependency cache copies, cleanup, application/browser/database startup, real accounts or external publication were used. These are finite/source checks, not rendered browser or real IndexedDB acceptance. The separate integration/checkpoint approval hold remains unchanged.
