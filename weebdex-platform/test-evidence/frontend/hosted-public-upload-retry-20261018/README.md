# Bounded hosted public-upload retry

Run [39144824203](https://github.com/9hnm24bzvm-wq/nostr-manga/actions/runs/39144824203), attempt 1, used exact CI commit 3e019d2d274dd2f9ca7a39c094c219bfc5ad260b and tree 3c5a47543db06c2304ba83f2bfcdd4f8988a3280. It corresponds to the reviewed two-file cleanup adapter/test repair c963d383 on the earlier be283 product baseline. The later recovery client is outside this CI source.

The 592-second run passed cold preparation, 12 proxy cases and the actual PostgreSQL compact-feed prerequisite, including the complete fixture-cleanup validation. The native baseline was canceled at 5001 ms; the batched first page took 151 ms and all five correctness flags passed.

The owned application fixture reached readiness. The Node browser runner then failed with zero recorded phase steps. The invalid public-upload diagnostic record contains default zero mutation/request counters; these are fallback values, not verified traffic observations. The reported zero page-error count also does not establish a page was created. Its retained closed diagnostics contain no useful failure category: stage=complete and fatal kind/code/stage=none. An exact browser cause, Chromium launch and sandbox support are therefore unestablished. No sandbox-permission or application defect is inferred from these incomplete diagnostics.

Final fixture teardown and outer cleanup were verified, with all ports closed, keys removed, no remaining owned processes and no forced outer cleanup. The separate fixture lifecycle-clean flag is false and is preserved.

All three schema-validated JSON artifacts are retained verbatim. The 2313-byte ZIP matched its GitHub digest; its three JSON members total 4342 bytes. Raw job logs, private fixture/browser logs, packages, keys and databases are not included. No third hosted attempt or genuine browser acceptance is represented.
