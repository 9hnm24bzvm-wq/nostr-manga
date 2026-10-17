# Persisted relation and recommendation UI reads

The retained Related and Recommendations controls now read the reconstructed public API's real persisted edge endpoints. The bridge uses GET `/api/v1/manga/{uuid}/relation` and `/api/v1/manga/{uuid}/recommendation`; it does not fall back to native catalogue commands.

Recommendations retain the server's real edge UUID, integer score, ranking order, fixed top-20 window and eligible total. Related titles retain the real directional edge UUID, relation label and owner-manga version domain. Edge provenance is stored separately from the displayed target manga UUID. No edge identity is used as a title identity or mutation authority.

Included edge attributes do not contain nested cover/creator relationships. The bridge therefore hydrates only the exact visible target manga UUIDs in batches of at most 100 with cover, author and artist includes. Missing, duplicate, out-of-scope, self-target and malformed rows fail explicitly; substitute titles, synthetic scores and fake counts are not invented. Edge-operation pagination is unsupported and is rejected rather than forwarded or silently dropped.

In public-catalogue mode, published titles expose the existing Related tab even when the title-detail DTO has no embedded relation list. The normal related-title component reads the dedicated endpoint and retains its existing preferences and empty state. Recommendations retain their existing owned read/retry/empty-state UI. The original frontend files and markup remain unchanged; the visibility change is an exact script-only overlay. The product gate still defaults off.

Fresh results are in `test-evidence/catalogue-edges-frontend-20261017/summary.json`: 120 focused checks passed, including 51 public API cases and 69 existing upload adapter/provenance cases. The composed enabled strict check passed with zero errors and 15 retained warnings across 679 unchanged canonical frontend files and 85 reachable integration files. These are finite adapter/source/type checks. They do not establish live HTTP, browser, production build or complete API parity. The API worker supplies and separately validates the matching backend endpoints.
