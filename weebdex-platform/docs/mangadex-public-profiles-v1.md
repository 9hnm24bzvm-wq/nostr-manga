# Reconstructed public user presentation

RECONSTRUCTED UNVERIFIED; source must be compiled, genuinely tested and reviewed again.

GET `/api/v1/user/{id}` resolves an existing typed User UUID, with no query parameters and no GET alias writes. It grants no admission, native-account binding, session or signing authority.

ProtocolPublisher targets use the shared original public_account_profile repository lookup, also used by the native reader facade. It reads current visible signature-verified kind0 metadata, validates exact owner/kind/object shape, and rechecks head/visibility. Replaced/deleted metadata never falls back to an old cached name. A hosted public publisher is known independently of admission; admission or a retained alias alone cannot create a public identity. Unknown publisher404. Known publisher with absent metadata gets null signed profile and empty metadata.

Display presentation uses bounded display_name then name. The fallback is the FULL original pubkey, explicitly labeled nameIsPublicKeyFallback, not an invented human username. Nostr metadata grants no roles, so roles[]. No original mutable revision counter exists, so version is omitted, not fabricated from timestamp or constant. The signed event, exact pubkey and published metadata remain explicit provenance.

NativeResource targets use the distinct existing NOT-disabled public user policy through a whitelist: actual username, public roles and persisted version only. Email, credentials, profile/contact JSON, permissions, preferences and sessions are not selected. Missing/disabled native user404. A wrong-kind UUID404. Native and protocol target types never merge authority.
