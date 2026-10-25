# Public language syntax and retained native labels

## Pinned contract

The inspected MangaDex 5.13.1 OpenAPI document is pinned to upstream revision
`60a3bb3c9f25675435a479c925d857015201739e`, SHA-256
`004031ccdfc4b66727dd1f85d09b5300cf60235bbab2de6f6ea4816094bd6d33`.
It specifies syntax, not an enumeration of recognized languages.

- Public manga `originalLanguage`, chapter/upload draft `translatedLanguage`,
  cover `locale`, group `focusedLanguages`, and cover `locales[]` use the exact
  full-string pattern `^[a-z]{2}(-[a-z]{2})?$`. Thus `ta` and syntactically valid
  `zz` are admitted; `tam`, `und`, uppercase, whitespace and `zh-hant` are not
  public write values. This is not a claim that every syntactically valid label
  identifies an ISO language or has a translated display name.
- Catalogue original/translated/available-language filters, feeds and aggregate
  use the distinct `^[a-zA-Z\-]{2,5}$` pattern. Values such as `tam`, `EN-us` and
  `--` satisfy that syntax. Matching remains exact and case-sensitive; admission
  does not promise a matching row. Do not tighten search to the write pattern.
- Group `focusedLanguage` search is a bounded opaque string in the pinned schema,
  not a language enum or either pattern. Existing query length/control bounds
  continue to apply. Cover locale search is explicitly the stricter exception.
- `LocalizedString.additionalProperties.pattern` is `^[a-z]{2,8}$`, which
  constrains **values**, not keys. Applying it literally would reject ordinary
  titles, descriptions and biography text, including the schema's own examples.
  This is a documented compatibility interpretation, not literal-schema parity.
  This implementation intentionally keeps bounded opaque map keys and ordinary
  bounded text; it does not reinterpret the anomalous schema as a locale enum.
  Relevant source positions include lines 182–201, 2440–2445, 5169–5171,
  5221–5226, 5836–5841, 6593–6595 and 7030–7032.

## Native/public boundary

The retained native routes still use their original language list: native `tam`
remains accepted and native `ta` remains rejected. Protocol import continues its
existing bounded BCP47-like projection and lowercase normalization. Stored
projected `und`, `tam` and `zh-hant` remain visible in public output as intentional
native/protocol extensions, not silently mapped or hidden to fit the write
schema. Mixed-case original metadata and signed events remain exact; their
existing derived lowercase projection is unchanged. No source event, manifest
or image byte is rewritten by these validators.

The sealed mutation actor selects `Native` by default and `Public` only for the
verified, transaction-rechecked public bound-account capability. Selection is not
accepted from request JSON, UUIDs, cookies or query parameters. Shared manga
metadata, chapter metadata and page replacement, cover updates, and upload
resume/commit validate using that selection. Publication records without an
actor receive an explicit internal contract, including verified original
publication insertion. Native publication entry points keep native defaults.
Group public constructors independently check strict public language values.
Restart reconciliation is a different, actor-independent boundary: `import_page`
verifies immutable signed source records and `materialize_protocol` applies the
unchanged protocol language projection before direct derived insertion and upload
receipt reconciliation. It does not reconstruct a Native mutation actor or
revalidate source labels against the native write enum. Stored command payloads,
hashes, manifests and authority bindings are unchanged. Replaying a public commit
still requires the same public signed request/account checks; a native replay
with `ta` is intentionally not an equivalent public-authority operation.
The normal signature, account/service binding, owner permissions, row locks,
version/CAS, immutable-source and prepared-command fences are unchanged.

## Editing and upload recovery

Public-only editors accept code entry rather than forcing the native `Locale`
list. Updates validate only fields emitted in their patches: editing title,
chapter or cover metadata with an unchanged unsupported imported language omits
that language, preserving the stored label. Group focused-language edits follow
the same changed-field rule. Localized dictionary keys remain opaque.

Upload staging retains its existing full-snapshot replacement semantics. An
omitted or empty language in an incomplete private draft remains an unset value;
any supplied nonempty public language must satisfy the strict pattern. A public
upload or page-replacement commit requires an explicit valid language. The client
must not silently drop an unsupported saved language, claim it was recovered, or
map it into another label. Existing native snapshots remain readable, but an
unsupported language must be deliberately changed before a public full-snapshot
save or upload commit. Ordinary chapter metadata edits can instead omit it.

## Verification scope

Source-bound unit checks cover exact public/native syntax separation, opaque
localized keys, omitted language updates and unchanged native import projection.
`tests/support/public_language_contract.rs`, included by the existing
`public_manga_writes` target, specifies real signed HTTP/PostgreSQL durable ta
writes, native tam/ta behavior, search distinctions, account/service/owner/CAS
refusals, resume/page replacement, actual original upload insertion, and genuine
imported-label/source preservation. Authored tests are not execution evidence.
Final receipts distinguish executed checks, intentional old-source failures and
unexecuted browser coverage. No browser, hosted/production action or deployment
is part of this lane.

### Executed qualification (2026-10-22)

The source-frozen candidate `3c9fff826e8beb4b18e97c10712b0368676cb63f`
passed the real signed HTTP/PostgreSQL language lifecycle, the neighboring full
signed title/cover/editorial lifecycle, and graceful committed-pending recovery
through a genuinely fresh process. The recovery check retains the same host and
synthetic database; it is not evidence for abrupt-crash, independent-host or
uncommitted/open-session recovery. The frozen executable is run directly rather
than rebuilt by the HTTP harness. Every selected lifecycle has verified inner
PostgreSQL and outer birth-tracked process cleanup, without forced termination.

The identical production tree at `35df25e` passed 157 library tests (two existing
ignored tests) and the imported native-label/exact-original-source HTTP case.
That import case was not rerun at the later test-only source freeze. The public
frontend finite suite passed 458 checks, and all 679 supplied original frontend
files retain their baseline bytes. The same 61 frontend behavioral probes pass
on the candidate; 31 fail on the old source. In an actual old-source HTTP control,
the first public `ta` title create returns `400 Unsupported language code`,
whereas the candidate persists it successfully. Neither a missing-module error
nor an incomplete baseline-export compile counts as a behavioral control.

The [sanitized receipt](verification/public-language-contract-20261022.json)
records exact source/spec/executable hashes, selected commands, bounded owner
limits, cleanup summaries, earlier fixture mistakes, reserve-guard refusals,
and explicit unverified scope. The reusable existing-API frontend probes are
[preserved separately](verification/public-language-existing-api-probes-20261022.mjs).
Full private runtime evidence and synthetic database archives are not committed
and must be excluded from any published recovery checkpoint. No browser launch
was retried; no hosted or production action was performed.
