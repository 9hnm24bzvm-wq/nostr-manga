# Reconstructed announcements

The supplied archive omitted both homepage announcement imports and the linked `/announcements` route. Their original markup, styling and behavior cannot be recovered from this attachment. The external integration is an explicitly reconstructed feature, not an exact reproduction of those missing files.

Existing supplied HTML/CSS is untouched. New components reuse the supplied Card, Button, Input, Textarea, Checkbox, Label and sanitized Markdown utility. Routes are materialized additively in the isolated build source; no original attachment file is overwritten.

Implemented source paths: public paginated `/announcements`, public `/announcements/[id]`, pinned published homepage items, and `/announcements/manage` for drafts, creation, editing, publication, pinning and versioned deletion. The client uses same-origin `/api/announcement`, a 30-second request deadline, truthful failures and current optimistic version numbers. It creates no fixture or fabricated announcement rows.

The Rust contract is a feed `{total,limit,page,data}` and announcements `{id,title,body,pinned,published,version,created_at,updated_at,published_at,relationships:{author}}`. Staff/sysop plus `announcement.create` or `announcement.update` may use `include_drafts=1` or unpublished detail. Each mutation requires its explicit create/update/delete permission; backend authorization remains authoritative. Creation defaults to an unpublished, unpinned draft. DELETE carries `{version}` and succeeds only on 204.

Build and later end-to-end workflow evidence are recorded separately. No rendered parity or announcement workflow test pass is asserted by this source implementation.
