# Reporting transport comparison successor

This separate test-only successor associates the reviewed reporting/discussion
cap classification change with the earlier exact transport source. The existing
seven-row integration association and native-user historical metadata are not
modified. No old pipe/timing receipt is promoted to the new transport bytes.

One physical file has two separately bound views: raw and QA-stripped. Each
view validates exact current bytes/SHA, unique literal reversals, and exact
previous bytes/SHA. The entire new metadata document is digest-pinned. Tests
prove stripping and reversal commute, and that each predecessor matches the
unchanged native-user association before its older reversals are applied.
Unknown source bytes, paths, modes, metadata, partial/duplicate literals and
reordered metadata fail closed. Only tests import the comparison helper.

Focused actual-current finite transport recovery and comparison controls:
104 passed, zero failed, one pre-existing optional original-transport
counterfactual skipped. This uses explicit in-memory primitive adapters. It is
not browser, HTTP, database, real pipe timing or runtime acceptance. All old
historical receipts keep their original source identities.

Private prerequisites: reviewed reporting commits 3fe06008, 1e95cfd and b922c9d.
The declaration-only 6417d0a helper was already byte-identical and was not
reapplied. The integration commit for this directory contains only tests,
comparison metadata and evidence; prerequisite product commits remain separate.
