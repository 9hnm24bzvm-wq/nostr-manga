# Native platform port: validation status and deployment constraints

This reconstructed source is not a verified native Windows release. The earlier
unpublished implementation and build cache were lost when the workspace reset.
No earlier test result establishes that this reconstructed tree compiles or runs.

## Intended binaries

The Rust backend builds `weebdex-platform` and `weebdex-admin`; it depends on the
sibling `manga-node` crate. A matching supported Rust toolchain and C build tools
are required for dependencies. Do not assume a Linux build establishes Windows
or macOS compatibility. Inspect the actual Windows PE imports and test on a clean
machine before deciding which VC runtime, architecture and OS versions to support.

The frontend assets, PostgreSQL installation, database migrations, operator
configuration and private signing keys remain separate deployment requirements.
Do not package runtime databases, private keys or credentials into distributable
archives. The protocol-only distribution must exclude tests and test helpers.

## Storage and signer requirements

Staged files, pending-write journals and derivative originals use the shared
private-file creator, so permissions/ACLs are applied before writing any bytes.
Upload leases compare native identity of opened files; equal bytes or path names
are not identity. Missing, replaced, symlink or Windows reparse-point lease names
must not be accepted. A write-capable file handle is used before flushing an
original at the uncertain-commit boundary. Directory durability goes through the
protocol crate's platform filesystem helper and errors are propagated before a
database reference can commit.

The current Windows durability design flushes the containing volume rather than
silently treating an unsupported directory flush as success. This requires an
appropriately elevated account and may affect unrelated workloads on that volume.
Do not grant elevation automatically or alter security settings during setup.
Test both the restricted account's expected fail-closed behavior and the elevated
account's actual publication/recovery behavior. A hosted CI runner normally runs
as administrator and does not establish ordinary-user support.

Signing keys must be regular non-reparse files with private permissions/ACLs.
The shared key helper validates the opened handle; the platform reads at most
129 bytes from that same handle and rejects content beyond its 128-byte bound.
Never weaken these checks to make a Windows launch succeed. Provision an account-
restricted storage root and private signer files through the reviewed operator
procedure, not by embedding secrets in configuration or source.

## Deployment and recovery limitations

Existing systemd, nginx managed-reader, shell and `/proc` backup procedures are
Linux-specific; they are not a native Windows service or native recovery system.
The portable core must reject unsupported external-reader operations without
clearing historical holds or acknowledging unverified reader shutdown. Built-in
Rust HTTP image delivery must be tested natively; it is not proof of nginx offload
performance parity.

In particular, `private-checkpoint.py` needs a separately reviewed native backup
ownership/lease design. Closed ports alone cannot prove every writer has stopped.
Do not claim full cross-platform recovery until disposable native backup/restore
and media/authentication validation have actually run.

## Required verification

- `cargo check --locked --manifest-path weebdex-platform/backend/Cargo.toml`
- Core and platform library tests on Linux, Windows and macOS.
- Upload lease identity, replacement, concurrent-reader, process-death, uncertain
  commit, sharing-denial and filesystem-flush failure tests on native Windows.
- Startup, PostgreSQL migration/verification, publication, reading and recovery.
- Record actual OS, architecture, runner identity, PostgreSQL version and binary
  dependencies; distinguish process-crash evidence from power-loss durability.

No native validation result is recorded by this document.
