#!/usr/bin/env node
// Fixed downstream diagnostic wiring only. Inner actions are explicit stand-ins;
// this test neither starts nor claims any browser, service, signer or database.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import { createJourneyCheck } from '../../scripts/journey-checks.mjs';
import { signedDownstreamCheckSuffixes, signedLibraryDiagnosticChecks } from '../../scripts/browser-signed-journeys.mjs';
const expected = {
  "signed-library": [
    "auth-preview-send",
    "auth-exact-job-retained",
    "auth-intent-bound",
    "auth-unadmitted-retry",
    "auth-refusal-verified",
    "admission-exact-prerequisite",
    "admission-exact-retry",
    "admission-exact-accepted",
    "admission-exact-projected",
    "admission-peer-denied"
  ],
  "signed-social": [
    "social-conversation-loaded",
    "conversation-target-controls",
    "conversation-mapping-response",
    "conversation-root-response",
    "conversation-feed-response",
    "conversation-source-visible",
    "social-comment-reviewed",
    "social-comment-sent",
    "social-comment-counterpart",
    "social-reaction-reviewed",
    "social-reaction-sent",
    "social-reaction-counterpart",
    "social-feed-verified",
    "social-comment-rendered"
  ],
  "signed-source-controls": [
    "source-account-admitted",
    "source-identity-selected",
    "source-conversation-loaded",
    "conversation-target-controls",
    "conversation-mapping-response",
    "conversation-root-response",
    "conversation-feed-response",
    "conversation-source-visible",
    "source-comment-reviewed",
    "source-snapshot-reviewed",
    "source-revoked",
    "source-hidden",
    "source-mapping-fenced",
    "source-library-review-fenced",
    "source-social-review-fenced",
    "source-state-preserved",
    "source-restore-prerequisite",
    "source-restored",
    "source-restore-mapping",
    "source-restore-ui"
  ],
  "signed-recipients": [
    "secondary-admitted",
    "secondary-remembered",
    "secondary-selected",
    "two-snapshot-reviewed",
    "two-snapshot-published",
    "two-snapshot-accepted",
    "two-snapshot-job-retained",
    "two-snapshot-primary-receipt",
    "two-snapshot-secondary-receipt",
    "two-snapshot-projected",
    "peers-published-prerequisite",
    "peers-revoked",
    "peers-revoked-reads",
    "peers-restored",
    "peers-restored-reads"
  ]
};
const source = fs.readFileSync(new URL('../../scripts/browser-signed-journeys.mjs', import.meta.url), 'utf8');
const start = source.indexOf('    if (phase === "signed-source-controls") {');
const end = source.indexOf('    report.signed_protocol_scope =', start);
assert(start > 0 && end > start);
const body = source.slice(start, end);
const labels = new Set([
  'denied authentication proof and actual unadmitted key retain one exact approved snapshot',
  'explicit original account admission and exact approved retry reach genuine public projection',
  'real canonical source conversation supports reviewed signed comment and reaction',
  'genuine source moderation removes current mapping and fences both stale signed reviews',
  'exact source restoration joins listeners and reprojects identical recorded publication',
  'two independently verified real recipients accept the identical reviewed library event',
  'actual mutual peer revocation never grants browser accounts federation authority',
]);

test('downstream public identities are fixed, phase-scoped, frozen and appended to unchanged common checks', () => {
  assert.deepEqual(signedDownstreamCheckSuffixes, expected);
  assert(Object.isFrozen(signedDownstreamCheckSuffixes));
  for (const [phase, suffixes] of Object.entries(expected)) {
    assert(Object.isFrozen(signedDownstreamCheckSuffixes[phase]));
    assert.deepEqual(signedLibraryDiagnosticChecks[phase].slice(23), suffixes.map(suffix => phase + '-' + suffix));
    assert.equal(signedLibraryDiagnosticChecks[phase].length, new Set(signedLibraryDiagnosticChecks[phase]).size);
    for (const suffix of suffixes) assert.match(suffix, /^[a-z0-9]+(?:-[a-z0-9]+)+$/);
  }
});

