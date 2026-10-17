# User-upload sort navigation repair

Both retained profile-upload sort handlers built their target as pathname plus query text without `?`. Selecting Oldest from an empty query therefore navigated to `/uploadsorder=asc`; selecting Latest with another filter could similarly change the route instead of its search parameters.

The isolated overlay adds the delimiter only when an encoded query remains. Both controls retain all unrelated parameters, remove/set only `order`, and avoid redundant navigation when already selected. The two exact JavaScript expressions are explicitly pinned in the real production preparation/markup verifier. All original source bytes, DOM elements, classes and styles remain unchanged; only the reviewed inline-handler expressions differ.

The actual composed handlers passed ten meaningful navigation cases: Latest/Oldest with empty, asc, desc, additional-language and encoded Unicode/ampersand filters. A separate exact-scope/source assertion and both real gate-off/gate-on production-preparation checks passed, thirteen cases total. The same test run against the unchanged pre-fix handlers failed; raw before/after output is retained under `test-evidence/frontend/user-upload-sort-navigation-20261017/`.

These are finite navigation and real preparation checks, not rendered browser or deployed-route proof.
