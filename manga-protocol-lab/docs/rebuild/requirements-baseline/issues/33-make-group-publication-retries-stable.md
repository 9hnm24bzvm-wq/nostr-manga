# 33: Make group publication retries stable

Status: done

## Acceptance

- Persisted caller timestamp and fixed publication payload retain Nostr event ID.
- Retry after temporary grant expiry returns committed event without new storage or audit.
- Current authority, review verdict and local removal remain enforced.
- Checked API index includes all wrapper routes and current feature contract links.

## Evidence

- results/group-retry-focused.log
- results/http-group-retry.json
- results/features-api-index-python.log
- docs/API-INDEX.md
- docs/group-publication-retries.md
- results/group-retry-shared-full.log
- results/group-retry-shared-release-http.log
- results/group-retry-shared-demo.log
