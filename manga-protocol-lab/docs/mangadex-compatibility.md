# MangaDex-shaped compatibility profile

Schema51 provides a documented content-read facade, public account/list/progress interfaces, and mapped Nostr publishing/group operations. It follows the owner's approved A/A/A direction from8October2026. This is not an implementation of every MangaDex endpoint, a password-login replacement, or a claim that an unchanged third-party client has been tested. No MangaDex accounts, content catalogue or image service were accessed by the fixtures.

The official OpenAPI5.13.1 / OpenAPI3.0.3 reference at https://api.mangadex.org/docs/static/api.yaml was rechecked8October2026. SHA256:004031ccdfc4b66727dd1f85d09b5300cf60235bbab2de6f6ea4816094bd6d33. Its content formats inform this adapter; Nostr signatures and immutable publication identities remain authoritative.

## Supported workflow / client matrix

| Workflow | Reference interface | Client requirement / boundary |
|---|---|---|
| Browse/search manga; chapters; series feed | GET /compat/mangadex/manga, /chapter, /manga/{uuid}/feed | MangaDex-shaped collections; documented filters only; hosted, projected, healthy content only |
| Exact manga/chapter | GET /compat/mangadex/manga/{uuid}, /chapter/{uuid} | Entity includes original signed event and manifest pointer in nostr extension |
| Ordered page retrieval | GET /compat/mangadex/at-home/server/{chapterUUID}; returned /data/{eventID}/{filename} | Every returned page belongs to that exact locally hosted chapter |
| Covers | GET /compat/mangadex/cover, /cover/{uuid}, /covers/{mangaUUID}/{filename} | One original cover per immutable series; no transformed variants |
| Descriptive authors and categories | GET /compat/mangadex/author, /author/{uuid}, /manga/tag | Credits are not verified human identities; categories retain exact free-form text and case |
| Chapter aggregation | GET /compat/mangadex/manga/{uuid}/aggregate | Instance presentation grouping only; exact IDs remain distinct; no inherited completion |
| Known managed groups | GET /compat/mangadex/group, /group/{uuid} | Public keys are fallback names; private membership/actor audit is not exposed |
| Individual publication | POST /compat/mangadex/nostr/publications | Existing signed Nostr event body and NIP98 HTTP authorization; native original-upload workflow first |
| Group creation | POST /compat/mangadex/group | Operator NIP98; body administrator public key; no private key returned |
| Group membership | GET/POST /compat/mangadex/group/{uuid}/members | Existing member public-key/role body; administrator role required; UUID addresses group only |
| Group original upload / publication / deletion request | POST /compat/mangadex/group/{uuid}/upload/{sha256}, /publish, /deletion-requests | Existing NIP98 group workflow and role/epoch/audit fences; publication references exact manifest; deletion remains locally moderated |
| Individual deletion request | POST /compat/mangadex/nostr/deletion-requests | Existing signed author request; never automatic protected deletion |
| Public accounts/profiles | GET /compat/mangadex/user/{key}, /user/me | Existing NIP98 identity; public kind0 metadata; UUID or exact public key; no invented roles |
| Public lists | GET /compat/mangadex/user/{key}/list, /user/list, /list/{uuid}, /list/{uuid}/members | Author-scoped list UUIDs derived from the current signed graph; all lists public |
| Reading status and completed chapters | GET /compat/mangadex/manga/status, /manga/{uuid}/status, /manga/{uuid}/read, /manga/read | NIP98 selects own library; exact IDs only; completion results limited to locally hosted matching chapters |
| Paginated public library | GET /libraries/{key}/entries, /compat/mangadex/user/{key}/library | Family-specific records; head-pinned pagination; no inferred replacement links |
| Signed reader writes | POST /compat/mangadex/nostr/account-metadata, /account-deletions, /library/batches, /libraries | Native signed payload and existing authorization rules; no unsigned progress mutations |
| Managed group profile | POST /compat/mangadex/group/{uuid}/profile | Administrator NIP98, metadata object and optional expected_head; group signs kind0; private audit |
| Existing account/library migration | Reviewed supplied-export plan/apply helper | Exact original and publication-ID mappings; separate explicit consent to publish history; private legacy lists excluded |
| Legacy catalogue UUIDs | Operator /admin/compatibility/aliases; migration aliases command | Explicit local aliases, never inferred ownership or rewritten signatures |

