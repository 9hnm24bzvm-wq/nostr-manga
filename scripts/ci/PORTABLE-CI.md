# Portable CI scope

The workflow is prepared for explicit publication/run authorization. It has not
been executed by the reconstruction task. Syntax checks are not native validation.

The full development branch runs core and platform library tests, the portable
filesystem and storage-lifecycle targets, and exactly one ignored native
PostgreSQL projection acceptance test. A zero-test projection result fails.
PostgreSQL is a new disposable cluster under the runner's temporary directory,
not a service or production database. Stop acts only on that owned data directory.
The helper masks a generated fixture password and exposes it only to the job.

Windows/Linux tools are discovered from installed PostgreSQL server directories;
macOS prepares Homebrew PostgreSQL 17 if absent. A missing installation fails
rather than silently skipping projection testing. The discovered server version,
runner identity, OS and architecture are recorded in job output.

Only four explicitly named release executables, SQL migrations, selected operator
documentation, build metadata and checksums enter the artifact. Core artifacts
contain no tests, fixtures, scripts, credentials, source or runtime databases.
The main backend package is not a complete frontend/deployment distribution.

Successful hosted-runner execution does not establish clean-machine dependency
availability, restricted-account Windows behavior, filesystem power-loss safety,
backup/restore correctness or performance parity. Inspect PE imports and native
runtime dependencies before a Windows release; use representative deployment
storage and accounts for acceptance. Windows volume flushing currently requires
an elevated account. Native tests must not be relabeled as non-elevated evidence.
