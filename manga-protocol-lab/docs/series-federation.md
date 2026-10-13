# Series publication federation

Schema27 extends the approved-peer publication transfer path to immutable series events (kind1002), in addition to chapters (1001). Series manifests are canonical CBOR and validated before admission. The receiver retains its own verified manifest; it does not depend on the source continuing to host it. A series-only transfer has no chapter originals to fetch. Existing moderator approval, publisher identity, peer endpoint/epoch and generation-safe retention rules still apply.

POST /admin/sync can receive either exact signed publication type. Its filter is evaluated before local staging. For series, publisher and created_at are present; chapter-only series/language/chapter/pages/size fields are SQL NULL, consistently in transfer and inventory evaluation. NULL is not silently changed to an empty string or zero.

POST /admin/sync-jobs adds inventory:"series"; omitted inventory remains "chapters". Filters, approved fallback peers, bounded ID-prefix partitioning, durable missing-item queues, retry budgets, cancellation and late job-generation fences use the existing worker. Reconciliation sessions are bound to inventory as well as peer/epoch, filter and prefix. A session cannot be reused across chapters and series. The transfer worker checks that a returned signed event has the requested inventory kind.

Example request body:

```json
{"peer":"<approved-service-key>","inventory":"series","filter":"created_at>=1700000000","fallbacks":[]}
```

Do not infer series deletion from a chapter deletion, or vice versa. Series inventory is explicit; a chapter's series reference does not automatically import or rewrite another publication. Associations and replacement linking remain instance decisions.

Existing stored chapter jobs without an inventory field deserialize as chapters. Active-job deduplication also normalizes that legacy default instead of accidentally creating a second equivalent batch after upgrade.

## Evidence

series-federation-regression.log and series-federation.json cover a real two-node PostgreSQL/strfry fixture: signed series publication, rejection by a chapter-only language filter, publisher/timestamp inventory filtering, durable series reconciliation, projection and a local retained series manifest. The same fixture still runs worker process-crash recovery and the120000-common-ID partition test.

series-inventory-full-regression.log passed integrated schema27 validation, including cross-inventory session rejection and legacy queued-job deduplication, Clippy and PostgreSQL immediate-restart checks. General automatic profile/social/library inventories remain a separate feature gap at this checkpoint. Performance and100TB capacity are not established by these fixtures.
