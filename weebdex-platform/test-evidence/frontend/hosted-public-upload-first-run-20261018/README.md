# First bounded hosted public-upload attempt

Run [39142927874](https://github.com/9hnm24bzvm-wq/nostr-manga/actions/runs/39142927874), attempt 1, used exact CI commit 323d0990101e6b6a21328b252444c68060ecb97f and tree 8df9b2ef48f11d5e8f5f9b3e343f54eaa464007c, corresponding to reviewed local source 9feba101. Its product baseline was published be283, separate from the later cc5 recovery client changes.

The run completed in 722 seconds. Pinned cold preparation and the 12 finite proxy cases passed. The actual compact-feed PostgreSQL test exited 0 and emitted all five required correctness flags; the deliberately expensive baseline was canceled at 5001 ms and the batched first page took 210 ms. The outer owned-command cleanup flag passed.

The native gate failed because the CI adapter admits exactly two fixture-cleanup keys, while the current producer always emits a fifteen-field lifecycle receipt. This is a deterministic schema mismatch. Only the normalized cleanup=false result is available in the one uploaded JSON; the private runner cleanup receipt was not uploaded and is not reconstructed here.

Browser execution was skipped. No Chromium process, sandbox support, IPC, upload/replacement, screenshot, mounted application or user-journey acceptance is established. No rerun or second batch is represented.

The 891-byte downloaded artifact matched its GitHub SHA256 and contained only the exact 1360-byte preparation.json. It was schema-validated by the workflow and retained for one day. Raw job logs remain outside Git; only their hash and normalized public facts are recorded here. No package, key, credential, runtime database or private receipt is included.
