# Captured native-account intent for chapter, upload and report commands

This additive contract extends the frozen title/editorial helper from `e47b33a`. A genuine selected signing key can be rebound from native account A to independently eligible B without changing its signer generation. The preserved baseline at `test-evidence/public-api/account-intent-baseline-20261017/` demonstrates six successful writes attributed to B and a seventh discovery/cancellation sequence, all captured while the UI displayed A. Its acceptance invariant deliberately fails seven observations. Admission and current native authority were valid; the missing precondition was the intended account.

## Exact wire extension

- Chapter PUT, upload begin, upload edit-begin and generic report POST accept `expectedAccountId` in their existing signed JSON body.
- Empty-body chapter claim/removal and current upload discovery accept only one `expectedAccountId` URL query field. The proof covers that complete original URL; the body stays empty. Unknown, duplicate or malformed queries are refused.
- The account UUID is resolved as a native User in the same transaction that locks the current binding/account. A different otherwise eligible actor returns409 before the catalogue, session, report or audit mutation. A wrong resource kind returns404. Explicit null and malformed body values are refused; missing remains compatible with existing API consumers.
- Chapter removal rechecks the same captured account in its separate original-source guard after durable local review. A failed fresh guard cannot submit an original deletion request. The already durable local review keeps its actual receipt and reports the source outcome truthfully.
- Native cookie routes, native permissions, target visibility, chapter/session versions, original source authority and persisted upload-session owner checks are unchanged.

The dedicated first-party upload/chapter adapter always injects its actual captured account UUID. Its caller cannot substitute another ID. Fresh discovery is account-bound before any returned session can be adopted. Existing session mutations retain their independent owner/CAS/fence checks; they are not broadly rewritten. Generic report creation has the optional API extension; this change does not claim a newly reachable report UI.

## Qualification boundary

The finite adapter suite preserves61 cases and adds3 captured-account seams. Its fake transport verifies generated JSON, exact empty-body query URLs, refusal propagation, no replay and no foreign-session adoption. This is not real HTTP or browser evidence.

The actual chapter lifecycle now adds captured-proof eligible-rebind refusals for all six persistent commands, exact native state/audit rollback, matching-account success, account-bound discovery, complete-URL proof binding and unchanged foreign-session ownership fences. Runtime and compile receipts will be recorded separately once the source-pinned warm build is qualified. No integrated-source or browser acceptance is asserted here.

### Frozen candidate31396b4, 20:16 UTC

Both actual composed strict modes pass (enabled0errors/12warnings, default0/15), with all679 canonical frontend files unchanged. The dedicated adapter64cases and combined upload220finite cases pass. The initial new discovery assertion incorrectly expected Error stringification; its actual VisibleError body is now checked exactly, preserving all no-adoption/no-dispatch assertions. Raw finite and strict receipts are retained in `test-evidence/public-api/account-intent-candidate-20261017/`.

Backend compile and corrected real HTTP execution are explicitly held and unexecuted. The serialized relation generation still owns the warm target, and shared free4,472,119,296bytes is below the guarded4GiB+256MiB start reserve. No further cleanup is authorized. The real pre-fix failing acceptance invariant remains available; finite passes do not replace the required corrected native actor/rollback observations.

## Separate captured chapter-removal effect successor

Source review independently confirmed a second seam: an ordinary confirmation promises visible moderator review, but the former server inferred immediate hide from newly gained grants on the same native account. The captured account UUID alone cannot distinguish those effects. This seam is source-confirmed; its new real grant-gain/loss fixture is staged, not executed.

Chapter DELETE now additionally accepts one optional signed URL `removalMode=request-review|approve-hide`. Explicit review always creates/reuses durable review and never upgrades to immediate hide after grant gain. Explicit approval freshly requires actual moderator status and `chapter.remove`; revocation or ordinary approval returns403 without falling back to a report. Existing consumers omitting the mode keep their legacy role-inferred API contract. The first-party adapter requires an explicit closed effect input and always signs it with the captured account in the exact empty-body URL. Claim and discovery retain their separate sole-account-query contracts.

The visible chapter dialog stores its choice when opened. Confirmation wording, submitted mode and accepted202/204 receipt use that stored choice; a different returned effect remains unconfirmed. Original-source review receipts and the fresh source/account guard are unchanged.

The finite upload batch passes224cases, including68adapter cases (64 preserved plus4 effect-binding cases). The actual Svelte compiler accepts the changed component with zero warnings. These are finite/syntax results only; the predecessor's full strict results do not qualify this successor. No new strict staging, Rust compile, HTTP guard or browser was launched under the disk hold.

The staged actual lifecycle covers ordinary approval403 with unchanged native state, a same-account grant-gain legacy compatibility204 versus explicit review202 that stays visible, a captured approval after grant loss403 without fallback, and fresh explicit approval204 accepting the real waiting report. It preserves separate source-status headers and native audit assertions. These observations must be obtained from the corrected executable before acceptance.

### First corrected native execution, 22:16 UTC

The source compiled offline/jobs1 in96.545s, and120 Rust units passed with2existing ignored. Both final strict configurations passed on the full removal-mode source. The first actual HTTP lifecycle passed all six captured-account refusal/rollback and matching-account write checks, then exposed a staged test expectation error: the existing foreign upload-session ownership fence intentionally masks the target with404, rather than403. Production behavior is preserved; only that expected status and the printed observation label are corrected. The failed run and clean physical guard receipt remain retained; this is not a passing lifecycle. Grant-gain/loss effect checks were not reached in that attempt.

### Corrected native qualification, 22:18 UTC

The corrected test-only source729ce958 passed its actual PostgreSQL/NIP-98/native-listener chapter lifecycle1/1 in1.63s. All six signed writes refused eligible account rebind with409 and exact native chapter/session/report/audit rollback, then succeeded as captured accountA after restoring its binding. Discovery refused409, foreign sessions stayed open, and the pre-existing owner privacy404 remained intact. Complete-URL proof mismatch and malformed/wrong-kind query refusals also passed.

Real grant changes now demonstrate the separate effect boundary: a retained omitted-mode compatibility request still uses legacy204 after grant gain, while the exact captured signed review remains202 and leaves its chapter visible. Ordinary approval and captured approval after grant loss return403 without report/hide fallback or native state changes. Fresh explicit approval returns204 and accepts the actual waiting report. Original signing/source status assertions continue through the pre-existing native original-source deletion-review journey.

Both actual strict modes on the unchanged production source passed (0errors,12/15warnings), and120 Rust units passed with2existing ignored. Six additional tests mount the real compiled Svelte component in approved pinned JSDOM: Cancel, double confirmation, captured review, explicit approval, mismatched effect/refusal with reload/no replay, and unmount ownership. These use explicit account/transport/catalogue stand-ins and are not genuine browser proof. The first DOM attempt lacked one standard JSDOM global and failed before interaction; its exact log is retained alongside the corrected6/6 result. Tiny compiled DOM generations remain retained; no additional cleanup was performed.

All1244 runtime source inputs and the corrected executable were unchanged. Guard auth-momzkdfi verified stopped/closed, zero survivors/unknown births/errors; minimum free6,190,284,800bytes. Sanitized source, compile/unit/strict/native/DOM logs, initial failure, exact physical receipts and a separate observation validator are committed under `test-evidence/public-api/account-intent-final-20261017/`. The pre-fix acceptance invariant remains failed7/7 and preserved. This is isolated-source qualification, not combined integration, a production bundle, genuine browser or load acceptance.
