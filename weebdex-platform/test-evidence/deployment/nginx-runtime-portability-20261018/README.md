# Official nginx runtime portability, 2026-10-18

**PASS: eight actual original supervisor lifecycle methods**, unchanged test
source, using official nginx **1.30.6**. Frozen product source is
`59c9c910b571fac5b97cbe78b2f01a694fadfa91` (tree
`c76a13c780836b4ca668685b125633fce78aee00`), from verified restored checkpoint
`19b646eb123c3a6323113716d3166bcea8030303`. Two additional finite source-wiring
checks and Python syntax compilation pass; these are separate from historical
full-suite counts. No global installation or system permission change occurred.

## Observed defect and bounded correction

The official package compiles absolute `/var/cache/nginx/*` temporary paths.
The original unprivileged supervisor inherited those defaults and failed startup.
A direct actual `nginx -t` against its unchanged generated configuration confirmed
`mkdir /var/cache/nginx/client_temp` failure. That failing source/binary association
and the actual one-case startup failure are retained in `retained-attempts.json`.

The fix creates all five HTTP temp directories through the existing canonical,
nonsymlink, owner/write-permission validator, explicitly scopes them to the owned
runtime, and selects its existing runtime log with `-e` before parsing the config.
Ownership, durable child gates, process-birth/pidfd signals, recovery/drain and
HTTP routes are unchanged. Each passing case additionally checked all five
actual directories and the owned startup log under a private runtime parent.

## Actual coverage and limits

The eight methods cover dead-master refusal, duplicate/alternate owners,
oversized control, owner EOF/successor, prepare/activate/suspend/stop, owner
SIGKILL/recovery, slow unlinked Range/drain, and wrong-owner refusal. The actual
package binary is supplied through an explicit test-module configuration
override; the historical `vendor/nginx-1.28.0` test constant remains unchanged.
The launcher adds only read-only argv/port/path/log checks, failure-log retention
and pre-unlink block accounting. No modeled nginx, relay or browser is used.

The deployment-template parser test uses an isolated **mapped copy**, not the
literal deployed environment. Its exact substitutions are four TLS certificate/
key paths, two access-log paths, and all three listen directives mapped to two
unprivileged numeric loopback ports. The wrapper adds private pid/startup-error/
error-log/temp paths, default access logging off, and events/http contexts.
Official `nginx -t` passes this copy and rejects the unquoted-brace negative case.
Nginx test mode initializes sockets: an earlier literal attempt hit a denied
privileged bind and created no listener. Only the corrected numeric-loopback
fixture is accepted. Certificate trust, literal privileged deployment, real DNS,
TLS public service and full reverse-proxy HTTP behavior remain unqualified.

## Provenance, resources and cleanup

The original workspace/runtime disappeared before resumption. The exact approved
81,022,976-byte old database and manifest were absent, so no deletion was
performed. Source was freshly restored and tree-verified. Drivers here were
reconstructed as new validation work; historical runtime continuity is not
claimed. Their exact final code, hashes, original failures and two green results
are retained. Driver files are forensic exact copies, with the original sibling
setup/source directory layout recorded in their code; they are not a production
bootstrap or a portable one-command installer.

The official package URL/size/SHA256, ELF hash/version, five source hashes and
all per-command log hashes are recorded. Only package extraction occurred;
maintainer scripts, global installation, rebuilding and service-manager changes
did not. The initial approved package setup was reused by later attempts.

All seven guarded attempts total **25.758 seconds** of the 420-second cumulative
execution cap. Investigation, harness repairs and source editing extended wall
clock time beyond the initial roughly seven-minute estimate; no single service
was left running during that work. Final guard: 4.312 seconds, 64 MiB filesystem
allocation envelope, 512 MiB owned RSS and 1 GiB available-memory threshold,
4 GiB +256 MiB start/4 GiB +128 MiB live disk reserves.

The final allocation envelope includes **all retained attempt directories plus
10,801,152 measured blocks-bytes** for the deliberately unlinked 10,800,000-byte
original while its reader may still hold it. The final recorded peak envelope is
16,789,504 bytes, avoiding the directory-only sampler omission identified after
the first green run. Resource-complete same-source repetition remains inside
the original cumulative execution and allocation caps.

The final guard physically verifies 54 owned process births stopped normally,
all ten recorded numeric-loopback ports closed, no escalation, survivors,
unknown births or cleanup errors. Some owned children disallow socket-FD
inspection; exact source/argv/port records plus closed-port checks are used,
while process-birth/pidfd ownership remains unchanged. Source hashes matched
before/after. Original package/runtime/logs, parser fixtures and synthetic TLS
private keys stay outside Git. Published evidence contains only source, counts,
commands, hashes and sanitized terminal receipts.
