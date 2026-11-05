# Captured local verification mail in the browser

The disposable browser fixture now provides an ordinary HTML mailbox at:

`<mail_origin>/mail/inbox?to=<exact-synthetic-address>&kind=verification`

Use the actual `mail_origin` returned by the owned fixture. Only one exact reserved
`@example.invalid` recipient is accepted. The server binds IPv4 loopback and the
page also checks its exact Host. Recovery links are deliberately excluded.

1. Register a fresh account through the real site UI.
2. Request its verification message through the real verification UI.
3. Open the mailbox page in the same browser context. An empty mailbox is not a
   failure until a message has actually been requested and accepted.
4. Click **Verify this test email**. This is the actual delivered link, not a
   token synthesized from a database or a fixture verification mutation.
5. Refresh the site session and verify the account's actual verified state.
6. Reopen the mailbox and click the same link once more; the ordinary application
   must reject its reuse as already used, invalid or expired.

Opening the mailbox alone never redeems the link. It reads the existing in-memory
mail sink only; there are no database reads/writes, new messages or external
requests. Tokens are never printed or persisted in fixture receipts. Do not save
mailbox HTML, screenshots containing link destinations, raw URLs, or browser
network records in test evidence. Record only nonsecret UI outcomes and message
counts. The page is no-store/no-referrer, has restrictive CSP and no scripts,
frames, forms or third-party resources. The exact link and display text are
validated and escaped before rendering.

A pass establishes real application issuance, accepted delivery to a local test
sink, browser redemption and one-time handling. It does **not** establish SMTP or
external-provider deliverability. This workflow is distinct from the explicitly
simulated `/fixture/reader-verification` helper; do not invoke that helper for this
acceptance case.

Current helper validation:

`python -m unittest discover -s tests/ci -p test_verification_mailbox_fixture.py -v`

The tests exercise the actual loopback HTTP handler with synthetic messages and
check rendering, capture, no redemption/logging, recipient/query/Host guards,
shutdown/maintenance rejection, invalid captured links and response headers. They
do not replace the final browser/account flow or real backend one-time-token test.
