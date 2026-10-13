# PostgreSQL manga node

Current schema:52. The selected server uses Rust,PostgreSQL,deadpool-postgres,tokio-postgres,Moka and real strfry. It does not use SQLx or SQLite.

Start with [the current handoff](../docs/START-HERE.md), [API index](../docs/API-INDEX.md) and [reconstruction acceptance matrix](../docs/rebuild/ACCEPTANCE-MATRIX.md). October8 release evidence is historical. Current focused tests are in results/rebuild-*; final combined acceptance and target-capacity certification remain pending.

## Build and test

From the project root, after the prerequisites in [build.md](../docs/build.md):

```sh
export CARGO_HOME="$PWD/vendor/cargo"
export RUSTUP_HOME="$PWD/vendor/rustup"
export CARGO_TARGET_DIR="$PWD/prototype/target"
export CARGO_INCREMENTAL=0
vendor/cargo/bin/cargo build --locked --release --manifest-path server/Cargo.toml -j2
bash scripts/test-postgres.sh
PYTHONPATH="$PWD/vendor/pydeps:$PWD" python3 -m unittest discover -s tests
```

Use the project's pinned toolchain,or adapt CARGO_HOME/RUSTUP_HOME to your own verified installation. The fixture runner starts isolated local databases and nodes; don't point it at production data.

## Node startup

Prepare the database, service key, administrator public key and storage paths. Review runtime/node.example.json and [security-review.md](../docs/security-review.md). Keep the database DSN in MANGA_DATABASE_URL. Provision secrets through your secure operator workflow, not source control.

```sh
prototype/target/release/manga-node migrate node.json
prototype/target/release/manga-node serve node.json
```

Migrations are explicit and checksum guarded. Raw strfry remains private behind the wrapper. Peer WebSocket AUTH and HTTP request signatures bind to the configured external endpoint. Registration is disabled by default and grants only personal-account authority when explicitly enabled. Group clearing/disabling preserves keys, roles and publications.

The node.example.json placeholders must all be replaced deliberately. The runnable demo uses public synthetic keys and cannot be promoted into a public service by changing its bind address.

Historical implementation notes are retained in [history](../docs/history/server-README-through-checkpoint26.md).
