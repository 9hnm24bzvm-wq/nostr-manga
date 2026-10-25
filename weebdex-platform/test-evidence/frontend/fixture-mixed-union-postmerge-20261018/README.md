# Fixture/mixed postmerge finite union

The isolated base is f1d54c027402829ca76fd3cf2f228a92ceef973a. Composition review confirms exact reviewed 810 test_mixed bytes, b731 mixed.py, b269 observer/static-v1 fixture and authentic JPEG before the separately reviewed fixture successor.

The deduplicated union is five files and 46 methods: test_mixed 14, test_diagnostic_observer 9, test_image_shards 18, test_process_metrics 3 and test_diagnostic_process 2. test_mixed already belongs to the previous four-file 37-method suite; it is counted and run once.

## Actual red and reviewed correction

The first exact postmerge run is preserved as 45 passes and 1 assertion failure. Its static current-plan fixture predates the reviewed b731 accounting/scope/caveat additions. The original v1 fixture remains unchanged. Commit 0474ebc adds an explicit static v2 with field-by-field rationale, preserving all other prior fields. The observer changes only the versioned basename and retains whole-plan equality plus the diagnostic-enabled-only delta checks. A separate development --plan probe confirms the explicit proposal; tests never generate their expected plan. Fixture-owner independent source review cleared the exact candidate before rerun.

## Actual full corrected rerun

All 46 unchanged methods passed, with 0 errors or skips, in 35.782 seconds. The 90-second owned-group guard exited 0 in 36.329 seconds without a resource trigger or survivors. The 71 tracked source pins and 47 warm dependency entries (bytes, modes, nanosecond timestamps and symlink targets) are unchanged before/after. Raw green and red logs are retained as deterministic gzip with their raw/gzip hashes. The initial setup-script transcription SyntaxError occurred before any test and remains separately disclosed. A later packaging-only assertion assumed uninterrupted verbose lines; three asyncio diagnostics interrupted them. The original log is unchanged and its exact 46/OK summary and guard success are verified.

The authentic 1,227,747-byte JPEG and its existing hash assertion, explicit MIXED_JPEG_SOURCE, and separate synthetic-JPEG seam remain intact. Only one of the four-image original corpus is recovered here. Original previous 37-method receipts are untouched.

## Scope and reproduction

Use run-union.sh from this worktree root under run-guarded.py, with the existing exact cbor2 5.6.5/coincurve 21.0.0 target named in the retained recipe. The target was reused without install, download or cache copy. Python 3.12.14 and Pillow 12.3.0 are recorded; actual aiohttp 3.14.4 differs from project 3.13.5. The recipe starts no application server, database or browser. Some finite tests deliberately own controlled loopback endpoints, subprocesses and synthetic worker workloads.

This evidence establishes those finite harness methods, not actual application load, capacity, full image-corpus recovery, a recovered historical 210-second plan, or browser acceptance. The owned-group guard samples process groups and resource floors; it is not an OS syscall sandbox or a complete pidfd ancestry proof. The runtime lease was explicitly released to the separately approved offline compact correctness lane after physical stop.
