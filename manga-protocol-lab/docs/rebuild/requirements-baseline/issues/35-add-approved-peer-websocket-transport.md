# 35: Add approved peer WebSocket transport

Status: done (final71-group regression passed)

## Acceptance

- NIP42 and approved-peer allowlist protect a distinct read-only peer event interface.
- Normal reader policy and publication/group signing authority remain unchanged.
- Revocation, unauthenticated/unapproved keys and attempted writes are exercised.

## Evidence

- docs/requirement-evidence-matrix.md
- results/peer-websocket-focused.log
- results/peer-websocket-release-http.log
- results/peer-websocket-tls.log

- results/backend-mvp-final-full.log
