# UUID signed-source discussion resolver

The existing title/chapter source discussion controls now resolve UUID resources through the read-only versioned `/api/v1/nostr/resources` contract from mapper commit `b77de36`. The returned original event, publisher, source kind and parent provenance remain distinct from the public resource UUID. Exact service/origin/catalogue scope, complete requested coverage, bounded response, discussion context and unchanged saved scope are required. Missing/revoked/native-only originals remain unavailable. The native gate-off 10-character resolver is unchanged.

This resolver never signs, posts comments, enrolls accounts or publishes a library. Existing explicit review/consent and original event signature checks still own public actions. The actual discussion context is now checked inside source resolution as well as when its result is installed.

11/11 finite mapper/orchestration tests pass. They execute the real strict mapper and source resolver with finite transport/scope boundaries, without a browser, signer, account or external service. Combined composed strict checks for this source plus the following UUID local-library batch pass in both modes (0 errors, 15 retained warnings); the complete receipt is stored with the local-library integration evidence. Real browser/relay acceptance remains unverified.
