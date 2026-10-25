# Normal co-located browser capability probe

Installed capabilities were verified: reviewed Playwright and playwright-core 1.62.1, the existing system Chromium wrapper and real browser binary. The bundled revision1234 browser download is absent, but the existing project runtime loader explicitly supports system Chromium. This is not a missing-browser diagnosis.

One bounded normal launch used chromiumSandbox=true with no custom browser arguments, no download, no escalation, no tunnel and no security/network setting change. The owned synthetic HTTP fixture and browser launcher ran in the same command environment. Chromium aborted before any page at process_singleton_posix.cc: socket() failed: Operation not permitted. Crashpad also reported its database prerequisite, but the terminal permission refusal is explicit. No bypass or second attempt followed.

The capability result is blocked, not browser acceptance. The local fixture received zero requests and closed. The probe ran0.437s, sampled215,592,960bytes peak owned RSS, no resource-trigger stop, no surviving known descendants or monitor cleanup signals. Playwright reported Chromium terminating with SIGABRT and completing its temporary-profile cleanup. The supervisor tracked PPID lineage plus exact births/pidfds, including detached browser descendants.

Current blocker: this execution environment does not permit the normal Chromium startup socket operation. A supported environment capable of hosting the unchanged browser and application together is still needed for genuine E2E. The prior denied bootstrap was not repeated; installed binaries do not make this normal-launch result a pass.

Raw log bytes are retained losslessly as gzip. The probe source/driver are the actual historical diagnostic, with their original local dependency path; they are not a portable CI entrypoint. Product source/builds/dependencies were not modified, and no actual application, account, backend or publication journey ran.
