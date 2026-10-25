# Official nginx bootstrap pin, 2026-10-18

The active original-protocol bootstrap now pins official nginx **1.30.6** and its
verified archive SHA256. Official sources: [download](https://nginx.org/en/download.html),
[archive](https://nginx.org/download/nginx-1.30.6.tar.gz),
[stable changes](https://nginx.org/en/CHANGES-1.30). The archive was downloaded
into bounded memory solely to hash it and inspect its safe member metadata and
version header. No archive extraction, build, installation or nginx execution
occurred in this source qualification. This is an HTTPS-origin/hash pin; no
vendor-signature verification is claimed.

Bootstrap `--check` now verifies both the new build path and retained historical
fixture path report the actual pinned version. The known legacy compatibility
symlink is updated deliberately; unfamiliar links, real directories and files
are refused without replacement. The fixture path name remains historical,
while the served version is checked truthfully. Existing historical receipts,
source archives and old binary provenance are untouched.

Checks: `bash -n manga-protocol-lab/scripts/bootstrap.sh`; five finite cases via
`node --test weebdex-platform/tests/ci/test_nginx_bootstrap_pin.mjs`; diff check.
The finite cases execute only the compatibility-link block in fresh temporary
folders and inspect source pin/order/version checks. They do not execute the
bootstrap/download/configure/build or imply a working deployed nginx parser.

## Independent-review successor

The first source-only candidate passed five finite cases but review found that
`--check` could invoke an unfamiliar compatibility path before the later build
branch validated that path. The successor requires an actual symlink to the
exact current relative target before compatibility execution. Six finite cases
now pass, including unknown-link, real-directory and legacy-link marker programs
that must never run. `result.json` preserves the first source association;
`review-correction.json` pins the corrected source/tests. No nginx runtime claim
was added.
