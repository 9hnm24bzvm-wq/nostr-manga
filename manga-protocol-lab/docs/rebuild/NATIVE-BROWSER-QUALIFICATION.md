# Native browser qualification

Build `72e656def51db26f`, actual cloud desktop Chrome. UI interactions use forms,
file choosers and downloaded files; no private IndexedDB reads or injected app
state substitute for outcomes. This is one desktop environment, not mobile,
signer-extension, eviction, power-loss or target-throughput certification.

## Small library

`results/rebuild-browser/final-parity-ui.json` records preservation across upgrade,
128-scalar Unicode names, oversized-name refusal, save/reload with the preview
HTTP origin stopped, actual export/import and two-live-tab notification.
Automation selector/tab-lifecycle mistakes are recorded separately from app
failures. The original native truncation was not reproduced; the HTML-contract
red and successful final native boundary behavior are different evidence.

## Large synthetic libraries

Inputs and hashes are in `results/rebuild-browser/native-large-plan.json`.
For N=30,000 or500,000:

- JSONL header: `{ "format": "manga-local-library", "version": 2 }`.
- N chapter records: ID i as64-digit lowercase hexadecimal, i=1..N;
  completed=false, page=i%30, updated_at=1.
-100 series: same ID rule, status=reading, updated_at=1.
-2 lists: ID i as32-digit hex, name="Synthetic reading list i", updated_at=1.
-100 memberships: list ID(1+i%2), series ID i, position=i, updated_at=1.
- Each row is `[family, record]`; final line is `{ "end": N+202 }`.

All data is synthetic. The actual browser imports through its file picker, then
updates chapter1 to page99, reloads, and exports through its download button.
`results/rebuild-browser/verify-native-export.py` compares every key and field in
the downloaded file against the input. Only the deliberate page and its new UI
update timestamp may differ; duplicate records and wrong trailers are refused.
The ordinary export header's revision/export timestamp is not record content.

30,000-chapter outcome: passed,30,202 exact records; actual export is3,918,318
bytes. See `native-30000-export.json`. The optimized backend regression build was
running simultaneously, so this does not establish isolated browser timings.

500,000-chapter outcome: passed,500,202 exact records,64,861,650 exported bytes.
Actual import, a first-chapter progress edit, reload and download were completed
through Chrome. `native-500000-export.json` verifies every key/field. Native bulk
import/export took on the order of minutes on this host; automation polling and
concurrent compilation mean those windows are not isolated performance results.
One earlier attempt lost its automation tab before observed completion; reopening
preserved the previous30,000-chapter library. The retained-tab retry completed.
See `native-500000-interrupted-attempt.json`; no staging-occupancy or power-loss
claim follows from that interruption.

At this pre-schema-5 checkpoint, native large-library public publishing/signer
behavior, mobile responsiveness, quota eviction and physical power loss remained
unqualified. The then-current `counts()` used four IndexedDB family-range counts
on ordinary refresh. That counts implementation and its performance concern were
superseded by the schema-5 correction and measurements below; the historical
round-trip results themselves are unchanged.

## Schema 5 upgrade and exact export (2026-10-10 22:16 UTC)

Build `e4f4862bd75c26bb` reopened the existing schema-4 native cloud Chrome library containing500,202 synthetic records. The unchanged public counts result retained500,000 chapters,100 series,2 lists,100 members and revision19. Three public `counts()` calls measured0.4/0.2/0.3ms (median0.3ms), versus the preceding build's2073.6/2105.2/2075.8ms (median2075.8ms). Public `list("chapters", null, 50)` measured1.6/0.6/0.6ms. This is a three-sample shared-host diagnostic while optimized compilation was active, not isolated capacity evidence.

A normal UI save of chapter1/page99 survived reload. The subsequent real browser export was downloaded and compared against the deterministic input: all500,202 keys and every field matched, except the explicitly expected page99/UI timestamp mutation. The64,861,650-byte export SHA256 is `520dff1e379359b25abcc3f50ed9413b2bb86996eb0b9f010192e3efa6680044`.

Evidence: `results/rebuild-browser/native-counts-improved.json` and `native-500000-schema5-export.json`; comparison implementation is `verify-native-export.py`. Model acceptance for schema5 is independently recorded in `results/rebuild-family-statistics/acceptance.json` (91 checks). No private IndexedDB inspection was used as the native outcome oracle.

## Small native public snapshot and recovery (2026-10-11)

The unchanged build `e4f4862bd75c26bb` passed a four-record, four-family flow in
actual dot cloud Chrome against a disposable PostgreSQL/strfry node with SHA256
`82c52b35720e623a303ec49fad73e65e08e033baf83b4fcc4349890af174c491`.
The fresh `127.0.0.1:18768` origin used an explicitly labeled temporary synthetic
NIP07-compatible adapter. The existing `18767` library was not touched. This
qualifies the actual browser/client and real HTTP flow, not an installed signer
extension.

The rendered UI saved and exported one chapter, series, list and membership;
rejected a wrong service pin and publication without consent; and retained the
same retryable signed snapshot after a real node outage and browser reload.
Restarting the node did not publish automatically. An explicit retry accepted
the same event, and independently verified signed HTTP readback confirmed its
projection. A subsequent local edit left that public head unchanged. Recovery
preview cancellation preserved the local edit and its actual exported backup;
approval restored every field of the original four records. A fresh reload and
another real browser export matched that baseline exactly.

Evidence: `results/rebuild-browser-publication/acceptance.json` and `README.md`,
with downloaded JSONL backups, sanitized UI observations and verified signed
HTTP outcomes. All production browser assets and the node binary were unchanged;
the isolated services were stopped after the check. Native extension prompts,
large-library browser publication, mobile behavior, eviction, disk exhaustion
and physical power loss remain outside this qualification.
