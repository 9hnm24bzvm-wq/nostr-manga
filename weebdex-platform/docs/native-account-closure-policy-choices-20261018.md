# Native account closure: three policy choices

Source review only, recovery 5cbd05d9. Native DELETE /api/v1/user/{id} and POST /api/v1/user/delete/{code} remain unsupported; this document implements no account closure, deletion, retention job or credential change. Original Nostr kind 5 authored-event withdrawal is a separate action and cannot close an instance account or promise relay erasure.

## Recommended first milestone

Offer an explicit, recoverable account-disable request rather than irreversible erasure. Require a current normal native session with recent reauthentication, exact captured native account/user revision and one-use purpose-specific confirmation. Atomically disable account authority and revoke all sessions/API clients/active auth flows and instance key bindings. Do not grant a device-local or signer identity native-account ownership. Stop unsent account-owned work and prevent new writes; never sign original withdrawal/deletion events as a side effect. Read access, reactivation and public presentation still need a defined policy before implementation.

The actual native principal rejects disabled users for both sessions and API clients (backend/src/domains/auth/principal.rs); explicitly bound Nostr account reads/writes also reject disabled users (backend/src/public_api/auth.rs). Session/client invalidation and native profile shared-user revisions exist, but there is no reviewed all-account closure transaction. Existing foreign keys, ownership, statistics/revision ledgers and audit evidence cannot be treated as cascade-delete permission.

## Smallest decisions the user must make

1. Recovery: should disabling be reversible indefinitely, or only during a chosen grace window? Recommendation: start with reversible disabling and a separately reviewed reactivation path; offer no erasure deadline or automatic purge yet.
2. Data treatment: which public contributions/profile identifiers and private account records should remain after disabling? Recommendation: initially retain the database records without public promises of erasure; suppress direct contact/biography in the closed-account presentation, preserve public discussion/catalogue provenance and restricted moderation/audit records, and let the user export private records before closure. Any future private-data removal/anonymization needs explicit retention and aggregate/revision rules, including copied author names and signed-source boundaries.
3. Shared ownership: what happens when the account is the only group leader/content owner or has pending publication/review work? Recommendation: require a deliberate handoff or operator-reviewed resolution first. Do not delete shared catalogue/community content, silently transfer original signer rights or invent private ownership.

Credential/session/binding revocation, fresh captured intent, clear pending/confirmed feedback and refusal of ambiguous outcomes are safety requirements for the implementation, not optional checkbox choices. A key's separately signed public graph and the browser's device library remain independently controlled. Private cross-device library storage is an optional product expansion, not a prerequisite for local-first reading or account disabling.
