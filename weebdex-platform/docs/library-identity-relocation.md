# Library and public identity route ownership

Management belongs in existing anonymous-accessible Settings → Preferences:

- `/settings/preferences#local-library`: device/account choice, device backups,
  saved-record editing, custom lists, optional public sync and verified recovery.
- `/settings/preferences#public-identity`: external signer, public profile and
  follow/relay metadata, metadata withdrawals, and exact public-action recovery.
- `/library` always exposes **Manage library**, before any account/scope fallback.
  The reader's existing settings area links to the same destination.

Neither settings section depends on a native account, a successful catalogue
scope check, or the preferences form's readiness. Account/client routes and
native account authorization retain their existing guards. Failed-scope records
can still be inspected and exported. There is no IndexedDB/schema/key migration.

Browsing has no library/identity management panels. One renderless root lifecycle
restores library state and owns save-error toasts across app and standalone reader
routes. Title signed conversation lives in the Comments tab; chapter signed
conversation lives in the existing reader discussion area. **Signed comments**
reveals optional controls, with its target bound to the current title/chapter.
Native account comments remain separate and unchanged when their thread exists.
Source-only readers guard native votes/thread links instead of inventing a thread.

Settings uses native Button/Input/Textarea primitives, ordinary section headings,
and existing spacing. Backups and mode selection are directly available. Scope
identifiers, recipient/signing plumbing, source proofs, canonical records, and
retry diagnostics require purposeful disclosure. All import/merge/replace,
persistence, custom-list/member, identity-adoption, admission, snapshot review,
exact retry/cancel/remove, canonical backup, recovery, metadata, comment/reply,
reaction, withdrawal, pagination, consent, and error controls remain available.

## Dismissal and durable intent

Management can now unmount independently. Pending UI reviews/results are invalid
once it closes. The optional publication lifetime assertion prevents starting a
new signature after dismissal. If an external signer has already produced a
snapshot, the original identity/library/recipient CAS checks still run and the
exact immutable job is saved before the UI lifetime is checked again. Delivery
then stops. Fresh HTTP proof signing and the final asynchronous cancellation read
also check lifetime before starting transport. Existing non-UI protocol callers
retain the original behavior through a default no-op optional assertion.

No in-flight HTTP request can be recalled. Durable exact jobs/actions remain
available in Settings for reconciliation, cancellation, or explicit retry.

## Validation boundary

- 77 focused source/runtime tests passed, including the reviewed browser-journey
  navigation/visual-evidence seams, real pinned Svelte client lifecycle/context
  tests, and actual publication/queue/delivery/IndexedDB modules.
- The signing/storage tests use real Noble signatures and original immutable
  outbox/CAS code. Fake IndexedDB, Web Locks/worker/service-proof/network adapters
  are explicit. They prove no POST starts after the tested dismissal boundaries;
  they do not claim browser/extension/server acceptance.
- All four saved-device families, namespace/revision, canonical records, and exact
  signed jobs reopen under unchanged database versions 1 and 5.
- 132 broader anchored semantic cases passed. Strict production-composed checking
  passed with 0 errors and 15 warnings; all 679 tracked frontend files remained
  byte-identical (676 supplied files plus 3 reviewed announcement additions).
- The actual production frontend build and owned cleanup are recorded separately
  in `test-evidence/frontend/library-identity-relocation/` and the current artifact
  manifest. No dependency installation, native backend change, or preview mutation
  was needed for this relocation.

Genuine isolated desktop/mobile screenshots and browser acceptance are a separate
pending gate. JSDOM provides no layout, image, or browser evidence. The compact-feed
native patch at base `19402fc` and its historical evidence are unchanged.

## Integration provenance

The separate reviewed harness commits `6270afb`, `ee76413`, and `b4cba5c` were
cherry-picked, without source overlap, as local `0e911eb`, `5bcaae8`, and `9886cab`
before the product commit. Their original cryptographic, consent, stale-result,
visibility, pagination, and compact 505/506 assertions are retained.
