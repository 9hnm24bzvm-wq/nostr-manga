# Final popularity/blocklist production Vite builds

Exact source 351cf41bfa2955e37cc3ac90cf4b4f37b965bcc9, tree ee48a2664a5ab89b4863b0bfa9440ffaf16afe7f. Both actual production profiles passed sequentially in a fresh owned worktree with the existing verified dependencies.

- Gate off: 44.357 s, 449 files, 7,266,474 bytes.
- Gate on: 46.364 s, 450 files, 7,305,886 bytes.
- All 899 physical artifact files were independently rehashed after both runs.
- All 917 frontend/build inputs, 679 canonical files and 39,226 existing dependency entries were unchanged. The 1,273-input snapshot of this sparse frontend-validation checkout is preserved before/after; it is not a full backend compilation manifest.
- Both pidfd-owned build trees stopped without signals, survivors or resource stops. Bounds: 2 GiB Node heap, one Terser worker, 900 s, 4 GiB disk floor plus 128 MiB cleanup reserve, 4 GiB owned RSS and 1 GiB shared available RAM reserve. No dependency installation/copy or deletion occurred.

`strict-build-association.json` independently verifies all 917 Vite inputs against the separate 919-entry, 755-case/full-strict evidence 36ebad0. The two extra app-contract hashes match the exact tested Git source; they are outside this frontend-only sparse build checkout. Both strict gates report zero errors, with 15/12 warnings. Integration d633690e has no frontend/build/contract difference from the tested source; subsequent unrelated source changes need their own scope association.

The actual integration manifests and exact raw build logs (gzip, unchanged decompressed hashes) are retained per profile. Physical artifacts remain at the owned paths recorded in `result.json` for checkpoint packaging. The historical shared build receipts were restored to their original bytes after capturing both actual outputs in this scoped directory.

This establishes production build success and the declared source association. It does not establish native-browser behavior, production load or a new full backend matrix. The existing supported browser execution blockage is unresolved; no browser retry, network/security workaround or tunnel was used.
