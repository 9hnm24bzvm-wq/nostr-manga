# Reconstructed announcements

The supplied frontend archive contained sidebar/home references to announcements
but omitted that feature's source subtree. These are additive reconstructed
semantics, not an assertion that an unknown original API was restored exactly.

GET /api/announcement returns the usual total/limit/page/data feed. Filters are
pinned=true|false, name, order=asc|desc, and private include_drafts=1. Default limit
is20, maximum100. GET /api/announcement/{id} reads a single published item.

Announcement fields are id/title/body/pinned/published/version/created_at/
updated_at/published_at and relationships.author. Body is plain Markdown/text
JSON; the backend never renders caller-provided HTML. No announcements are
seeded or fabricated. New drafts default to unpublished and unpinned.

Staff/sysop eligibility is necessary but insufficient for mutation. The
persisted explicit permissions announcement.create/update/delete are checked
for POST /api/announcement, PUT /api/announcement/{id}, and DELETE respectively.
PUT requires the current version and optional title/body/pinned/published.
DELETE takes JSON {version}, performs recoverable soft deletion, and returns204.
Create returns201 and update200 with the persisted DTO. Draft read requires
staff/sysop plus create or update permission. No signup flow grants these
permissions and a moderator role alone cannot publish an announcement.

Every mutation appends an audit row with actor, old/new version and publication/
pin state. Publishing and unpublishing update published_at transactionally.
Existing authentication, CSRF, body bounds, pool and no-store guards apply.