The reference Rust workflow and actual HTTP fixtures are tested. No named external reader/editor application is certified. Public instances permit anonymous reads; restricted instances require adapted NIP98 read authorization. Do not reuse MangaDex passwords or access tokens on this service. Website/browser/mobile UI work remains outside this backend phase.

## IDs and immutability

Default UUIDs are deterministic version8 identifiers derived from a namespaced hash of resource kind and exact target. A UUID is a local API alias, not a protocol identity. Explicit reviewed legacy UUIDs can become preferred in collection results. Older aliases remain resolvable; an existing alias cannot silently be moved to another target. Explicit legacy alias registration is operator-only, idempotent for the same mapping and rejects conflicting targets. Derived user/list aliases are created by the adapter from existing public identities and signed library records.

Series/chapter identity remains the signed event ID. PUT in-place edits are refused with501. Corrections use a new immutable publication and a separate deletion request. Series deletion does not delete referencing chapters or shared original bytes. Cover URLs verify the referenced series and expected filename; another retained use of the same bytes does not revive a deleted series's cover alias.

Author records represent credits on a particular immutable series; identical names are not merged into a person. Public group fallback labels are the key itself, not invented source group names or asserted MangaDex affiliation. Original uploader signatures do not identify the acting group member.

## Read contract and limits

Collections include result/response/data/limit/offset/total. Detail responses include result/response/data. Live offsets may repeat/skip after concurrent changes. Supported orderings include created/updated timestamps, manga title/year/latest chapter, and chapter/volume labels. Numeric-looking chapter/volume labels get a bounded numeric sort component without changing stored text or identity.

The query adapter supports its explicit allowlist: IDs, title, year, status, language and availability filters, content rating/demographic, included/excluded category IDs and modes, credit IDs, manga/uploader/group selection and timestamp bounds where applicable. Invalid, duplicate scalar or unsupported parameters are rejected rather than ignored. Native non-language exact comparisons remain case-sensitive. Missing optional metadata is not invented, especially content classification, dates or authorship. A client that mandates such fields may require an adapter or better source metadata.

Manga/series expansion, cover, author/artist, and public-key fallback user/known-group relationships are supported. No private group roles are added. Unavailable related records are not fabricated. Raw unknown optional manifest fields remain preserved in the hash-addressed original, even when absent from facade attributes.

Query text is limited to32KiB; collection limits are0..100. Response/selected-page metadata budgets are2MiB, with SQL-side preflight before loading a large selected page. Tag lists cap at1000. Aggregation caps at5000 publications and256-character labels; use a paginated feed beyond that. Statement/lock deadlines and query slots bound work. These limits do not certify deployment throughput.

The at-home hash is the chapter event ID; filenames use original SHA256 plus verified format extension. dataSaver and data-saver paths deliberately return the same original bytes, disclosed by originalsOnly; there is no compression implementation. forcePort443=true fails when the configured service does not use443. Image responses remain under the instance read policy, have no-store behavior, and hold a response lease until consumed, disconnected or timed out.

## Reviewed legacy alias tool

After reviewing a mapping file containing series and chapters dictionaries from source UUID to exact imported Nostr event ID, an operator can run:

```sh
PYTHONPATH=vendor/pydeps:. python3 -m src.postgres_migration aliases mapping.json \
 --origin https://instance.example --service-key EXPECTED_SERVICE_PUBLIC_KEY \
 --key-file /private/operator.key --accept-review
```

The helper validates all input mappings before sending, proves the expected service key and verifies signed response receipts. It changes no publication, original, signature or reading history. Up to1000 mappings per family are accepted per invocation. Individual registrations are committed independently: if interrupted, retry the same reviewed mapping. Conflicts fail explicitly. The operation requires the instance operator, which may differ from the original importing publisher.

