# Ordinary account surface inventory — 28 October 2026

Working integration inventory, not a claim of end-to-end browser acceptance.
The ordinary account is the native browser session. A public profile UUID is
presentation/target identity, never authentication. Advanced signing APIs retain
their distinct authority; ordinary controls must not select a key or silently
switch authentication modes.

| Reader surface / route | Implementation and endpoint |
| --- | --- |
| Browse, search, title, reader | Public catalogue reads; no login or signing required |
| Register, sign in, verify, sign out | Native `/auth/self-service/*`, `/api/auth/check`; `AccountOnboarding` now only account + verification + Continue reading |
| My profile / `/user/{id}` | Public profile DTO; trusted loader mapping handles native legacy aliases to canonical UUID |
| Profile biography/contact editor `/user/{id}/settings` | `AccountProfileEditor` / `session-account-profile-client` → GET/PUT `/api/v1/account/user/{UUID}/profile` |
| Profile Block / self settings shortcut | `SessionProfileActions` reads `/account/me`, directory, follow status, preferences; explicit block merges `/account/settings` |
| Settings → Account | Existing native browser credential/settings flow `/auth/self-service/settings*` |
| Settings → Preferences | Existing local preferences form; `AccountPreferenceSave` + `AccountBlocklistEditor` use cookie-only `/api/v1/account/settings` |
| Settings → Advanced (`/settings/identity`) | Normal account status and native Manage account/My profile/Sign out; portable-key setup/profile/publication tools in closed Advanced identity tools |
| Rate title | `PersonalRating` → `session-rating-client` → cookie-only `/api/v1/account/me`, `/rating` |
| Report title/chapter/author/group/user | `PublicReportForm` → `session-report-client` → cookie-only `/api/v1/account/report/reasons/{category}`, `/report` |
| My reports `/my/reports` | Retained native session report collection over same moderation storage |
| Comments / replies / votes and their reports | Ordinary stored native thread components, no account-type chooser; original native session writes |
| Individual Follow on user/group/title | `SessionFollow` → `session-follow-client` → cookie-only `/api/v1/account/{kind}/{UUID}/follow` |
| Following titles/groups/users `/following/{kind}` | `AccountFollowing` → `session-following-reader` → `/api/v1/account/user/follows/{kind}` |
| Public-list Follow and `/following/lists` | `PublicListFollow`, `FollowedLists` → `session-list-follow-client` → cookie-only `/api/v1/account/list/{UUID}/follow`, `/user/follows/list*` |
| My Library, My Updates, History, private custom lists | Guest device records remain private. Native-account library subsystem and metadata revision migration; explicit transfer, never silent upload |
| User directory `/users` | `session-user-directory` → `/api/v1/account/user`; UserSearch uses normal Session + AccountActionGate |
| Add title, drafts, edit catalogue, upload | Cookie-only canonical contribution/upload adapters; externally signed modes remain separate advanced APIs |

## Shared ordinary-account safeguards

`native-account-scope.ts` accepts only its exact same-origin account API namespace,
uses same-origin cookies and captured native expected-account header, and never
imports a signer. `/me` must identify native-session authority; operation receipts
must repeat the captured canonical account and native-session provenance. Ratings, reports and user/group follow mutations require verified email. Existing
private owner operations (library, following reads, manga/list saves, profile and
preferences) explicitly request owner context without adding verification gates;
backend action permissions remain authoritative.

Missing login/verification has one contextual `AccountActionGate`: normal auth
opens in a labeled new tab so the task survives, then Continue here refreshes the
session and resumes a **read only**. Closing or replacing the task suppresses late
results, including a delayed auth refresh overwriting a newer account. Ratings,
follows and reports always require a distinct explicit user write action.

Focused synthetic tests are not a production deployment, live user-account test, full aggregate pass or browser acceptance. See [source checkpoint status](SINGLE-ACCOUNT-CHECKPOINT.md).

## Focused extension evidence

Normal profile/preferences/directory/block client tests use the actual native
transport with synthetic HTTP data, not a real account. They cover canonical
self-only profile CAS, native provenance, redacted directory rows, preserved
hidden settings, explicit block/unblock, no implicit unfollow, refusal vs unknown
outcome, and account changes after dispatch. Normal profile/action DOM tests
exercise actual Svelte lifecycle, including a no-effect-flush account switch;
the profile draft binding remains safe while that switch clears the form.
