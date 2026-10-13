# UI9 finite frontend patchset

Production source frozen 2026-10-13 06:58 UTC. This is source-only evidence; no browser journey or application runtime test has executed.

## Reader history and progress admission

The external library action owner captures immutable reader target, component lifetime, stable browser authority epoch, explicit library-selection epoch and any already-open library context before awaiting storage. Initial persisted-mode discovery is allowed only under unchanged admission. The currently mounted reader may admit an initially confirmed account through a stable-authority effect, with one token per actor/authority/chapter. Ordinary history/follow Session refreshes do not trigger another POST. Destroyed opening callbacks cannot write for a later account. ChapterHistory.UpdateBatch accepts an explicit captured context and assertion; local progress asserts its original destination inside the IDB transaction, including after asynchronous scope/row reads. Exit can finish only an immutable device page intent already admitted while live, never choose a new account or mode.

Acceptance still required: defer opening, navigate away, switch account/mode, release opening; no old-reader write. Verify initial-account readiness, stable Reader lifetime and one acknowledged history write. Verify final device progress remains bound to its admitted namespace.

## Personal vote projection and counts

Personal discussion reads use stable actor/authority epoch, fresh matching account checks, owned cancellation and no-store. Harmless Session data refresh no longer cancels an otherwise-current read. Actual authority changes still invalidate it. The vote component owns reactive personal reads by thread/authority and disposal. A user click reads the actual current personal vote before choosing Vote versus Unvote. Successful204 is followed by fresh public thread counts; failed count reconciliation reports acknowledged success without replay or invented increments.

Acceptance still required: hold vote GET across reader-history reconciliation; verify existing Up produces Unvote, current count is authoritative, and logout/disposal cannot attach an old private result.

## Client name contract

Both API create and rename validate the unchanged backend contract: Unicode White_Space trim,1–64 Unicode scalar values, at most256 UTF-8 bytes, no Unicode Cc controls. Lone UTF-16 surrogates are rejected. The create UI uses semantic validation and actionable400 feedback. Exactly four approved validation-only attributes/text anchors are normalized in preservation evidence. The raw input maximum128 UTF-16 units permits64 supplementary characters; semantic validation imposes the scalar limit.

Acceptance still required:1/64/65 ASCII characters,64 supplementary characters, Unicode White_Space versus FEFF, controls and whitespace-only input. No schema range expansion was made.

## Latest query ownership

A shared latest-search owner invalidates old generations immediately, cancels the previous debounce and owned fetch, and starts the latest request without mutex admission loss. Close/disposal, clear-query and callback-target changes discard old completion. Public manga/author/group search uses anonymous local reads; user search preserves fresh current-authority reading. Existing non-owned API callers retain their prior transport. Two precisely approved existing inline preference-search callbacks forward the cancellation signal; all other selector changes are script-only.

Acceptance still required: A delayed while B is typed; only B can attach, including author/group selectors. Clear and close/reopen must not reveal a late A result.

## Browser source preparation

The harness resolves only SHA-pinned, already-installed official Playwright1.62.1 at its reviewed absolute runtime location. It does not inherit NODE_PATH or install packages. Its network guard allows only the recorded UI/mail/original services, plus at most two distinct recorded protocol recipients with exact loopback HTTP origin, matching root WebSocket and64hex service key. The primary must match the fixture identity. All eight signed phases dispatch through the independently authored signed module. Node syntax checks are preparation evidence, not browser execution.

## Preservation and artifact status

All676 supplied originals remain byte-identical. Composed Svelte/esbuild source compilation passed79 components and46 scripts, with seven existing initial-value warnings recorded. The420 definitions/429 exact anchors span91 original paths. Original styles/classes/layout outside enumerated handler/guard/key/panel/validation exceptions remain strict byte comparisons. Global rendered HTML parity is not claimed. Build9 source387 is a preserved previous artifact. Guarded build10/source420 passed07:02:37UTC with unchanged inputs and no remaining owned descendants. Its current397-file artifact, exact bootstrap CSP and408-member portable package are verified; browser behavior still awaits the independent runtime gate.

Authoritative finite source inventory: `test-evidence/frontend/round9-owner-source-repairs/source-manifest.json`; summary and compile evidence are adjacent. Browser/system tests remain behind the independent implementation review gate.
