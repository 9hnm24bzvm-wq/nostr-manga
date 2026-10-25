# Signed downstream isolation after run 38584846100

## Verified observation

Hosted run 38584846100, attempt 1, used public source e1ea103 / tree
6950c9b7a819874d630831e61ea1907a0c8c40fc. Its verified closed artifacts show
all 23 common setup/security checkpoints passing independently in each of
signed-library, signed-social, signed-source-controls and signed-recipients.
This includes actual local saving, list membership, signer refusal/connection,
wrong-key and disallowed-origin refusal, verified recipient memory and selection.

Later broad original steps failed. Their existing public receipts do not reveal
which inner operation failed. Source review found no additional native select
label mismatch at those first boundaries. No backend, signing, moderation or
projection defect is inferred from these coarse failures.

## Proven comparison defect

The harness formerly hashed JSON.stringify(event) to compare saved signed events
with genuine backend projections. Its synthetic signer returns a JavaScript
object in template-first member order. The original Rust Event has a different
field declaration order, and read_event explicitly serializes
serde_json::to_value(&event). That Value uses sorted object keys with the
original serde_json features. JSON object member insertion order is not part of
the signed Nostr event identity.

The checked-in synthetic public fixtures were generated locally with a genuine
Schnorr signature, then round-tripped by compiling the exact Event declaration
extracted from original auth.rs and invoking the actual to_value operation.
Existing Rust 1.90.0 and cached original-lock serde_json 1.0.151 artifacts were
used, with default/raw_value/std features and no preserve_order feature. No
package installation, service or database was started.

The actual Rust output and Node input have identical seven fields and valid
signatures, but the former object hashes differ. The new test-harness helper
projects exactly id, pubkey, created_at, kind, ordered tags, content and sig in a
fixed order. Missing, unknown, malformed or sparse fields are rejected rather
than silently discarded. Every signature/content/tag byte remains significant.
Actual Nostr id/Schnorr verification remains a separate required check.

Saved-state array equality stays unchanged. Exact metadata request-body byte
hashes also stay separate: transport ownership/count assertions still compare
the actual serialized request bytes, original recipient, two attempts and
unchanged event-signing count. The helper changes neither application behavior
nor the signed event/template/transport body.

This proven mismatch would fail genuine counterpart comparisons. It does not
explain an earlier failure before such a comparison was reached.

## Finite downstream checkpoints

The common 23 checks are unchanged. Four phase-specific inventories append 44
fixed status-only checkpoints around the first failing independent boundaries:

- signed-library: authentication-denied retention, original unadmitted-key retry,
  explicit admission, exact accepted event/projection and federation denial
- signed-social: source conversation, comment/reaction review/send/counterparts,
  genuine feed and rendered comment
- signed-source-controls: admission, source/review preparation, actual moderation,
  visibility/mapping/local-state fences, restoration prerequisite/control/UI
- signed-recipients: secondary admission/proof/selection, two-recipient publication
  and exact projection, separate peer-policy revocation/restoration reads

The existing recorder rethrows original errors. Later unentered gates stay
pending. No diagnostic retains inspected values, messages, bodies or secrets.
The Node inventory and Python projection/gate contract have identical phase/order
requirements. No timeout was increased and no consent, ownership, source,
recipient, signature, no-replay or durable-state assertion was removed.

## Reproduce the Rust serialization witness

From weebdex-platform, using existing compatible compiled artifacts:

```
rustc --edition=2021 tests/ci/fixtures/signed-envelope/rust-event-roundtrip.rs \
  --extern serde=/verified/deps/libserde.rlib \
  --extern serde_json=/verified/deps/libserde_json.rlib \
  -L dependency=/verified/deps -o /tmp/signed-event-rust-serialization
/tmp/signed-event-rust-serialization \
  < tests/ci/fixtures/signed-envelope/node-event.json
```

The output must match rust-event.json. The fixture is a synthetic, cryptographically
signed envelope for serialization testing; its content is not a publishable
library graph. No private signing key is retained.

## Evidence boundary

Local source/driver wiring and cryptographic serialization seams validate this
correction and the fixed diagnostic reachability. They are not new browser,
service, projection, moderation or two-recipient acceptance. A fresh authorized
hosted run is required to locate and establish the remaining actual outcomes.

Local validation on the isolated 94c00b8-based source:

- 149 browser source/driver seams passed when the single current-source-plan
  hash-binding test was explicitly excluded with Node's test-skip-pattern.
- The unfiltered browser glob ran 150 tests: 149 passed; its only failure was the
  expected stale journey-diagnostics.py binding in browser-actor-follow-plan.json.
  That current plan must be refreshed after the combined integration. The test
  and historical plan were not changed or weakened here.
- All 44 Python CI regression tests passed, including the closed diagnostic
  inventory/projection/gate tests. All 676 canonical frontend hashes passed.
- The new diagnostic seam executes actual enclosing source at every added gate,
  injects each of the 44 original-error failures in turn, and checks exact step
  ownership, finite status-only output and no replay/reordering. Inner browser
  operations are explicit stand-ins, so this remains harness evidence only.
- The five envelope tests bind the extracted Rust declaration and actual public
  fixture, reproduce the old hash mismatch, independently verify canonical-array
  event ids and genuine Schnorr signatures, reject malformed/unknown fields, and
  keep transport-byte ownership separate from envelope equality.
