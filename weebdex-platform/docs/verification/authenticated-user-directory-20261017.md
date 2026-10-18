# Authenticated User.Search adapter and retained query-prefix compatibility

`API.User.Search` now uses the genuine `/api/v1/user` directory through fresh request-bound Nostr proof and an actual active native-account binding. Public `/user/{uuid}` profile reads remain a separate operation. The directory enumerates only public native-user fields; it is not an anonymous directory, private account details, or a synthesized protocol-publisher list.

## Supported controls and privacy

The retained `name` filter maps exactly to `username`, and `page` maps to bounded row offset. `id`/`ids[]`, username ordering, limit 0–100 and offset 0–10000 have explicit typed checks. Unknown, duplicate, ambiguous, excessive and unsupported controls fail before account/signing access. Query controls are never silently dropped. Results retain server order and actual collection totals. Canonical User resources must have only username, public roles, actual positive resource version and no relationships; unexpected private fields are rejected before any resource provenance reaches a consumer. No email, preferences, private follows, mutable profile claims or protocol-publisher identities are invented. ID-filter results must match an exact requested canonical User UUID; genuine nonpreferred aliases remain fail-closed until a qualified typed User alias proof exists.

Every directory response must match the actual captured account, instance-account scope, persisted native library source and explicit request-proof requirement from backend commit `244f87fec5f54788c025daa91493804f8da9dfcc`. Denied reads have no anonymous/native-cookie fallback. The existing no-store signed transport is reused. Cancellation and signer/service changes discard stale output.

The existing Users and UserSearch scripts no longer persist or reuse directory results through browser history in UUID mode. An anonymous view clears cached rows, explains the account requirement and makes no search/signing request. Signer changes clear the displayed result and require a new authorized read; delayed old errors cannot replace the new identity state. The native gate-off behavior and all original markup remain intact.

## Retained runtime query-prefix correction

The retained root layout overrides `URLSearchParams.toString()` to return a leading question mark. Plain-runtime tests had not covered that behavior. The new directory adapter, the earlier following collection reader and the earlier upload-sort repair now normalize this leading mark before adding one delimiter. Six new regressions execute the actual retained layout override: directory and following requests preserve their first real query parameter, while both upload sort actions work with empty/nonempty filters. Standard URLSearchParams behavior is also covered. The previous upload-sort evidence is not rendered-browser proof and did not exercise this root-layout override; this successor closes that exact coverage gap.

## Qualification

The API owner independently reviewed the exact frozen directory source hash against `account_reads.rs` and the production attachments. Authority/provenance, typed target filters, paging/order, public projection, native fallback fence, history bypass and retained query serialization matched; no concrete privacy or contract blocker was found. This was read-only source review, not another HTTP/browser run.

The final corrected source passes 393 combined finite cases, including 12 directory adapter/bridge cases, 6 actual directory component-script cases, the 6 retained-layout regressions and real production preparation in both gates. Canonical frontend inventory remains 679 unchanged files. Exact commands, source pins and raw output are under `test-evidence/frontend/user-directory-20261017/`.

One warm gate-on composed strict check passed with zero errors and 15 existing warnings before the final delimiter-only corrections. Its receipt is explicitly labelled pre-correction. A final full strict generation was not run because this bounded batch was requested to use finite checks near the shared disk floor. The final frozen combined integration must rerun its applicable aggregate checks. No dependencies, cache copies, cleanup, browser/application/database startup, real account or publication were used. Browser/IndexedDB acceptance and integration/checkpoint approvals remain separate and unchanged.
