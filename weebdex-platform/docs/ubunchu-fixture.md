# Bounded local Ubunchu image fixture

## Source and licensing boundary

The user-supplied [GitLab project](https://gitlab.com/ubunchu-translators/ubunchu)
was publicly readable without login on 2026-10-13. The inspected immutable
revision is `9f065a3fde464be893dfba9fb5fc6abc93722cf9`, project468079.
Its tree has8 Japanese chapter directories containing96PNG originals,
11 English PNG and8 English PDF files. Only Japanese chapters1–2 were selected;
English translations/extra art were not selected or given inferred licenses.

The [author's own site](https://seotch.wordpress.com/ubunchu/) credits Hiroshi
Seo (Seotch), publisher ASCII MEDIA WORKS Inc., and declares attribution and
noncommercial conditions for the manga artwork. All8 Japanese releases were
checked for their paired chapter notices. Each English notice links
[CC BY-NC3.0](https://creativecommons.org/licenses/by-nc/3.0/); each Japanese notice
links [CC BY-NC2.1JP](https://creativecommons.org/licenses/by-nc/2.1/jp/).
Those differing supplied versions are preserved, not silently replaced with4.0
or presented as a broad license covering every repository asset.

This is a candidate for genuinely noncommercial local disposable tests, subject
to those conditions. Private testing by itself does not resolve whether a use is
commercial. Commercial use or publicly deploying the images needs a separate
rights decision. No upstream scripts/software were executed, no data was uploaded
externally, and no runtime fixture was started for this data review.

Preserve the downloaded chapter license notices and original credit images.
Use this attribution beside the fixture/catalogue record:

> Ubunchu! Japanese original, chapters1–2. Author: Hiroshi Seo (Seotch).
> Publisher: ASCII MEDIA WORKS Inc. Source: the linked GitLab revision and author's
> site. Supplied notices: CC BY-NC3.0 (English), CC BY-NC2.1JP (Japanese).
> Downloaded originals unchanged. Any later local resized/compressed test
> derivatives must be identified as such. No author/publisher endorsement implied.

Chapter1 is titled “Ubunchu has come along!” and chapter2 “Let's play CUI with
Gnomes!” in their supplied English notices. Retain both notice files per chapter.
Chapter2's credit image is actually named `ubunchu01_00.png`; preserve full paths
instead of flattening colliding basenames.

## Exact selected workload

`tests/fixtures/ubunchu-manifest.json` provides28 exact pinned raw URLs and SHA256
hashes:24 original PNG (22 story pages plus2 credit images) and4 notice files.
PNG total:9,499,508bytes. There are23 images at1246×1635 and one at1246×1633;
20 grayscale, one RGB, and3RGBA images. These are realistic panel/text pages,
not a representative large-manga corpus or measured performance evidence.

The download stays in ignored `.cache/fixtures/ubunchu/<revision>/`. Keep this
cache, image-bearing screenshots and any derived fixture images out of GitHub,
Catbox checkpoints and public artifacts unless the rights/sharing decision is
separately reviewed. The manifest, inventory and numeric receipt contain no
image bytes, credentials or runtime database. A local catalogue fixture never
creates original Nostr admission, visibility or publication authority.

## Bounded reproducible fetch

After reviewing the intended use against the license, deliberately run:

```sh
python3 scripts/fetch-ubunchu-fixture.py --acknowledge-noncommercial
```

The reviewed local utility reads only data from exact HTTPS GitLab URLs, refuses
redirects and unsafe paths, verifies immutable hashes/lengths and uses at most
28files,4MiB/image,128KiB/notice,32MiB total,20s/request and240s overall. It neither
installs software nor starts tests/services. Cached files are verified too.
Actual per-image dimensions/color modes, chapter totals and manifest hash are in
`test-evidence/datasets/ubunchu/receipt.json`. The utility itself has not been
executed as an acceptance test; the recorded data-only download used equivalent
bounds and verified every selected PNG structurally with the installed decoder.
