# Use PostgreSQL for the scale first reference node

Status: accepted by the user on 6 October 2026.

## Context

The reference node must start on a small VPS and grow toward a 100 TB library and very large reader traffic. The initial SQLite implementation established behavioral contracts but centralized metadata access in one process and was not the selected long-term architecture.

## Decision

Use PostgreSQL with deadpool-postgres/tokio-postgres and Moka. Do not use SQLx. Build separable API, storage, validation, replication and projection roles whose correctness survives multiple processes. Keep ordinary public image delivery outside the database path. Use the same architecture at pilot scale with smaller limits.

## Consequences

The service gains an explicit database deployment/migration requirement and must handle pool budgets, concurrent writes, cache consistency and worker fencing correctly. More connections do not fix poor queries or hot rows automatically. Legacy tests remain valuable but must be rerun through PostgreSQL before claiming parity. The database choice does not become a requirement for other interoperable implementations.
