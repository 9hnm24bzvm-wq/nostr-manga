# Local-key publication fixture qualification

Test-only source667f6d63 corrects a stale fixture fromf593787. A local-key activation changes canonical database ownership; rewriting the prior review owner/generation cannot adopt its relay selection. The repaired test proves both old and relabelled reviews refuse before any job or POST, explicitly selects recipients for the current key, and uses actual previewPublication before signing.

Before:2/3pass; after:3/3pass. Real deterministic local Schnorr verification, exact immutable manifest/event/job readback and prior-key outbox isolation remain asserted. Production publication bytes and its ownership/mapping/recipient guards are unchanged. Worker/lock/service/network adapters remain finite stand-ins; no live browser, network or backend acceptance is claimed.
