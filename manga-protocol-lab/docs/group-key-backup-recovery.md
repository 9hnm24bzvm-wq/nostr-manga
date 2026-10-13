# Offline managed-group key backup and recovery

A managed group is an ordinary Nostr key held by its instance. This procedure preserves that identity; it does not transfer trust, recreate membership, undo revocation or remove anyone else's copy of the key. The operator and designated group administrator must agree on who holds the backup.

## Prepare an offline backup

1. Obtain the group's public key from its existing account/group record. Keep that public value separately from the backup so recovery can verify the expected identity.
2. Locate the instance's configured `group_keys_dir`. The key file is `<group-public-key>.key`. Do not paste its contents into chat, HTTP requests, shell arguments or logs. Never include this directory in public source/checkpoint uploads.
3. Use an operator-controlled, encrypted backup destination with a pre-existing directory that is not writable by other users. Encryption is supplied by your chosen storage system; the helper does not encrypt raw key files or manage passphrases.
4. Run the offline tool, substituting file paths and the public key only:

```sh
manga-key-tool backup /private/groups/GROUP_PUBLIC_KEY.key /encrypted-offline-backup/GROUP_PUBLIC_KEY.key GROUP_PUBLIC_KEY
```

The tool checks the source's public-key identity, regular-file status, restricted permissions and bounded format. It creates a new destination with owner-only permissions, syncs file and directory, then verifies the saved key by reading it back. Existing destinations, including symlinks, are never overwritten. It performs no network or database work and prints only public verification metadata.

5. Check exit status zero and `verified:true`, then verify the detached/remounted backup again:

```sh
manga-key-tool verify /encrypted-offline-backup/GROUP_PUBLIC_KEY.key GROUP_PUBLIC_KEY
```

Store the backup offline. A failed write/sync/readback is an unconfirmed backup; a partial destination may remain. Review it before retrying with a fresh destination. This helper is intended for trusted operator-controlled local directories, not an adversarial filesystem or hardware vault. Buffer zeroization does not guarantee every cryptographic-library or operating-system copy is erased.

## Recover on another instance

1. Prepare the replacement instance with its own service identity and operator policy. Restoring a group key must not automatically import the former instance's peer, publisher or account permissions.
2. Use the same offline backup command to copy the verified backup into the replacement instance's configured group key directory, using the required `<public-key>.key` filename. Verify it again locally.
3. As the replacement instance's operator, send a NIP98-authenticated POST to `/admin/groups/import` with JSON `{"key":"GROUP_PUBLIC_KEY"}`. The value is public. The server reads its already-local protected key file; there is no private-key upload API.
4. Check `membership_recreated:false`. The imported key alone grants no old member access.
5. Explicitly bootstrap the intended new administrator through the operator-authenticated POST `/admin/groups/GROUP_PUBLIC_KEY/bootstrap` with `{"administrator":"ADMIN_PUBLIC_KEY"}`. This only works for a disabled, key-only recovered group without existing members. Subsequent membership changes use the usual group-admin API.
6. Inspect `/groups/GROUP_PUBLIC_KEY/members` as that administrator. Restore only the roles you currently intend. Old members must remain unable to act until deliberately admitted.
7. Publish/read a test through the normal scoped API and confirm the event author is the original group public key. Review replication/admission independently on every instance.

Copying a key cannot revoke a compromised old holder. If the old private key is compromised, preserving the same identity also preserves that holder's signing ability. Key rotation/migration requires a separate explicit policy; do not mistake host shutdown or membership removal for private-key revocation.

## Verification

`offline_keys` unit tests cover identity mismatch, malformed/oversized source, loose permissions, source/destination symlinks, successful copy/readback and refusal to overwrite. The real two-instance `http_groups` fixture uses the backup binary before public-key registration, then verifies that no membership is recreated and explicit bootstrap grants only the chosen new administrator. Only synthetic test keys are used.

## MVP lifecycle decision (8October2026)

The owner chose profile clearing and operator disabling, without a group-deletion endpoint. Empty group metadata clears descriptive fields; disabling the group prevents new signing operations under its versioned authority. These actions preserve the key file, membership/roles and already published content. Disabling is reversible through the operator's existing API. No permanent retirement, key-erasure or group-record deletion workflow is included in the MVP. Ordinary chapter/series deletion requests remain separate and locally moderated.
