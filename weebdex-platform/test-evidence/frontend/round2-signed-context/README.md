# Round-two signed-conversation source repair

Scope: UI2-10 from `docs/round2-ui-source-audit.md`.

Only `platform-integrations/local-library/ProtocolIdentityPanel.svelte` and `protocol-identity.ts` are changed by this patch. The frozen source patch and SHA-256 receipt are adjacent to this file. The previous panel hash is independently checked against its recorded pre-repair hash. This is source evidence, not browser acceptance.

The selected native target, recipient origin/pinned key and signing identity generation form one context. A context change synchronously clears transient conversation source/events/reply/review/consent; metadata is cleared on transport/identity changes, while durable exact signed intents and their receipts are preserved. Mapping, conversation and metadata reads capture their context across successful and failed awaits, so stale results do not repopulate a different selection. Reply and withdrawal handlers validate membership in the currently verified collection.

Initial signed delivery uses the same context guard during preflight and before transport. If context changes while the external signer is open, a valid returned event is saved durably before the next context check, then remains unsent at its original recipient. An already sent request cannot be recalled; its exact receipt or outcome-unknown record is retained. Explicit saved-record reconciliation/retry uses that record's immutable original recipient/body, independently of the newly selected conversation. The outbox exposes original signed tags/content and receipts name their recipient/target.

The frontend owner independently reread these seams and reports source compilation PASS. No runtime tests were run by this repair owner. Browser acceptance must cover A→B and A→B→A edits, target-kind changes, recipient/service-key/signer changes, component disposal, delayed successful/failed mapping/social/metadata reads, target changes during signer/authentication prompts, and exact original-recipient reconciliation after accepted/lost responses.
