# Reader queue replacement ownership

A finite mounted-Svelte probe against `ffef606d50d2739ebe4e4841e3bbf88c72304f65`
exposed an initial-key replacement race. A disposed Queue's pending tick could
claim `page.isLoading`; its own `load` then returned because it was disposed.
The live replacement saw the old claim and emitted no image request. This is a
JSDOM/Svelte reproduction, not native-browser acceptance.

The additive `frontend-overlays.mjs` patches recheck disposal after each relevant
await and exact page identity before claiming it. An owned-page set releases
claims synchronously on disposal; late finalizers cannot clear a newer Queue's
claim. Supplied frontend files, markup and styles remain untouched.

`tests/ci/test_reader_queue_lifetime.mjs` executes the actual composed queue
script with controlled ticks. All six assertions fail against the exact old
overlay and pass after the repair: first/second update ticks, process admission,
final per-page tick, obsolete page identity, and synchronous abort/claim release.
The mounted synthetic scenario now observes real initial/selected image HTTP
requests. It uses a digest-checked image and a declared decode stand-in; real
browser decoding, layout, disk IndexedDB and production performance are untested.

Run the finite regression:

```sh
node --no-warnings --test --test-timeout=1000 \
  weebdex-platform/tests/ci/test_reader_queue_lifetime.mjs
```
