# Exact popularity-client loopback probe

Test-only source: `a317a97c9eeeba6069c5759df0c31fcc6f8d04c2`.
The production frontend freeze and its755-case qualification are unchanged.

The mandatory owned-fixture hook executes the production `client.ts`,
`popularity.ts`, query and validation closure from its current checkout. It reads
both supplied canonical Manga and Chapter UUIDs at24h,7d and30d, restricted by
`ids[]`, then requires an empty204 analytics acknowledgement to add exactly one
view to each window. Repeating with the same Node visitor jar must acknowledge
without another count. Scores use exact BigInt comparison. Each successful run
records22 HTTP requests and18 window observations, with script/source hashes.

The probe accepts HTTP loopback origins only, bounds the whole run to30seconds,
each response to1MiB and aggregate bytes to8MiB. It restricts transport to the
two top reads and four exact supplied-target POSTs. Its explicit Node in-memory
jar exercises the production same-origin credential intent and backend cookie
policy without retaining cookie values. It is not native-browser persistence or
a whole mounted-application claim.

Run from the composed current checkout with `node --experimental-vm-modules
weebdex-platform/tests/ci/probe_public_popularity.mjs`, setting
`PUBLIC_API_ORIGIN`, `MANGA_ID`, `CHAPTER_ID` and optionally
`POPULARITY_PROBE_RECEIPT`. The API fixture must supply its own current canonical
UUIDs and run the hook while its synthetic server is alive. No sibling checkout
or historical script fallback is allowed.

Three lightweight HTTP stand-in selftests pass: empty/positive baselines and
same-cookie dedup; fail-closed on a duplicate that changes observed count; and
external-origin refusal before a request. The raw log, exact pins and rerun recipe
are retained under `test-evidence/frontend/popularity-client-probe-20261018/`.
These selftests qualify the harness only. Actual database-backed bridge evidence
is pending the API owner's composed-fixture run, separate from earlier backend
wire-only acceptance and the755-case frontend qualification.

## Actual owned-backend association

The subsequent mandatory hook `f505bccdc6dfbd82bbf0bdee7a7b18ee9c360dff`
passed against the actual synthetic TCP/PostgreSQL API. Source/evidence successor
`a33db0633124c6f52ab86dd6baedcd1c7520e5c6` retains the raw receipt at
`test-evidence/public-popularity-client-bridge-20261018/actual-public-popularity-client.json`.
Its SHA256 is `f3cf1885ee1be90bf0878f4351dcc36e834ff9a5654809087ee31bbbce0efc95`.

All22 requests and18 observations passed in252.578ms, consuming16,978response
bytes. Manga scores were0→1→1; Chapter scores were3→4→4 across24h,7d and30d.
This covers both a real empty initial ranking and positive baseline, confirmed
analytics increments and repeat dedup with the same scoped visitor jar. The
script hash `04c8d44ce06028d6775e70b18132ac07ee4c062a9c7b5b173173f338ffdd4653`
and all four executed module hashes match the final351cf41 frontend source.

The API hook pins script and module bytes at compile time, requires the script
from its current checkout, checks the actual canonical fixture targets and
retained receipt, and has no optional or historical-sibling fallback. The API
owner's receipt records a clean physical runtime stop. The earlier three-case
selftest result remains the historical harness-only result; this later evidence
closes its pending real-backend association without changing the frontend product
freeze or claiming native-browser cookie acceptance.
