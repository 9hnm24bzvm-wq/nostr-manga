# Recovered upload product source audit

This work starts from verified older remote commit e7588f7d, not restored lost product2c7/fd096fe source. The supplied frontend files remain unchanged; new behavior is supplied by reviewed exact-source overlays.

## First reconstruction: begin/autosave

The recovered UploadChapter and UploadChapterEdit compositions assign a begun session without establishing a saved signature. The new overlay acknowledges only the returned server draft/files; it does not overwrite the local draft. Retained language and newer edits remain dirty. Autosave skips invalid empty language rather than issuing an invalid PATCH. Existing capture-before-save/acknowledge-after-success behavior is retained, with focused finite refusal and late-response seams.

Source audit before test execution: the relevant overlay module imports only Node fs/path/crypto and contains exact string replacements. Tests parse those replacement records as data and evaluate only the selected composed begin/persist and upload-resume snapshot/acknowledge bodies with explicit local save inputs. They do not import application dependencies, open sockets, execute package scripts, build, or claim durable/backend acceptance. The original immutable frontend hashes and every relevant replacement count are checked.

## Accepted202 reconstruction gap

The recovered base uses native targetOwner/HTTP helpers and does not contain the late signed public upload-client. Its native/public upload commit handlers return Json200, and its routers expose current-session GET rather than the late exact-session receipt GET. The historical accepted202 fence cannot be copied into this different source by pretending those APIs exist. A meaningful existing-adapter fence may be added separately, but no backend receipt endpoint or full public adapter is invented here. Actual MangaDex-compatible public contracts and Nostr authorization require their own recovered/reconstructed source and verification; no parity claim is made.

No runtime/build/remote upload is performed by this source audit or finite seam design.
