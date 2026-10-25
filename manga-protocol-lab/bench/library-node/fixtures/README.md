# Finite harness fixtures

`mixed-210s-bounded-plan-v1.json` is a newly reviewed, static current-plan
contract captured from source `be283b664f699e8c7175f6eac0200cd22e212c8a` with:

    python3 -B bench/library-node/mixed.py --plan --duration 210 --qualification-contract full-dispatch-bounded-completion

It is not a recovered copy of the absent historical
`results/rebuild-mixed-prep/proposed-210s-plan.json`, a workload result or
authorization to execute load. The finite test compares actual CLI output
against this checked-in contract and verifies that enabling the optional
observer changes only `additional_diagnostics`. The expected file is never
generated during the test. Intentional plan changes require separate review
of both the fixture and its coverage.

The JPEG metadata test uses the authentic public-domain Met image restored at
`server/tests/fixtures/real-images/met-57682.jpg`. Its exact 1,227,747 bytes and
SHA256 `3ac595477a047f9fa78166a88fd2a8a0306d3d6698fca655e430b5c4e1b4603a`
match the pre-existing manifest, fetched from its recorded official image URL
on 2026-10-18. No substitute or re-encoding was used. This single fixture does
not restore the other three excluded images or qualify a benchmark corpus.

## Current contract v2 after demand-accounting composition

`mixed-210s-bounded-plan-v2.json` is the separately reviewed current contract for the same explicit210s/full-dispatch-bounded-completion CLI arguments after b731 added workload_scope and demand_accounting. It preserves every previous non-caveat field exactly, replaces the page-equivalent caveat with its reviewed synthetic-fan-out wording, and adds the device-local-save/profile-sync distinction. The monthly inputs and30-day arithmetic are explicit; optional public-library publication cadence remains null and integrated-demand qualification remains false. The observer test still compares the complete parsed CLI plan exactly and checks that enabling diagnostics only adds additional_diagnostics. Expected data is never generated inside the test.

The originalv1fixture remains unchanged. Neither fixture recovers or proves equivalence to the missing historical proposed-210s-plan.json. The45/46actual postmerge red is retained separately before this versioned fixture correction. No offered-load/capacity claim follows.
