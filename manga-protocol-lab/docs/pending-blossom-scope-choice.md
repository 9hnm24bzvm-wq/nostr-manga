# Pending server-scoping choice

Asked7October2026 at15:49UTC. No answer yet. Recommendation A: require server scope for every authenticated blob action. Alternatives: B uploads/deletions; C deletions only; D accept unscoped tokens for all actions (BUD11 baseline). Keep dependent authorization changes pending while working on independent capabilities.

Official sources checked: https://github.com/hzrd149/blossom/blob/master/buds/11.md (domain-only optional server tags, Base64url without padding, cross-server replay warning); BUD01 mandatory CORS/root retrieval; BUD02 exact-byte upload/descriptors; BUD06 optional upload preflight; BUD12 deletion and optional/unrecommended listing.

Existing source40/41 legacy routes use origin-scoped tokens. Do not silently treat the recommendation as user approval or weaken their default while awaiting an answer.

Resolved 7 October 2026 at16:18UTC: user chose A (message private-chat-reference-020-redacted). Require server scoping for every authenticated blob action by default. Implement domain-only server values on new standard routes, retaining legacy full-origin semantics on legacy routes. Disclose this stricter local policy rather than claiming baseline acceptance of unscoped BUD11 tokens. Anonymous reads still follow instance read policy.
