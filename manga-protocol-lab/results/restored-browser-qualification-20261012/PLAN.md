# Fresh local native-browser fixture, 2026-10-12

Status: SOURCE ONLY / UNEXECUTED. No service, process seam, browser launch, syntax
check, or fixture generation has been run for these new sources. This document
does not release the hold; wait for explicit parent authorization. The earlier
native fixture controllers were deliberately excluded from the checkpoint;
these sources are fresh, not claimed byte-restored.

## Scope and evidence boundary

Use accepted `build-artifacts/linux-x86_64/manga-node`, SHA256
`4c499f724af4db1843f54c51e47746114f777de15b446979a3e8e8a0e7a3e23a`,
and all unchanged production browser files from public build `e4f4862bd75c26bb`.
The controller rechecks the 57 prior accepted asset hashes and accepted node at
startup and shutdown. It serves only exact asset-manifest paths. Do not rebuild,
install, edit production files, use private application state, seed IndexedDB,
expose an external listener, or use headless/browser-test runners. The actual
browser is dot cloud Chromium through its supported CUA tools.

1. After release, start only the small loopback reachability probe on a fresh
   19000–19999 port. Check it in cloud Chromium. Executor reachability does not
   prove browser-host reachability. If needed, launch the exact controller from
   supported native Terminal on the browser host; do not bypass a denial.
2. Run lightweight helper and real probe CLI/HTTP seams, including startup
   failure, deadline, explicit stop, signal, path/host rejection, and receipts.
3. Start one new private runtime for a tiny four-family service smoke. The
   controller starts only fixed initdb/createdb/migrate/postgres/manga-node
   commands. PostgreSQL stays in the foreground. A dedicated subreaper owns all
   subprocesses and joins exact-identity pidfd cleanup in the same launch.
4. The fixture parent supplies only a synthetic NIP07 interface to an unchanged
   same-origin app iframe. Private signer material stays in mode-0600 runtime
   files; no private keys are served or embedded in archived sources. Signatures
   are restricted to kind11999 library pointers and kind27235 requests for the
   configured loopback node's exact library/enrollment routes. Same-origin,
   exact Host, custom request header, strict JSON and 16KiB body limits apply.
5. Import the tiny deterministic JSONL through the real file chooser. Confirm
   local default, explicit service identity/admission/publication consent,
   independently decoded public root/chunks, recovery preview/replacement,
   actual reload, and actual exported-backup equality. The parent owns the
   independent public verifier. Then qualify 30000 and 500000 chapters
   sequentially, with the distribution below, fresh runtime/browser origins,
   and no concurrent load job. Old18xx origins are outside the allowed range.
   A private marker prevents silent reuse of a fixture service origin.
6. Capture terminal shutdown receipts, verify no owned remaining processes,
   and recheck accepted source/binary hashes. Runtime files/databases/logs are
   private and excluded from checkpoint. Publish only separately reviewed
   public-only setup, verdicts, synthetic backups, hashes, and receipts.

## Files

- `fixture-controller.py`: fixed probe/serve/command/status CLI; no HTTP control
  endpoint, no arbitrary command route. Fixed local file-only commands are
  node-start, node-stop and stop.
- `owned-processes.py`: ancestry/pidfd/subreaper ownership adapted from the proven
  restored-service-smoke guard. No name-, socket- or pre-existing-group kills.
- `fixture-parent.html`, `fixture-bridge.js`: labelled synthetic signer and
  unchanged same-origin app iframe. No app imports or storage manipulation.
- `synthetic-inputs.py`: streaming deterministic JSONL and explicit hash manifest.
- `source-seams.py`: actual generator/probe CLI/process/HTTP tests. `--services`
  adds one real PostgreSQL/node smoke, signing-scope requests, owned/outside signer
  path verification and node restart. No private generator/signing helpers are asserted.
- `browser_evidence.py`: fixed signed public-head/root/chunk and exported-file
  verification CLI, using the existing pinned-proof client and bounded decoder.
- `test_browser_evidence_cli.py`: four proposed actual export CLI cases, including
  changed fields, duplicate rows and missing footer. The signed public path still
  requires the real-node small-case check; these cases cannot qualify it alone.

## Input equality contract

