// Pure helper admission only. No key generation, nak execution or listener.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { localOrigin, plannedRelayPort, narrowFilter, NAK_SHA256, MAX_SECONDS } from '../../scripts/ci/independent-nak-browser/fixture.mjs';
test('independent signer only accepts a canonical explicit unprivileged loopback browser origin', () => {
  assert.equal(localOrigin('http://127.0.0.1:45678'), 'http://127.0.0.1:45678');
  for (const origin of ['https://example.invalid', 'http://localhost:45678', 'http://127.0.0.1:80', 'http://127.0.0.1:45678/', 'http://user@127.0.0.1:45678', 'http://127.0.0.1:45678/path', 'http://127.0.0.1:45678?q=x', 'http://127.0.0.1:45678#hash', 'ws://127.0.0.1:45678']) assert.throws(() => localOrigin(origin));
});
test('planned relay port is exact, unprivileged and cannot silently choose a random replacement', () => {
  for (const port of [1024, 45678, 65535]) assert.equal(plannedRelayPort(port), port);
  for (const port of [undefined, null, 0, 80, 1023, -1, 65536, 1.5, NaN, '45678']) assert.throws(() => plannedRelayPort(port));
});
test('forwarding-only relay filter cannot subscribe to public application events or arbitrary recipients', () => {
  const recipient = 'a'.repeat(64), author = 'b'.repeat(64), valid = { kinds: [24133], '#p': [recipient], authors: [author], since: 1, limit: 64 };
  assert.equal(narrowFilter(valid, recipient, author), true);
  assert.equal(narrowFilter({ kinds: [24133], '#p': [recipient], limit: 0 }, recipient, author), true);
  for (const change of [{ kinds: [0] }, { kinds: [24133, 27235] }, { '#p': [recipient, author] }, { authors: ['c'.repeat(64)] }, { limit: 65 }, { since: -1 }, { search: 'anything' }]) assert.equal(narrowFilter({ ...valid, ...change }, recipient, author), false);
  assert.equal(narrowFilter(null, recipient, author), false);
});
test('fixture pins official binary and never implements bunker decryption or event signing', () => {
  assert.equal(NAK_SHA256, 'e2fbbdf24cda6fae99c777f11003e0b517b43ac1a40224c78fa87a051354e394');
  assert.equal(MAX_SECONDS, 1200);
  const source = fs.readFileSync(new URL('../../scripts/ci/independent-nak-browser/fixture.mjs', import.meta.url), 'utf8');
  assert.doesNotMatch(source, /schnorr\.sign\(|nip44|\.decrypt\(|\bfetch\(/);
  assert.equal((source.match(/schnorr\.utils\.randomSecretKey\(\)/g) ?? []).length, 1);
  assert.match(source, /spawn\('\/usr\/bin\/timeout'/);
  assert.match(source, /fs\.openSync\(logPath, 'wx', 0o600\)/);
  assert.match(source, /fs\.chmodSync\(runtime, 0o700\)/);
  assert.match(source, /fs\.rmSync\(runtime, \{ recursive: true, force: true \}\)/);
});
