# Approved catalogue and compatibility choices

Asked7October2026 at22:06UTC; owner explicitly approved A/A/A at8October2026 00:07UTC. The choices below are now accepted. Website/UI remains outside scope. These choices gate new optional wire fields/cover dependencies and the compatibility target, not the independent discovery/recovery work.

1. Series metadata: recommended optional descriptions, alternative titles, free-form categories, credits, original language, publication status and year, with titles required and unknown optional fields preserved. Other choices: titles/categories only; exactly MangaDex metadata; another contract. Nostr defines no manga-specific metadata schema.
2. Covers: recommended one optional original-image descriptor in the immutable series manifest, verified and locally retained like chapter images. Alternatives: ordered multiple covers; instance-local covers outside protocol; defer.
3. Compatibility: recommended documented read compatibility followed by explicitly mapped publishing/group operations, retaining key-based write authorization. Alternatives: read-only release; migration only; specific clients required unchanged. MangaDex credentials do not become Nostr signing keys.

Existing accepted choices remain: free-form category text and instance-local synonyms; optional textual chapter/volume labels; no protocol series-equivalence/revision links; exact publication IDs; immutable delete/upload corrections; self-hosted originals; local moderation policy.
