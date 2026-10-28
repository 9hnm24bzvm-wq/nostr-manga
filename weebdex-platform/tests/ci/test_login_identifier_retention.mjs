import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { createFrontendOverlays } from '../../platform-integrations/frontend-overlays.mjs';
const root = new URL('../../frontend/', import.meta.url).pathname;
const path = root + 'src/routes/(app)/auth/login/+page.svelte';
const original = fs.readFileSync(path, 'utf8');
const transformed = createFrontendOverlays(root).transform.handler(original, path).code;
const branch = transformed.match(/if \(method === "password" && typeof body.identifier === "string"\) \{[\s\S]*?\n\s*flow = data;/)?.[0];
assert(branch, 'retention remains attached to the failed-flow UI branch');
const apply = new Function('method', 'body', 'data', 'let flow; ' + branch + '; return flow;');
const input = (name, value, type = 'text') => ({ attributes: { node_type: 'input', name, type, value } });
test('failed password login preserves only identifier; clears returned password; no persistent storage', () => {
 const result = apply('password', { identifier: 'reader', password: 'private' }, { ui: { nodes: [input('identifier', ''), input('password', 'do-not-retain'), input('csrf_token', 'token', 'hidden')] } });
 assert.equal(result.ui.nodes[0].attributes.value, 'reader');
 assert.equal(result.ui.nodes[1].attributes.value, '');
 assert.equal(result.ui.nodes[2].attributes.value, 'token');
 assert.doesNotMatch(branch, /localStorage|sessionStorage|body.password/);
});
test('server-selected identifier and hidden identifier are not overwritten', () => {
 const result = apply('password', { identifier: 'old-reader' }, { ui: { nodes: [input('identifier', 'server-reader'), input('identifier', '', 'hidden')] } });
 assert.equal(result.ui.nodes[0].attributes.value, 'server-reader'); assert.equal(result.ui.nodes[1].attributes.value, '');
});
test('OIDC response does not inherit password-flow identifier', () => {
 const result = apply('oidc', { identifier: 'old-reader' }, { ui: { nodes: [input('identifier', '')] } });
 assert.equal(result.ui.nodes[0].attributes.value, '');
});
test('group overview retains its name without repeating the raw group ID as a handle', () => {
 const groupPath = root + 'src/routes/(app)/group/[id]/Group.svelte';
 const input = fs.readFileSync(groupPath, 'utf8');
 const output = createFrontendOverlays(root).transform.handler(input, groupPath).code;
 assert.match(output, /<h1[^>]*>\{\$group.name\}<\/h1>/);
 assert.doesNotMatch(output, /@\{\$group.id\}/);
 assert.match(output, /<span class="sr-only">Group<\/span>/);
});
