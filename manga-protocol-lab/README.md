# Federated manga backend

The current implementation is a Rust/PostgreSQL backend built around Nostr and strfry. It uses deadpool-postgres, tokio-postgres and Moka; no SQLx or SQLite is used by the current server. Historical SQLite experiments remain under prototype/ and are not the current run path.

## Current reconstruction status

This tree was recovered from October 8 checkpoint26/schema52 and reconstructed on October 10 after the later temporary workspace was lost. Historical release counts do not validate this tree. The feature-first reconstruction is active; final combined functional acceptance and production traffic capacity remain open.

The backend supplies signed catalogue publication, retained originals, approved federation, instance policies, managed groups, social events, public library operations and recovery tooling. The functional browser client is local-first. Full catalogue/reader/social/moderation website presentation remains a separate phase.

Current evidence and precise remaining gaps:
- [Acceptance matrix](docs/rebuild/ACCEPTANCE-MATRIX.md)
- [Rebuild status and safeguards](docs/rebuild/STATUS.md)
- [Backend reconstruction](docs/rebuild/BACKEND.md)
- [Operator tests](docs/rebuild/OPERATOR.md)
- [Browser client and its verification boundaries](browser-client/README.md)
- [Verified encrypted checkpoint receipts](docs/rebuild/CHECKPOINTS.json)

## Build and test

Follow [build.md](docs/build.md) and [the restored toolchain record](docs/rebuild/TOOLCHAIN.md). On the prepared research environment:

```sh
bash scripts/test.sh
```

That canonical gate runs current Python/operator, package, browser model/build and real PostgreSQL/strfry integration checks. `scripts/deep-test.sh` is an alias. Model browser tests are distinct from rendered-browser acceptance. Never run fixture scripts against production databases or keys.

See [START-HERE](docs/START-HERE.md) and [API index](docs/API-INDEX.md). Source backups exclude installed toolchains, runtime databases and credentials. Verified checkpoint browser build assets are included; server release artifacts will be identified separately after exact-build verification.

## Important decisions

- One public library per key; no private/master-key library hierarchy
- Immutable chapters/series; corrections use deletion requests and new uploads
- Each instance retains its originals and independently moderates content
- Approved-peer federation and reader/account/publisher permissions are separate
- Optional self-registration is disabled by default
- Groups use instance-custodied keys; clearing/disabling does not erase keys, roles or publications
- MangaDex-shaped mapped APIs use Nostr signatures, not MangaDex credentials

Earlier milestone notes are preserved under [docs/history](docs/history/README-through-checkpoint26.md). Raw benchmark supplements remain indexed by RAW_DATA_ARCHIVES.json. Historical experiments do not certify8B monthly images,350M page views,100TB or300M monthly progress saves. Further performance experiments remain paused under the feature-first instruction.
