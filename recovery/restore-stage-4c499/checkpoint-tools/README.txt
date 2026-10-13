CHECKPOINT RESTORATION AND BACKUP WORKFLOW
Read RECOVERY-PROVENANCE.json for the precise baseline, modifications and limits.
A partial rebuild is not recovery of the lost October 10 refactor.
Historical source README claims are not recertified by packaging verification.

Source-only restore: install/build dependencies using the original source setup.
Vendor code, browser generated public/, compiled binaries, runtime databases,
private keys, logs/results, generated archives, and hidden work folders are absent.
Known deterministic demo/test seeds remain source fixtures, never production keys.
The node.example.json contains placeholders, not live credentials.

ZIP uses AES-256 authenticated encryption and STORE (zero compression). Filenames
remain visible in ordinary ZIP central-directory metadata. Use an AES-compatible
extractor (7-Zip or pyzipper); operating-system ZIP support may vary.
The password is a separate random 256-bit value; keep it outside this archive.

Workflow dependency: python -m pip install pyzipper==0.3.6
1. Review/update reviewed-files.json for the exact allowed source paths.
2. Review source for credentials and personal data. Built-in marker scans are
   defense-in-depth, not a complete secret or personal-data detector.
3. Prepare provenance JSON naming the precise baseline and exclusions.
4. Run checkpoint.py pack with --root, --allowlist, --provenance, --archive and
   --password-file. Archive and password paths must be new for each checkpoint.
5. Run checkpoint.py upload --archive PATH --password-file SECRET_PATH
   --receipt PATH only when upload to Catbox is authorized.
6. Upload succeeds only after HTTPS readback, byte hash comparison, and full
   AES/SHA/CRC/size verification. Only then does the success receipt change.
   Failed upload/readback keeps the previous receipt; a returned URL is saved
   separately as .pending-upload.json. Do not retry uncertain uploads blindly.
7. Deliver the verified URL/hash and password separately to the intended user.

Passwords never appear in CLI arguments, manifests, receipts or normal output.
Password files are mode 0600. Retain them securely until the user can decrypt.
Receipt commits use an atomic same-directory replace. Preserve older checkpoints.
