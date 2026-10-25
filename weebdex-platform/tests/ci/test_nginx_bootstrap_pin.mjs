import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
const source = fs.readFileSync(new URL('../../../manga-protocol-lab/scripts/bootstrap.sh', import.meta.url), 'utf8');
const block = source.split('# BEGIN NGINX COMPATIBILITY LINK\n')[1]?.split('# END NGINX COMPATIBILITY LINK')[0];
test('official nginx 1.30.6 archive is pinned and checked before extraction/build', () => {
  assert.match(source, /download https:\/\/nginx\.org\/download\/nginx-1\.30\.6\.tar\.gz vendor\/nginx-1\.30\.6\.tar\.gz/);
  const checksum = "echo '3cf46f5ed7504d460125866b7bea99fb357f31d887e2e63b35503310e9b8b6c8  vendor/nginx-1.30.6.tar.gz' | sha256sum -c -";
  assert.ok(source.includes(checksum));
  assert.ok(source.indexOf(checksum) < source.indexOf('tar -xzf vendor/nginx-1.30.6.tar.gz'));
  assert.ok(source.indexOf('tar -xzf vendor/nginx-1.30.6.tar.gz') < source.indexOf('(cd vendor/nginx-1.30.6 && ./configure'));
  assert.match(source, /test "\$\(vendor\/nginx-1\.30\.6\/objs\/nginx -v 2>&1\)" = "nginx version: nginx\/1\.30\.6"/);
  assert.match(source, /test "\$\(vendor\/nginx-1\.28\.0\/objs\/nginx -v 2>&1\)" = "nginx version: nginx\/1\.30\.6"/);
  assert.doesNotMatch(source, /download .*nginx-1\.30\.5/);
});
function fixture(mode) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'nginx-pin-finite-'));
  fs.mkdirSync(path.join(dir, 'vendor'));
  const link = path.join(dir, 'vendor/nginx-1.28.0');
  if (mode === 'legacy' || mode === 'current') fs.symlinkSync(mode === 'legacy' ? 'nginx-1.30.5' : 'nginx-1.30.6', link);
  if (mode === 'foreign') fs.symlinkSync('operator-managed', link);
  if (mode === 'directory') fs.mkdirSync(link);
  const result = spawnSync('bash', ['-euo', 'pipefail', '-c', block], { cwd: dir, encoding: 'utf8', timeout: 2000 });
  assert.equal(result.error, undefined);
  return { link, result };
}
for (const mode of ['absent', 'legacy', 'current']) test(`compatibility link ${mode} resolves to truthful current source`, () => {
  const { link, result } = fixture(mode);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(fs.readlinkSync(link), 'nginx-1.30.6');
});
test('unknown compatibility links and existing directories are never overwritten', () => {
  const foreign = fixture('foreign');
  assert.equal(foreign.result.status, 1);
  assert.equal(fs.readlinkSync(foreign.link), 'operator-managed');
  const directory = fixture('directory');
  assert.equal(directory.result.status, 1);
  assert.ok(fs.statSync(directory.link).isDirectory());
});

test('check-mode refuses an unfamiliar executable path before invocation', () => {
  const verify = source.split('# BEGIN NGINX COMPATIBILITY VERIFY\n')[1]?.split('# END NGINX COMPATIBILITY VERIFY')[0];
  assert.ok(verify);
  const body = source.split('verify_all() {')[1].split('\n}')[0];
  assert.ok(body.indexOf('verify_nginx_compatibility_link') < body.indexOf('$(vendor/nginx-1.28.0/objs/nginx -v'));
  for (const mode of ['foreign-link', 'directory', 'legacy-link']) {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'nginx-check-finite-'));
    const vendor = path.join(dir, 'vendor');
    fs.mkdirSync(vendor);
    const name = mode === 'foreign-link' ? 'operator-managed' : mode === 'legacy-link' ? 'nginx-1.30.5' : 'nginx-1.28.0';
    fs.mkdirSync(path.join(vendor, name, 'objs'), { recursive: true });
    fs.writeFileSync(path.join(vendor, name, 'objs/nginx'), '#!/bin/sh\ntouch invoked-marker\n', { mode: 0o700 });
    if (mode !== 'directory') fs.symlinkSync(name, path.join(vendor, 'nginx-1.28.0'));
    const result = spawnSync('bash', ['-euo', 'pipefail', '-c', verify + '\nverify_nginx_compatibility_link\nvendor/nginx-1.28.0/objs/nginx -v'], { cwd: dir, encoding: 'utf8', timeout: 2000 });
    assert.equal(result.status, 1, mode);
    assert.equal(fs.existsSync(path.join(dir, 'invoked-marker')), false, mode);
  }
});
