# Run a MangaPub node

## Prepare

1. Install Rust 1.90+ and build with `cargo build --locked --release`.
2. Provision a dedicated PostgreSQL database and restricted database role. Use verified TLS for a remote database.
3. Install strfry and GNU coreutils `timeout`. Keep the raw relay private; clients must pass through the protocol wrapper's authorization boundary.
4. Provision a fresh service signing key in a private file and select a separate administrator Nostr public key. Use the `manga-key-tool` CLI's usage output to inspect its offline key operations. Never commit private keys or database credentials.
5. Copy `config/node.example.json` to a private configuration path and replace every `REPLACE_WITH_...` placeholder. Choose real storage limits, a durable object path, the exact external HTTPS base URL, and private strfry data paths.

The template is deliberately not immediately runnable. Its `database_plaintext_local` setting is false. Do not disable database TLS for a remote database. Self-enrollment is disabled by default; admitting an account does not grant publisher or peer privileges.

Supply the database connection string through `MANGA_DATABASE_URL` using your deployment's secret-management mechanism, rather than saving it in version control or shell history.

## Migrate and serve

```sh
target/release/manga-node migrate /private/path/node.json
target/release/manga-node serve /private/path/node.json
```

Migration is explicit and checksum guarded. Keep backups and review changes before applying migrations to an existing database. The service command does not replace a deployment-specific process supervisor, reverse proxy, certificate renewal, monitoring or backup policy.

The configured base URL must exactly match the URL clients sign for HTTP authorization. The wrapper does not derive trust from arbitrary forwarded headers. Keep PostgreSQL and raw strfry inaccessible as alternate public protocol entry points.

## Optional direct image serving

The normal node can serve guarded originals without NGINX. Enabling managed direct serving is a separate explicit configuration choice for public reads. Its helper path must point to the included `src/serving_supervisor.py`, with Python 3 and a reviewed NGINX executable available. See the `PublicServingConfig` in `server/src/serving_role.rs` and `NodeConfig` in `server/src/node.rs` for the accepted fields and defaults.

The supervisor coordinates a private control channel, exact reader-process ownership and shutdown receipts. Direct serving exposes only verified public original-image aliases, not private staging, manifests, public-library objects, private submissions or keys. An uncertain reader drain or failed helper must not be treated as safe deletion.

## Operational boundaries

- Independent instances retain and moderate their own data. Deleting locally cannot erase copies elsewhere.
- A host holding a group signing key can act as that group. Recovering the key does not automatically establish current local membership or publisher authority.
- Public-library replacement is not global consensus or an automatic multi-device merge.
- Configure disk headroom beyond declared object quotas for PostgreSQL/WAL, temporary data, backups and repair.
- Review current endpoint policies, private-network exceptions, CORS, registration and admission before exposing the service.
- Obtain independent security and deployment review before a public production launch.
