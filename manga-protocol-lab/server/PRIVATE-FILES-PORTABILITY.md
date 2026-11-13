# Private file portability (reconstructed, not yet compiled)

Service signing keys, managed group keys, offline key backups and local outbox
payloads are opened through the shared `private_files` helper. Reads use the
validated open handle and remain bounded; checking a pathname is not enough.

On Unix, regular files reject symlinks with `O_NOFOLLOW` and retain the original
owner-only permission policy. New files use exclusive creation and mode 0600.

On Windows, new files receive a protected DACL at creation, before any secret
bytes are written. Only the current account, SYSTEM and Administrators are
allowed. Existing private files must have a compatible owner and conservative
DACL; null/unsupported ACLs and other allowed accounts are rejected. File and
directory reparse points are rejected by opened-handle attribute checks. New
private directories apply inheritable protected ACLs, and existing group/outbox
roots must pass validation. The program does not repair broad ACLs or grant
itself extra permissions.

The creation handle uses normal read/write/delete sharing so storage leases and
verified readback can reopen it. `CREATE_NEW` still prevents overwriting an
existing file. The owner-controlled parent-directory contract is unchanged;
this is not a sandbox against a hostile administrator or the same account.

Directory durability uses the shared platform filesystem helper. Native Windows
requires its documented volume-flush privileges. A failure is an error, not a
successful durable backup.

## Required validation

The reconstructed implementation has not been compiled or executed in this
reset workspace. Do not reuse the earlier lost-port test results as evidence for
this source. Run private-files, offline-key and outbox unit tests on Linux,
Windows and macOS. Windows testing must include broadened/inherited ACLs,
reparse-point paths, exclusive-creation conflicts, and readback while the original
creation handle is still open. The full platform signer consumes the same helper.
