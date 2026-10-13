# Pending endpoint discovery choices

Asked 7 October 2026 at 16:37 UTC, message Sentinel_1e03f84c105c81918b5837a0310fd2e6. No answer yet.

1. Announcement lifetime: recommend A, no automatic expiration and always verify the service key on connection. B: 30 days, C: 7 days, D: instance-defined expiration. NIP01 replacement has no mandatory TTL; NIP40 supplies optional expiration.
2. Automatic outbound address policy: recommend A, public HTTPS only with explicit private/development overrides. B: HTTPS including private destinations, C: operator-domain allowlist, D: manual approval per new endpoint. Nostr does not prescribe equivalent server-side outbound safety.

Do not treat recommendations as answers. Existing expected-key behavior remains unchanged until these dependent choices are resolved.

## Additional workflow choices

Asked 7 October 2026 at 17:09 UTC, numbered3–4 to distinguish them from discovery choices1–2. No answer yet.

3. Review intake: recommend A, admitted accounts may submit for one-off review without ongoing publisher permission. Bexplicit review invitations, Cany signing key with configured anti-abuse limits, Dreview intake disabled by default.
4. Group action attribution: recommend A, local operator audit only. Bpublic member tag in group event, Cseparate public signed attribution record, Dno additional member attribution.

These choices do not authorize changing existing behavior until answered. See submission-review-proposal.md for the bounded workflow proposal.

## Resolved

On 7 October 2026 at 18:02 UTC, the user answered `AAAA` (message Sentinel_088ee197e904819198c1145f3d1e5547), selecting A for all four choices. Announcements have no automatic expiry; service possession must be verified on connection. Automatic outbound access permits public HTTPS with exact explicit private/development overrides. Admitted accounts may submit for one-off review. Group acting-member attribution belongs only in local operator audit records. Earlier pending wording above is historical.

## Announcement wire review still pending

Question5 asked 7 October2026 at18:10UTC, message Sentinel_c5c757817a3881918acdb43e1f2e4a48. Recommend A: provisional replaceable kind11998, service-key signature, required protocol discriminator, JSON version1 and up to8 endpoint origins,16KiB event budget, latest timestamp/lowest-ID ordering, no expiry, empty list withdraws addresses, no peer grants. Bsame format with explicitly configured kind/no default; Cfull schema review first; Dpause wire work. The current official kind registry did not list11998, which is not a reservation or proof of non-use. Do not allocate the kind until this review is answered.

Question5 resolved A at18:28UTC on7October2026 (Sentinel_3266c0891fa48191bf714ebc094906a8). The user approved the experimental announcement format and provisional kind11998. It is not an official Nostr allocation. Implementation may now proceed with signature validation, deterministic replacement, empty-list withdrawal and no automatic expiry.
