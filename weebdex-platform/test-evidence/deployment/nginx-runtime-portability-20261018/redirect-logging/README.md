# Redirect log privacy follow-up

Source `4351752c3188aea9199383aa27b865e193f58ea7` adds explicit `access_log off`
to the HTTP redirect server. It prevents inheritance of a caller's combined
request log, which may contain query strings, while preserving the exact 308
redirect URI. No HTTPS, forwarding, body-budget or ownership behavior changes.

The existing proxy finite file now passes 12 cases (one new case), including the
omitted-directive inheritance witness and exact redirect target. This does not
add a Node file or revise historical full-suite counts.

Official nginx 1.30.6 accepts the mapped parser fixture and rejects the same
unquoted-brace negative case. This follow-up deliberately supplies a combined
parent log directed to a private fixture file; the redirect's source-level off
directive remains present. Mapping remains limited to four TLS paths, two log
paths and three loopback listen directives; wrapper pid/startup/error/temp paths
stay private. No literal privileged deployment or live redirect request/log
behavior is claimed.

The parser-only guard passed in 0.293s; five owned births stopped normally and
two loopback ports closed, with no survivors, unknown births or errors. Combined
with prior attempts, guarded execution is 26.051s of 420s; this follow-up peak
named allocation is 6,062,080 bytes of 64 MiB. Runtime/package/TLS keys remain outside
Git. Source hashes matched before/after.

The earlier exact 59c9c910 eight-method source/evidence is unchanged, including
its deliberately-unlinked inode resource proof. No unaffected lifecycle rerun
was performed. This directory has its own manifest; the parent manifest remains
the historical 0b1c1ed nine-file association.
