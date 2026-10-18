# Native account reading-status UI successor

Source: `76e443bc13863a6e99c76fb892bed10978f8700b`.
Backend contract: `96241c03b60b2103f667e8471f300c65513e6a4f`;
its actual Rust/HTTP qualification remains independently pending.
This separate successor follows the frozen device-list rename. Earlier composed
strict/Vite evidence does not qualify these changed frontend bytes.

The existing Manga account-follow dialog now shows the actual six-value reading
status after an active native-account follow has been observed. Save uses the
separate signed native status preview, canonical account/title captures and the
independent status-change revision. Original signed-library status reads are not
used as private account state. Device saves remain the primary existing action.
No null clear or automatic follow is offered.

The dialog retains its captured account across Reload, including same-signer
account rebinding. Close and reopen deliberately starts a new capture. The client
checks exact target/account/provenance, UTC6 persisted timestamps, tombstone
versions and confirmed changed/version intent. Unknown or malformed receipts
require read-only Reload before another mutation. A known successful status save
stays visibly successful if its later membership refresh fails; further controls
remain fenced. Shared pending guards prevent membership/status interleavings,
and close/target/signer-generation checks reject stale continuations.

## Retained lightweight evidence

`test-evidence/frontend/native-account-reading-status-20261018/` contains the
exact source pins, raw final log, result and rerun recipe.

- 60/60 affected cases pass: 19 new production-client cases, 16 new compiled
  Svelte/JSDOM cases and 25 existing follow-client/component cases.
- Six actual options, exact captured POST payload, absence/follow prerequisite,
  stable no-op, native/import same-clock changes, remove/readd, account rebind,
  stale CAS and malformed/unconfirmed receipts are exercised.
- Actual compiled UI exercises both pending-operation fences, removed status
  clearing, known save versus failed refresh, explicit Reload without replay,
  and delayed close/signer/target invalidation during preview and write.
- Svelte5.55.1 and JSDOM27.0.1 come from existing verified local dependency roots;
  no dependency installation or cache copy was performed.
- Independent read-only review of both production hashes found no concrete
  blocker. The reviewer did not execute these tests or claim full-app acceptance.

The actual production clients and AccountFollow component execute. Bound account
access, HTTP storage and leaf UI primitives are explicit finite stand-ins. This
is not real backend HTTP evidence, the whole mounted application, a native
browser, persistence/crash acceptance or a production/parity claim. Fresh final
composed strict and Vite association is still required. Canonical frontend files
were not edited.
