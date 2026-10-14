# Reader and home failure follow-up

## Actual baseline

Verified hosted run38584846100 used public commit e1ea103f2d8c0d8091eb63acf11f240c1e350591 and tree6950c9b7a819874d630831e61ea1907a0c8c40fc, identical to local reviewed94c00b8b0688b121499743ed213e465eec69bc37.

All controlled403, controlled404 and delayed-reader checkpoints passed. Normal previous navigation also passed its exact intended last-page route/title, same-realm and persisted completion assertions. Only its later unrelated-destination checkpoint failed. All account-failure cases, including every title-unfollow429 checkpoint, passed.

Home capacity429 and private expired-cookie401 cases passed. The real PostgreSQL503/recovery and restricted recovery-cookie403 cases failed without inner checkpoints. The real PG fault was activated and its terminal cleanup was clean. No page errors or proxy crashes were reported for these phases.

## Reader correction

The normal success case is uniquely on the end-of-chapter screen. The generic first title link can therefore select the retained end-screen Title link, whose click handler stops propagation and immediate propagation. SvelteKit's ancestor click interception cannot receive that event, allowing normal browser document navigation. Earlier cases selected a sidebar link with no such handler.

The normal step now selects the retained current sidebar header h1 title link. This is the ordinary same-document traversal needed by the original same-realm test. Product click handlers, intended last-page state and loading behavior are unchanged. Nine nested fixed checkpoints distinguish title control, title route, chapter link, page-one route/title, decoded bytes, same realm, persisted page, incomplete status and settled status. The original destination assertions remain unchanged.

## Home503 correction

The exact composed HTTP wrapper retries native429 and transport failures, but settles a503 response once. The former step expected three503 rows without issuing further user gestures. It now obtains those same three native failures from the initial request plus two explicit visible Retry updates gestures during the same bounded real outage. These are not automatic503 retries.

Each gesture must yield exactly one new503/unavailable row, visible owned error and actionable idle retry control before the next gesture. After the third failure, an additional bounded stability assertion rejects background replay. The original exactly-three503, no-false-empty, fixed-cluster recovery and visible post-restart200 assertions remain. The original15-second outage, independent watchdog, fixture/phase deadlines and finally recovery/route-observer cleanup remain unchanged. No503 product retry behavior was added.

Fourteen PG checkpoints distinguish capability, open, held read, begin receipt, actual outage, initial failure, second/third denied explicit retries, exact three denials, truthful nonempty/error state, resume receipt, fixed recovery, recovery retry and recovery read.

## Restricted403 diagnostic boundary

No product cause is established for the final home case. Fourteen fixed checkpoints now distinguish login, logout, visible recovery request, genuine mail/link validation, cached normal session, genuine303 redemption, restricted auth shape, native403 prerequisite, widget navigation, feedback, single read, explicit retry and repeated denial. The original checks and credential/observer cleanup are unchanged. Values, tokens, URLs and exception strings are not exported by the checkpoint recorder.

## Verification

-39 new reader/home tests: exact composed HTTP503/429/403 settlement semantics; retained link event ownership; strict explicit-retry response/idle behavior; actual PG and restricted403 step bodies, including every checkpoint failure and cleanup
-64 reader/account tests, including all nine new nested reader-destination failures
-Eight relevant Node suites:133 test results passed, including exact-lock dependency-backed title selector tests
-Python CI projection/gate suite:44 tests passed
-Existing composed semantic seams:132 cases passed
-Canonical attachment parity:676 files identical

Every original assertion and wait expression remains ordered and unchanged. UI10 retains80 insist and16 until calls. UI11–16 retains all106 original insist,43 until and10 stable calls, plus an initial single503 invariant and a no-background-replay stable check. All25 UI10 and26 UI11–16 original step-call expressions are unchanged.

No browser, application service, database, hosted workflow or external publication was started by this follow-up. Source and stand-in seams do not establish a new genuine browser pass. A fresh reviewed hosted run is still required.
