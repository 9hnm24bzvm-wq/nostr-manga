# Opt-in actual-application runtime alignment

This is a runtime qualification candidate, not a product transport fix or a claim
that the complete upload journey has passed.

The actual-application run 40152070737 at bd965386 used Playwright 1.62.1,
Chromium revision 1234, executable SHA-256
`0b20b130e7edd9dd51873be867761295fe0cfad490c2b9a64f95bd3cfc08fa71`.
It cleared language selection, then recorded one HTTP-200 preview GET with native
`net::ERR_ABORTED`. The `mutation-count` substep label does not establish a count
mismatch: its poll checks observer health first. The exact raw assertion was not
exported. Four observed writes are consistent with the expected operations.

The separate bounded counterfactual run 40133173860 at 8533f513 qualified all six
expected observations using Playwright 1.64.1. Chrome 156.0.8078.47 reproduced the
complete-EOF/native-abort behavior; Chrome 157.0.8103.6 finished that same request.
Both browsers still rejected truncation and owner disposal. The report SHA-256 is
`986736d71e1e37b06d2dbd0cffe098a5dbb1281ef523503503899dfc7edbeb6c`,
artifact 12575976218. This is not whole-application causal proof; the old actual
application executable is different from both counterfactual executables.

The new `ci/hosted-browser-chrome157` workflow branch explicitly selects
`BROWSER_RUNTIME_PROFILE=cft-157` and the existing public-upload batch. Existing
application/startup workflows remain unchanged. The dedicated sandbox-preflight
workflow remains legacy-only and is not part of this actual-app job. Actual-app
launch still requests the normal Chromium sandbox, with no insecure override.

Acquisition reuses the exact existing counterfactual lock and two-archive
generation/MD5/CRC32C-checked downloader and bounded extractor. Only Chrome 157
is selected for execution. Both previously observed archive SHA-256 values are
checked before extraction, and the selected executable must equal
`ec4e78d65a5382c2b50aaf28c6a655a624973b3d3c711a2a4ae34369fd9a3444`.
Playwright and core are installed with lifecycle scripts disabled, then every
installed file is compared against its exact lockfile-SRI-verified npm cache
tarball before importing the driver. Runtime recording and loading revalidate
these bytes and the complete package inventory; missing/unknown/mismatched
profiles fail without fallback. Public preparation admits only the unchanged
legacy pair or the exact new pair and executable hash, with no extra telemetry.

One separately approved hosted run is needed: this fixed wrapper, existing
140-minute cap, isolated fixtures, no automatic retry. All upload counts,
signature checks, EOF/native-terminal assertions and cleanup requirements remain
unchanged. Combined startup/shutdown/restore fixes, if included in the published
source, must be listed separately rather than claiming a browser-only experiment.
