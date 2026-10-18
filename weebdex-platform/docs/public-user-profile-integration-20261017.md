# Public UUID user profile integration

Public UUID uploader/profile links now use the existing supported `/api/v1/user/{uuid}` read endpoint and retain the normal user Overview/Uploads layout. Public user presentation preserves the real username and roles. Native profile versions are retained when supplied; protocol-publisher profiles that have no mutable revision do not receive an invented version.

The native layout now uses the existing absent-thread-safe comments provider. No thread IDs, comments statistics, biography, permissions or account session are fabricated. Unsupported native follow/block/profile edits are not exposed on these public presentations. An explicit read-only marker prevents their controller from borrowing a site Session to read or mutate another identity's follows. Existing native-mode Group/User controls remain unchanged.

No new Settings panel, accordion or identity screen was introduced. Public catalogue activation remains explicit and defaults off. Account/follow read APIs recently implemented by the backend worker remain a separate integration: the retained follow-list methods currently have no visible Following route consumer, and these public profile reads do not imply authenticated native-account authority.

Fresh finite regressions pass 120 checks covering profile normalization/omitted revisions, exact public API routing, read-only actor separation, existing actor follow lifetime behavior and upload provenance. Both composed strict modes pass with zero errors and 15 retained warnings. `test-evidence/public-user-profile-20261017/summary.json` records hashes and exact scope. These results do not establish genuine browser, physical layout, production build or complete API parity.
