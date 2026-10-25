# Native-account follow controls: source candidate

## Implemented

This frontend source candidate targets qualified backend `265804862294a9fa82dcaeabdd3843779be047b2`.

- The existing title action row gains a deliberate `Follow on account` action. Anonymous device-library save/status/progress behavior stays unchanged. No device mode or native cookie Session is substituted for request-bound account authority.
- Existing group and native-user action rows gain the normal account-follow dialog. Only profiles explicitly marked `native_public_user` are eligible; protocol-only publishers are not converted into native follow targets. Self-follow is refused. The old group cookie-only social adapter/readiness controls are fenced in public UUID mode instead of sending UUIDs through an unsupported native-cookie adapter.
- The exact requested page UUID must match the selected canonical target before an account lookup begins. Unproven manga/group/user alias redirection cannot silently retarget a mutation. Normal canonical collection/profile links work. Broader aliases require a separately qualified typed mapping proof.
- Every status/receipt validates exact captured account, target UUID, resource kind, membership revision semantics and source scope. POST/DELETE carry the real `expectedAccountId` and the known membership version only when available. Status 404 means absent or currently hidden, not version zero. The dialog exposes an explicit `Remove saved follow` operation in that unknown state, without inventing hidden metadata.
- A known-version receipt must report both the correct version arithmetic and a `changed` flag consistent with the captured membership and requested state. Unknown-version 404 cases may legitimately be a change or no-op. The same independently discovered confirmation gap was fixed in the earlier list-follow and rating clients, with focused regressions.
- Unconfirmed, denied, stale or malformed responses freeze mutations until an explicit reread. Closing, leaving or changing signer generation discards delayed results. No automatic replay, optimistic save, public-library publication or change to another account is introduced.

The three exact native action-row additions are enumerated public feature exceptions; the group cookie-adapter availability fence is separately enumerated. All supplied frontend files remain unchanged. This is an explicit instance-account domain, including the backend's documented user-follow extension; it does not claim a global publisher follow graph or MangaDex-hosted account parity.

## Executed evidence and outstanding check

Evidence: `test-evidence/frontend/native-account-follow-ui-20261017/`.

- The final composed finite batch passed 342/342 tests. Its 21 dedicated native-follow cases cover all three kinds, genuine private account/target/CAS payloads, native versus publisher eligibility, self-follow, rebind, unknown tombstones, hidden membership removal, no-op defaults, contradictory receipts, denials, response loss, concurrency, canonical-target guard, lifetime/generation and actual component/overlay attachments. The same batch covers earlier library, source mapping, list, rating, settings, upload and preparation regressions.
- The final follow component compiles with the real installed Svelte compiler in memory with zero warnings; no generated build output is written.
- Real production preparation for both gates passes in the final finite batch, including exact exception validation and immutable 679-file frontend inventory.
- Both full composed strict modes passed on the initial candidate: zero errors and the existing 15 warnings. Those receipts are deliberately labelled **pre-correction**. They precede the small independent-review saved-follow-removal and receipt-consistency fixes and must not be treated as qualification of the final source.
- A fresh final full strict run is pending. Shared disk free space fell below the existing 4GiB floor (observed 4,166,529,024 bytes), and new tests/builds were stopped pending the parent's existing cleanup approval. No cleanup was performed or alternate build route used to evade that hold.
- The backend owner independently reviewed settings/follow authority and identified both corrected gaps. Final source hashes are recorded for follow-up review.

No rendered browser, real account, external signer prompt, browser IndexedDB, deployed-service or production qualification is claimed. A native account-follow collection UI for titles/groups/users remains separate follow-on work; this batch's controls operate on the existing title/profile pages. Template administration, protocol-publisher graph following and unrelated native-cookie mutations remain fenced.