The tiny case has counts `{chapters:1,series:1,lists:1,members:1}`. Large cases
have `{chapters:N,series:100,lists:2,members:100}`; totals are 30202 and 500202.
Chapter IDs are lowercase hex(index+1), padded to 64 digits. Chapters are complete
with null page. Series/list IDs are SHA256 of
`restored-browser-20261012:series:<index>` and
`restored-browser-20261012:list:<index>` (list truncated to32 hex digits).
Series statuses rotate reading, on_hold, plan_to_read, dropped, re_reading,
completed. List0 is "Synthetic favorites", list1 "Synthetic queue". Member
index i maps to list floor(i/50), series i, position i modulo50. Every record's
updated_at is1791840000. `expected_record(family,index)` is the source contract;
its index is generation index, not SHA-ID lexical order. Tiny uses index0 only.

The version2 `manga-local-library` manifest schema is
`synthetic-browser-input-v1`. It includes counts, total, timestamp, file,
bytes, sha256, canonical_records_sha256, family_sha256, chapter_id_rule,
namespace, statuses, list_names, member_rule, distinguished_chapter and synthetic.
Canonical equality ignores backup-envelope revision/export time: sort by family
order chapters/series/lists/members, then record key; hash compact UTF-8 JSON
`[family,record]` with sort_keys=True, ensure_ascii=False, allow_nan=False, and
one LF per record. Membership key is list ID concatenated with series ID.

Public setup fields are `origin` (node), `service_key`, `publisher`, and
`client_origin`; safe node/build hashes are included. The setup/status files in
runtime contain public metadata, but copy them into evidence only after review.
Private signing material is `.runtime-final/<run>/user.key`.

## Held command examples (not executed)

From the project root, only after explicit release:

    python3 results/restored-browser-qualification-20261012/fixture-controller.py probe --runtime "$PWD/.runtime-final/browser-probe-01" --web-port 19351 --lifetime 180
    python3 results/restored-browser-qualification-20261012/fixture-controller.py command --runtime "$PWD/.runtime-final/browser-probe-01" --action stop
    PYTHONDONTWRITEBYTECODE=1 python3 results/restored-browser-qualification-20261012/source-seams.py

After probe/process review and explicit tiny-service release:

    PYTHONDONTWRITEBYTECODE=1 python3 results/restored-browser-qualification-20261012/source-seams.py --services
    python3 results/restored-browser-qualification-20261012/synthetic-inputs.py 1 results/restored-browser-qualification-20261012/tiny-input.jsonl
    python3 results/restored-browser-qualification-20261012/fixture-controller.py serve --runtime "$PWD/.runtime-final/browser-tiny-01" --web-port 19361 --node-port 19362 --pg-port 55361 --lifetime 2700

Service fixtures enforce a24GiB sampled disk floor,1GiB available-memory floor,
32MiB total private process-output cap, fixed command deadlines,2-second inbound
socket inactivity timeout plus an absolute5-second request timer,16KiB signing-body
cap,4096-signature budget and maximum45-minute
lifetime. Node physical/staging quotas are1GiB each; max_blob_bytes is1048576.
Browser worker120000ms and application request/sign deadlines are unchanged.
The absolute HTTP timer is additionally capped by the remaining controller lifetime;
trickling headers/body cannot renew it. Expiry marks a failed fixture before raising
through blocking I/O. Even if HTTPServer catches that exception, the controller
observes the saved abort and proceeds to joined cleanup. Signals set an abort flag;
the active request can delay that path by at most its remaining timer plus scheduling.
Cleanup has its own finite waits; this is not a promise of exact shutdown at the
nominal lifetime or immunity to SIGKILL/host suspension. Both lifetime and request
deadline receipts have failure exits. Proposed real trickling-header seams cover the
request deadline, shorter controller deadline and SIGTERM during an active request.
The verifier uses at most300 seconds,32KiB retained receipt,96MiB input/export,
64KiB JSONL lines, and the existing20-second/2MiB public-response cap. It reads
the private key only from an unaliased owned .runtime-final/<run>/user.key, with
mode0700 owned runtime directory and mode0600 singly-linked regular key file, never outputs it, and
performs no writes to the application. The controller has no verifier HTTP API;
execute the fixed local CLI in the verified service host context if necessary.
Both a successful verifier exit and verified:true receipt are required.

## Limits

This can qualify synthetic signer integration and actual browser persistence and
transport. It does not qualify installed extension prompts, real user accounts,
remote signers, mobile hardware, eviction, disk-full, physical power loss, or
capacity beyond the observed run. Global resource guards do not attribute
per-process memory/I/O or guarantee a hard no-overshoot limit. SIGKILL cannot
execute cleanup, so terminal evidence is required. The tests and controller are
currently unexecuted proposals; a source review is not a runtime pass.
