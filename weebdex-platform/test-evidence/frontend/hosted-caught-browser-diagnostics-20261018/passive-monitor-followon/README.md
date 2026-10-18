# Passive-monitor review correction

Exact source `26a79f6775f4b52ae745c9fab5f1b321faa50378` follows historical
`d57a1f0` / `c7a45aa`. Only one runtime expression changed: the monitor captures
`error.code` once inside a guard with fixed `unknown` fallback. Independent
review reproduced the old thrown-getter exit change and unstable second-read
allowlist bypass. Its separate control evidence retains the red/green details.
This directory does not claim to contain those separate controls.

The same full affected recipe, retained byte-for-byte at the parent path, now
passes 76 Python methods, eleven direct Node lifecycle control groups and two
actor source-association cases. The new eleventh group checks thrown/unstable
getters without a process or browser; the other commands retain their earlier
real harmless process-seam scope. Python 7.908s, complete guard 8.540s/30s,
no signals or owned survivors. All31 selected source/fixture/workflow pins
match both before/after and exact committed bytes. Prior receipts are untouched.

No Chromium, service, database, network fixture, build, install, host-security
change or CI retry was performed. The existing sandbox, workflow, app source,
serialized schema and all acceptance/refusal gates remain unchanged. A future
single-public-upload run remains separately unapproved. The directory manifest
binds all other files; raw output is retained as lossless gzip.
