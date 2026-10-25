import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
const source = fs.readFileSync(new URL('../../../manga-protocol-lab/src/serving_supervisor.py', import.meta.url), 'utf8');
test('all nginx HTTP temp paths are explicitly private-runtime scoped', () => {
 for (const [directive, name] of [['client_body_temp_path','client_temp'],['proxy_temp_path','proxy_temp'],['fastcgi_temp_path','fastcgi_temp'],['uwsgi_temp_path','uwsgi_temp'],['scgi_temp_path','scgi_temp']]) {
  assert.ok(source.includes(`${directive} {self.runtime}/${name};`), directive);
 }
 assert.match(source, /for name in \("client_temp", "proxy_temp", "fastcgi_temp", "uwsgi_temp", "scgi_temp"\):\s*directory\(self\.runtime \/ name\)/);
 assert.equal((source.match(/_temp_path \{self.runtime\}\//g) || []).length, 5);
});
test('nginx startup error logging is private before configuration parsing', () => {
 assert.match(source, /str\(self\.binary\),\s*"-e",\s*str\(self\.runtime \/ "nginx\.log"\),\s*"-p",/);
 assert.ok(source.includes('error_log {self.runtime}/nginx.log notice;'));
});
