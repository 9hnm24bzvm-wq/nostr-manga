# Additive migration ordering

The applied migration list is an immutable ordered prefix. A new migration must
be appended after all migrations that may already have been applied. Numeric
gaps are allowed, but an earlier number cannot be inserted later into history.

- 89: public resource aliases (reconstructed, exact protocol UUID codec retained)
- 90: reconstructed versioned manga edge aliases
- 91: original signed public-list aliases and bounded projection cursor
- 92: report reasons/versioning
- 93: permanently retired reservation; no migration ever existed at this number
- 94: cached parent withdrawal consistency for group/forum visibility
- 95: next available slot for settings, not implemented or reserved by source here

Historical checksummed migrations are unchanged. Migration94 retains the original
live-source checks and ordinary unmapped native-group visibility, while enforcing
the same persisted parent withdrawal gate used by the public catalogue/media.
