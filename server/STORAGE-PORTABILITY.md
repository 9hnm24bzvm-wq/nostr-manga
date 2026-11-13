# Native filesystem portability

This reconstructed implementation is uncompiled and unexecuted after the workspace
reset. Earlier results from the lost implementation are not evidence for this code.

Linux preserves the existing boot ID + device/inode + root identity encoding and
owned mutation/read lifetimes. Windows uses full 128-bit file IDs, native capacity
queries and protected ACLs for private files/directories. Existing broad ACLs and
reparse directories are rejected. All namespace durability failures are errors.

Windows namespace barriers flush the containing local volume. This requires an
elevated service account and can impose substantial shared-volume latency. The
program does not grant itself privileges. Test both the expected standard-account
failure and elevated success; a hosted administrator CI run alone does not prove
restricted-account operation. Do not use network/cloud-backed storage. A file
rename must stay on the same filesystem; failed publication must preserve staged
and accepted data. Runtime tests and power-loss validation remain outstanding.

The legacy managed nginx/pidfd external-serving adapter is Linux-only. Non-Linux
entrypoints for export reservation/configuration/reconciliation/withdrawal/drain
reject before side effects. Built-in guarded HTTP originals are the portable path,
not static-offload performance parity. Do not clear legacy export holds. Non-Linux
identities include a process-session nonce, so old holds cannot inherit a new
process's proof. Existing public namespaces require verified Linux recovery;
native corruption quarantine only attests absence when no public tree exists.

