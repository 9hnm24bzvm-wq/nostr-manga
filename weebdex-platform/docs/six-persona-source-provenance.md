# Source provenance and qualification

The six-persona changes use one exact, reversible source-association layer and one final frontend composition stage. Earlier qualification receipts and their source hashes are retained unchanged. Historical component tests explicitly restore their predecessor; current behavior tests execute the current components and clients.

The qualified predecessor tree is `a035b00056557f83348db13c6288a680fd35aca8`, published in commit `c6808f4ade487d3384a9d062405948c6e4e4d76f`. The corresponding local checkpoint commit was `9a70b771bfa8cb1b20f82c926f65956b93c2d5e6`; differing commit metadata does not change the tree. Reproduction depends on the published tree, not that local-only commit ID.

From a clone with the review branch fetched:

```sh
git rev-parse c6808f4ade487d3384a9d062405948c6e4e4d76f^{tree}
git show a035b00056557f83348db13c6288a680fd35aca8:weebdex-platform/backend/Cargo.toml
```

`docs/fixtures/six-persona-source-association-20261028.json` binds current bytes to predecessor bytes, with exact reverse edits for offline historical comparisons. Its integrity is checked by the comparison helper. Source association is not evidence that behavior, HTTP, browser, or release gates passed.

The optional remote-signer dependency is restored separately with `python3 scripts/restore-remote-signer-deps.py`, using the reviewed lock and official archive integrity values without package hooks. Production preparation and strict checks verify the archives and package files, including the existing cryptographic aliases. No signing key, relay session, account credential, or database is supplied by this process.

Current execution receipts must be consulted separately. The published WIP checkpoint and the prior qualified build must not be described as a qualified release of these newer changes.
