# Git preservation and recovery

The prepared source baseline is a real local Git commit on an isolated worktree, based on the existing public repository's actual initial commit. It has not been pushed. A connected GitHub write returned HTTP 403, `Resource not accessible by integration`; publication remains blocked until the owner repairs access and a parent task verifies it.

- Repository: https://github.com/9hnm24bzvm-wq/nostr-manga
- Local baseline branch: `checkpoint/final023-preservation`
- Local baseline commit: `7a39e77b9d490c250a4cb1a05402187a50aa1232`
- Actual upstream parent: `e8101c79dc8daf1bb251745e880e397168cef298`
- Baseline Git tree: `d2cd419d56839defb2cf486d2c527751380dd215`
- Source/export manifest SHA-256: `b68e345b4076752091668497216e33ca8e97e01f2eac6d1a4ae927cb8fd5b81e`

The sanitized source export contains 2,044 files / 28,929,988 bytes. The local baseline commit adds one self-excluding manifest, making 2,045 tracked project files. Every copied file was independently rehashed; the worktree is clean, and `git fsck --strict` passed. No project build or test was executed while preparing this baseline.

An exact Git bundle includes original commit-author metadata and is kept local only. It is not included in the encrypted external recovery supplement. This source-and-evidence recovery therefore preserves project bytes and verified commit/tree/parent identity facts, but it cannot reproduce the exact original commit object if the corresponding Git objects are lost and the upstream source is unavailable.

## Recovery without personal Git metadata

1. Restore and hash-verify the main encrypted project checkpoint and later evidence supplement. Never restore credentials, runtime databases, private-key files, browser state, or raw logs into the source checkout.
2. Obtain the existing public repository with a normal read-only clone. Verify its real upstream commit ancestry rather than inventing historical commits.
3. Create a separate feature branch and worktree for recovery. Preserve the adjacent `manga-protocol-lab` and `weebdex-platform` layout.
4. Restore only the sanitized selected source/evidence, original frontend assets, public configuration, and additive recovery notices. Verify the complete export manifest, all 676 supplied frontend identities and all 52 original SQL migrations.
5. Make a new truthful recovery commit. A new recovery commit need not have the old local SHA; its message should identify the restored baseline and state what could and could not be recovered. Do not fabricate past changes or author metadata.
6. Publish only after the connected GitHub route has authorized write access. Preserve any remote changes, avoid force-rewriting `main`, and independently verify the exact remote commit and tree before calling the source backed up.
7. Import the prepared 21-ticket graph as current GitHub issues, retaining completed, bounded-pass, blocked and unexecuted states. Use separate worktrees and incremental commits for subsequent changes.

Compiled executables and frozen frontend build bytes are recovered separately from the encrypted artifact checkpoint. Source Git alone is not a backup of those executable/build bytes. Browser acceptance remains unestablished until run on a supported reachable host.