async function probe(phase, failAt) {
  const report = {}, steps = [], check = createJourneyCheck(report);
  const failure = Error('private synthetic downstream error');
  const event = { id: 'private synthetic event', kind: 11999 }, job = { id: event.id, event, recipients: [{ message: 'private receipt' }] };
  const globals = {
    phase, report, signedCounts: new Map(), mutationRequests: [], lifecycleCoverage: {},
    seeded: { catalog_namespace: 'private namespace', series: { event_id: 'private source' }, chapters: [{ event_id: 'private chapter' }] },
    protocol: 'private origin', owner: 'private public key', fixture: { service_key: 'first service' },
    second: { origin: 'private second origin', service_key: 'second service' }, recipients: [{ origin: 'private origin' }, { origin: 'private second origin' }],
    insist(condition, message) { assert(condition, message); },
    async databases() { return { device: { records: [] } }; },
    async actions() { return [{ event: { ...event, kind: 1111 } }]; }, async counterpart() { return { body: event }; },
    signedEventDigest() { return 'private digest'; },
    async step(label, operation) { if (!labels.has(label)) return; const row = { label }; steps.push(row); try { await operation(); row.status = 'passed'; } catch (error) { row.status = 'failed'; row.error = error; } },
    // Explicit navigation-only stand-ins for the two real route-owned review
    // pages. Diagnostic callbacks below remain responsible for every enrolled
    // checkpoint; this wiring driver neither models nor claims native dialogs.
    page: { async close() {} }, context: { async newPage() { return { async close() {} }; } }, ownedPages: new Set(),
    prepareIdentity() {}, prepareSignerControls() {}, open() {}, selectSigner() {}, reviewSnapshot() {}, sendAction() {},
    inspect(suffix, operation) {
      assert(expected[phase].includes(suffix)); assert.equal(typeof operation, 'function');
      return check(phase + '-' + suffix, async () => {
        if (suffix === failAt) throw failure;
        // Only this callback must run to preserve the enclosing lexical assignment;
        // its actions/counterpart reads above are explicit private stand-ins.
        if (suffix === 'social-comment-counterpart' || suffix === 'peers-published-prerequisite') return operation();
        if (suffix === 'social-conversation-loaded' || suffix === 'source-conversation-loaded') { for (const name of ['conversation-target-controls', 'conversation-mapping-response', 'conversation-root-response', 'conversation-feed-response', 'conversation-source-visible']) await globals.inspect(name, async () => {}); }
        if (suffix === 'auth-exact-job-retained') return [job];
        if (suffix === 'two-snapshot-accepted') { for (const name of ['two-snapshot-job-retained','two-snapshot-primary-receipt','two-snapshot-secondary-receipt']) await globals.inspect(name, async () => {}); }
        if (suffix === 'auth-refusal-verified' || suffix === 'admission-exact-accepted' || suffix === 'two-snapshot-accepted') return job;
        if (suffix === 'admission-peer-denied') return { status: 403 };
        if (suffix === 'social-reaction-counterpart') return event;
        if (suffix === 'social-feed-verified') return { body: { events: [] } };
        if (suffix === 'source-revoked') return { result: { success: true } };
        if (suffix === 'source-restored') return { restored: {}, source: { event_id: 'private restored' } };
      });
    },
  };
  await vm.runInNewContext('(async () => {' + body + '})()', globals);
  assert(steps.length > 0);
  if (failAt) {
    assert.deepEqual(report.journey_checks.find(row => row.check_id === phase + '-' + failAt), { check_id: phase + '-' + failAt, status: 'failed' });
    assert(steps.some(row => row.error === failure), 'The original step must own the original diagnostic failure');
    assert.doesNotMatch(JSON.stringify(report), /private|error|stack/);
    const positions = report.journey_checks.map(row => expected[phase].indexOf(row.check_id.slice(phase.length + 1)));
    assert(positions.every((position, index) => index === 0 || position > positions[index - 1]), 'Later checks cannot replay or reorder a failed gate');
    return;
  }
  assert.deepEqual(steps.filter(row => row.status !== 'passed'), [], 'Unexpected wiring failure: ' + steps.map(row => row.error?.stack).filter(Boolean).join('\n'));
  assert.deepEqual(report.journey_checks, expected[phase].map(suffix => ({ check_id: phase + '-' + suffix, status: 'passed' })));
  assert.doesNotMatch(JSON.stringify(report), /private/);
}

test('actual downstream outer code reaches every added checkpoint once in public-contract order', async () => {
  for (const phase of Object.keys(expected)) await probe(phase);
});

test('each new diagnostic records and rethrows the original failure without exporting it', async () => {
  for (const [phase, suffixes] of Object.entries(expected)) for (const suffix of suffixes) {
    await probe(phase, suffix);
  }
});