## Evidence and honest exclusions

Focused fixtures: compatibility-read-guarded.log, compatibility-group-http.log; generated http-mangadex-compat.json; Python alias tests and real importer fixture. The final schema48 regression passes62 result groups, including86 library unit tests with one optional stress test skipped, Clippy and independent-process/PostgreSQL restart checks.115 Python tests pass. See compatibility-full-final.log and compatibility-python.log. The initial broader run caught a changed corruption response status; the correction keeps verified unreadable originals at404 while operational database failures remain503, and preserves response-held byte budgets.

Unsupported full-service MangaDex features include its OAuth/password/account system, unsigned account/list/progress mutations, captcha/reporting/moderation APIs, recommendations/ranking/statistics, upload-session protocol and unrestricted in-place edits. Use the tested native Nostr workflow for corresponding agreed backend features. This profile advertises full_drop_in:false; it must not be described as universal unchanged-client compatibility or production/100TB capacity certification.


## Public reader interfaces (schema50)

The existing signed kind11999 root and content-addressed CBOR graph are the only authoritative library. Convenience JSON exposes provenance but is not itself a standalone proof of each record. Native signed metadata, deletion, chunk batch and root-upload handlers are reused by the mapped write routes, including exact URI/body authentication and replay checks.

Use `family=chapters|series|lists|members`, `limit=0..100`, `offset`, and optional `head=<event-id>` for library entries. Order is bucket then record key. Pin `head` after the first page: replacement or deletion returns409 rather than mixing snapshots. An unpinned absent library produces empty list/status/read-marker collections; raw library-head and entry endpoints return404. Page scans cap at16MiB; exact lookup/filter scans cap at64MiB, and oversized views fail explicitly. Detail list relationships cap at5000; use paginated member records beyond that. Read-marker lookup accepts at most100 manga IDs and5000 locally hosted matching chapters. Status maps cap at10000 series. Response budget remains2MiB. These are bounded convenience interfaces, not a substitute for raw graph sync for arbitrarily large complete exports.

List IDs are scoped to both publisher and native list ID. Two keys may reuse the same native list ID without being merged. List rename preserves its local UUID; removing it makes its old alias unreadable. Shared content-addressed chunks do not share account ownership or deletion rights. List positions and exact series IDs are retained. Unhosted series may appear as exact references without fabricated metadata.

`/user/me`, `/user/list` and own status/read-marker routes require NIP98 even on public instances. Public-key-addressed library views obey the instance's public/authenticated/admitted read policy. Account enrollment policy is still being decided; these interfaces do not grant publishing or account-write admission.

## Managed group public profiles (schema51)

An administrator may publish name/about/website and other bounded kind0 metadata using the group's existing protected signing key. A profile operation is limited to a JSON object of16KiB serialized content. The service rechecks membership, role, group state and version in the same transaction as the new public head, relay outbox and private actor audit. Publishers cannot edit group profiles. Revoked administrators and disabled groups are denied. An audit failure rolls back the operation.

`expected_head` provides optional compare-and-set protection; conflicts return409. Timestamps advance monotonically and are capped at60seconds ahead; exceeding that returns429. No group private key or acting member identity appears in the public profile. Empty metadata clears descriptive fields and restores the public-key fallback label, but does not delete the group, its membership, or publications. Group-profile/record deletion policy is a separate open design choice.

## New feature verification

Reader workflow tests: `results/reader-state-focused-2.log` and `results/http-reader-state.json`. Group profile tests: `results/group-profile-focused-2.log` and `results/http-group-profile.json`. These focused tests cover authorization, replay, snapshot replacement/deletion, shared-object isolation, private actor audit and fail-closed rollback. The final schema51 regression passes66 result groups,88 library units (one optional stress skip),115 Python tests,Clippy and process/database restart checks. Optimized build,enhanced demo and extracted-source library/binary tests pass. The schema48 figures above describe historical baseline evidence only.
