# Security and deployment boundary

Current first-party review: checkpoint26/schema52,8October2026. This records tested safeguards and known limitations. It is not an independent audit or a public-deployment approval. Historical prototype findings are preserved in [history](history/docs-security-review-through-checkpoint26.md); they include obsolete SQLite-era descriptions and are not the current architecture.

## Safeguards with local evidence

- Signature/event-ID verification; exact NIP98 URL/method/body binding; persistent replay rejection
- Independent account,publisher,peer and group-role permission fences; revocation rechecked at commit
- Separate approved-peer read-only NIP42 stream, endpoint/challenge binding,idle-auth timeout and revoked-peer disconnection
- Real TLS/WSS nginx proxy checks, verified certificates/hostnames and rejection of forged forwarding scope
- Bounded original uploads and manifests, hash/size/format validation, private staging and complete-before-hosted admission
- Retained-original accounting, generation-fenced cleanup, shared-reference preservation and guarded reader lifetime
- Moderated protected deletion, ordinary author deletion, persistent tombstones and no automatic resurrection
- Signed service identity/endpoint discovery, exact private-network exceptions and destination pinning for learned replication routes
- Group local role/audit checks, offline key recovery and explicit fresh-authority bootstrap
- Closed-default bounded enrollment, operator denial fences and disabled registration on restore
- Temporary permission/metadata database failures remain503; true denial remains403

See [requirement-evidence-matrix.md](requirement-evidence-matrix.md) and [RELEASE-20261008.md](RELEASE-20261008.md) for files and fixtures. Passing tests establish the exercised contracts, not every possible exploit or race.

## Operator launch checklist

1. Provision fresh private service/group keys and separate administrator authority. Never reuse public demo keys. Protect key files, DSNs and backup encryption secrets with your chosen secure operator workflow.
2. Keep raw strfry and PostgreSQL private. Expose only deliberately reviewed wrapper/proxy routes. Configure the exact external HTTPS/WSS origin; signatures must not follow untrusted forwarding headers.
3. Review verified database TLS, certificate renewal, hostname changes and endpoint announcements. Local test certificates are fixture-only and must not become trusted production roots.
4. Review registration, reader/publisher/peer admission, CORS origins, private-network exceptions and per-instance moderation before activation. Registration does not grant publishing or federation privileges.
5. Set staging/physical limits below actual available storage with headroom for database/WAL,temporary files,backups and repair. The200GB example is a configuration budget,not a disk-capacity guarantee.
6. Exercise offline backup/restore in a separate environment and review paused roles before reactivation. Recovering a signing key does not restore or prove local membership authority automatically.
7. Add deployment-appropriate process supervision, request/connection fairness, monitoring, alerting and tested certificate/key rotation. In-process limits alone do not solve public abuse or denial of service.
8. Obtain independent security review and actual deployment capacity/durability measurements before public launch. None are supplied by the local demo.

This checklist does not configure a real deployment or authorize changing system security settings.

## Remaining qualification

No public deployment, production certificate/key lifecycle, independent-machine disaster test, physical power-loss durability, remote-object-store durability,universal Sybil/abuse protection or target monthly traffic certification has been performed. Existing founder-loss fixtures use separate nodes/logical databases in a shared test environment.

The host of a group key can impersonate that group; moving the same key does not revoke retained copies. Signed service identity proves possession/continuity, not reputation or a TLS-channel-bound attestation. Public data can be copied and local deletion cannot compel another instance. Libraries use the chosen last-write-wins semantics; fresh-device recovery needs a surviving known route. Latent storage corruption needs an operational scrub/backup strategy, even though detected corruption is tested to withdraw affected hosted content.

No unchanged external MangaDex client or browser/mobile signer adapter is certified. Raw strfry must never be exposed as an alternate authorization path. Further performance experiments remain paused; earlier measurements retain their explicit limits.
