# Detail statistics target binding

The retained detail bridge previously required one statistics entry but accepted
any returned UUID. A malformed response could attach another resource's counters
to the current title, chapter, or group.

Exact canonical matches retain the existing one-request path. A different UUID
must now have the existing server `/legacy/resolve` endpoint's exact typed alias
proof in the current catalogue namespace. Resolver refusal, unresolved input,
wrong kind, or a different preferred UUID fails closed. Genuine aliases remain
supported, as required by `docs/public-statistics-v1.md`; merely demanding byte
identity would incorrectly reject that supported API behavior. Error responses
are returned unchanged. No native HTTP or write fallback was added.

Evidence: `test-evidence/frontend/detail-statistics-target-20261017/result.json`.
The same 62 finite tests fail 7 cases against the pinned pre-fix module and pass
all 62 with the correction (51 prior tests plus 11 explicit new cases). The actual
composed gate-on strict check passes with 0 errors and 15 retained warnings;
679 canonical frontend files remain unchanged. An initial missing local build
alias failure is retained alongside the corrected check. Dependencies were reused
only after their existing manifest was verified. No browser or live HTTP runtime
was run for this fix.
