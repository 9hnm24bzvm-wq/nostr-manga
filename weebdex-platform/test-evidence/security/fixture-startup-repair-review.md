# Independent fixture startup repair review

Reviewed 2026-10-13 12:12–12:18 UTC. **Finite verdict: PASS for all three exact source repairs below. The single new malformed-media error-code finding was corrected and independently rechecked. No remaining scoped source blocker found.** This verdict retains the existing parent runtime gates and does not claim successful seed, HTTP, browser, or cleanup execution by this reviewer.

Only source/evidence reads, SHA256 hashing, in-memory source-text reversals, non-executing Python AST parsing, and this report were performed. No Cargo, fixture, target application, PostgreSQL, Node, Chromium, test binary, or network service was run. No implementation source was edited. The text reversals and AST parsing did not import or execute the supervisor.

## Exact inspected boundary

- Provisioner `backend/examples/browser_fixture_provisioner.rs`: `dcfe71335710e5ac5f9e201a06c533005417823508cf5b676fff002c90d3d559`
- Final supervisor `tests/runtime/browser_fixture.py`: `00c5edf0fb1f0afc2808a078d79b2a2bc688690a60652c69d1c746f6771c42d5`
- Prior reviewed auth-only supervisor: `c7af1cc05ffb88d19a8741819daa3800d9430ba7f308e0b76f38536fbd071627`
- Auth route/handler source `backend/src/domains/auth/flows.rs`: `c37227be7e2fe7619987f6af0c67081daf06f2a9bb3ec81d385566cb5b681f23`
- Router source `backend/src/router.rs`: `544a69ff1466f7a6231b062c6631a17f35b71a48ef5409aa8bea45489634087b`
- Actual dependency `../manga-protocol-lab/server/src/upload.rs`: `390c2304dfa1c490ab5802b8216eb9f6173011d3b18e203d7c3f4df4f5abd87b`
- Native media handler/filename validation `backend/src/domains/uploads/media.rs`: `56aa7ba41275411a871b8ca60c7928adad5e27161b7933b33d0f39479f5bd015`
- Native storage ID validation `backend/src/domains/uploads/storage.rs`: `0310aa66e42a376ddf7a45c2484d5ab949aee5d0fabf642a5677df28136cd0c9`
- Protocol media delegation `backend/src/protocol/media.rs`: `8af6719b3ddd38a8ebf9883409f2c7040f036e5de1b363866e4664010d6d6f94`
- API error serialization `backend/src/error.rs`: `23f52787c1661f59f1e1e241c1cb1d6a11282c02e15a3bc92ec3b56c82110baa`

Both repair owners confirmed their final implementation sources frozen at the hashes above. The actual original UploadStore source hashes identically to `test-evidence/original-regression-008/server/src/upload.rs`.

## Volume grammar and bounded diagnostics

The unchanged original `UploadStore::open` requires a nonempty volume of at most 64 bytes containing only ASCII alphanumeric bytes or hyphens (`upload.rs:144–151`). The former underscore-containing fixture names violated that contract. The new `SOURCE_VOLUME="browser-fixture"` and `RECIPIENT_VOLUME="browser-recipient-fixture"` satisfy it. Setup uses the first constant for the shared source/runtime storage config (`provisioner.rs:320–323`); recipient setup uses the distinct second constant (`:753`). The repair adapts disposable fixture configuration to the original policy without relaxing that policy or joining the two volume identities.

`SeedStage::label` and `safe_failure` (`:30–89`) return only explicit static string literals. Error-chain text is inspected internally only to classify the fixed invalid-volume reason. Raw error chains, request/config data, filesystem paths, credential contents, signed bodies, and arbitrary I/O error messages are never interpolated into the emitted failure diagnostic. The failure branch (`:985–994`) emits those fixed labels and exits 1 without asserting success. Stage context leaves the original errors intact internally and does not alter publication/admission semantics.

The added grammar and diagnostic tests (`:1002–1039`) cover both constants and representative nested failures. They were inspected but not executed by this reviewer. Existing compiler PASS reported by the owner is separate evidence, not a test execution or source-to-binary reproduction by this reviewer.

