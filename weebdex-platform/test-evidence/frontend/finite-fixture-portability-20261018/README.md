# Finite fixture portability, 2026-10-18

PASS: both repaired finite modules, 20/20 cases, no skips or failures. The owned
60-second runner completed in 5.343 seconds (unittest 5.043) and verified no
surviving owned group members, with no cleanup signals. Source is frozen at
`b269bf2e8aa45c921c33e6cd9ada5d60592f2e0a`; actual affected file bytes match
that commit after the run. The prior JPEG and exact historical observer test
methods separately reproduced their original FileNotFoundError before repair.

The single public-domain Met JPEG is recovered verbatim from the original
manifest URL, exact size/hash, not generated or re-encoded. Its COM variants
retain the original stream and an additional actual Pillow check confirms
identical decoded pixels. The other three dataset images remain absent.

The historical 210s proposed plan is NOT recovered. A newly reviewed static
current-plan fixture replaces that brittle historical dependency; the renamed
test verifies actual CLI output and the observer-only metadata delta. Historical
47/49 raw results remain untouched. This does not establish historical-plan
equivalence, restore benchmark results or authorize load. Observer temporary
files now live in the runner-owned TMPDIR without an ignored results directory.

Two previously approved official hash-pinned CPython3.12 wheels were recovered
into an isolated target: cbor2 5.6.5 and coincurve 21.0.0. The exact prior installer
was reused with only owned output paths adjusted. Official metadata was rechecked;
--no-deps/--only-binary/--require-hashes/no-cache/no-compile are retained. Its raw
`dependenciesInstalled:false` means automatic additional/transitive packages,
not the two explicitly installed packages. Setup took 11.105s and 3,624,960 peak
allocated bytes, below 64MiB/180s. No package/wheel/runtime archive is published.
Current aiohttp is 3.14.4 versus required 3.13.5, so full requirements parity is
false. No other package version was changed.

Only the two finite modules ran. Their small controlled signed HTTP/CLI/worker
fixtures are harness seams, not a large benchmark, application load, full
repository qualification, browser or deployment.