## Correct method-specific auth expectations

The exact dynamic route is registered with `post(update)` (`flows.rs:39`). A GET to `/auth/self-service/nonexistent` therefore reaches method rejection rather than the update handler's kind validation. Supervisor `:957` now requires 405, an empty body, and exact `Cache-Control: no-store`, even with `Accept: text/html`. This preserves a negative no-SPA assertion.

The separately supported GET route `/self-service/{kind}/browser` is registered at `flows.rs:29`; `create` calls `validate_kind` at`:388`, and the unknown kind returns 404 at`:188`. Supervisor`:958` requires that separate unknown browser-flow request to return 404, JSON content type, a JSON object, and exact no-store. Existing outer response headers cover auth method errors (`router.rs:475–485`), while the auth nesting/fallback excludes an HTML-success substitution (`:62–76`).

Unknown POST behavior remains intact: `update` still calls `validate_kind` before mutation-origin validation, flow lookup, or body parsing (`flows.rs:610–622`), so a normally admitted bounded request for an unknown kind is rejected 404. This change does not add or change any backend route or handler.

The auth-only supervisor delta was independently checked against the previously reviewed hash `2f5caccde5292d226c2616a9566cb711741bfb6f1589100950700d63419bf434`: removing the four new lines and restoring the auth URI as the second item of the original 404 loop reproduces that exact hash. Thus all other supervisor bytes, including ownership, watchdog, lifecycle, deadline, and existing HTTP assertions, remain unchanged for that repair. In the final supervisor, the unchanged auth assertions are now at`:958–959` because of the additional malformed-media check.

## Missing media versus malformed media

The original absent-media test used `/data/Unknown000/unknown.png` with an invalid seven-character page ID. The unchanged native `valid_name` requires an allowed extension and `storage::valid_id` (`media.rs:253–260`), which in turn requires exactly 10 ASCII alphanumeric bytes (`storage.rs:37–39`). When the absent chapter has no original-protocol mapping, the protocol helper returns `None` (`protocol/media.rs:102–117`); the native handler then validates the filename before its not-found query (`uploads/media.rs:39–41`). Therefore the original malformed URI legitimately returns 400 rather than 404.

The final supervisor replaces only that 404-loop URI with `/data/Unknown000/Unknown001.png`: both ID components are exactly 10 ASCII alphanumeric bytes, and the allowed `.png` extension passes native filename validation. Its genuine absent-row query can now establish 404. A separate assertion retains the old malformed URI and requires 400, JSON content type, exact no-store, and the actual serialized error code `invalid_request`, even with `Accept: text/html` (`supervisor.py:953–955`). The malformed request is not silently dropped, accepted, or converted into an absent-resource request.

The first frozen follow-on candidate `a7fbce8d049ed8aeabec84d39b4bf95219594df286ccaba8e93561bcbb67033b` incorrectly required code `bad_request`. This reviewer reported that concrete startup blocker: `ApiError::bad_request` returns code `invalid_request` (`error.rs:33–35`), and `IntoResponse` serializes that code with no-store (`:78–91`). The fixture owner corrected only the expected code. The final source hash above was independently observed and its corrected assertion rechecked.

The final media delta consists of exactly two changed lines relative to the prior reviewed auth-only supervisor. Removing the added malformed 400 assertion and restoring the previous 404-loop URI reproduces `c7af1cc05ffb88d19a8741819daa3800d9430ba7f308e0b76f38536fbd071627` exactly. This independently preserves all prior auth, ownership, watchdog, lifecycle, deadline, and other operational-check bytes. The final Python source also parses successfully without import or application execution.

## Acceptance boundary

These source corrections close the three identified deterministic startup mismatches and the new assertion-code typo. Use the exact final reviewed sources and corresponding compiler artifacts for the already-authorized fresh runtime batch. Real readiness, native HTTP results, browser journeys, and terminal cleanup must be established by their actual execution receipts; this reviewer did not reproduce them. No broader application audit or unrelated policy change was performed here.
