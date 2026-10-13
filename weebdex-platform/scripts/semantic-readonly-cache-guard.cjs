// Preload for local semantic verification. Deny Node filesystem mutations that
// resolve into dependency caches, including writes through package symlinks.
const fs = require('node:fs');
const path = require('node:path');
const { fileURLToPath } = require('node:url');
const { syncBuiltinESMExports } = require('node:module');
const roots = JSON.parse(process.env.WEEBDEX_READONLY_CACHE_ROOTS || '[]').map(root => fs.realpathSync(root));
const original = { realpathSync: fs.realpathSync, existsSync: fs.existsSync, writeFileSync: fs.writeFileSync };
const fdPaths = new Map();
const denied = [];
let checkedMutations = 0;
function actual(value) {
  if (typeof value === 'number') return fdPaths.get(value);
  if (value instanceof URL) value = fileURLToPath(value);
  if (Buffer.isBuffer(value)) value = value.toString();
  if (typeof value !== 'string') return null;
  const full = path.resolve(value); let parent = full;
  while (!original.existsSync(parent)) { const next = path.dirname(parent); if (next === parent) break; parent = next; }
  return path.resolve(original.realpathSync(parent), path.relative(parent, full));
}
function ensure(value, operation) {
  checkedMutations++;
  const resolved = actual(value);
  if (resolved && roots.some(root => resolved === root || resolved.startsWith(root + path.sep))) {
    denied.push({ operation, resolved });
    throw Error(`Read-only dependency cache mutation denied: ${operation} ${resolved}`);
  }
}
function writable(flags) {
  if (typeof flags === 'string') return /[wa+]/.test(flags);
  return Boolean(flags & (fs.constants.O_WRONLY | fs.constants.O_RDWR | fs.constants.O_APPEND | fs.constants.O_CREAT | fs.constants.O_TRUNC));
}
for (const name of ['writeFile', 'appendFile', 'truncate', 'unlink', 'rm', 'rmdir', 'mkdir', 'chmod', 'chown', 'utimes', 'lchmod', 'lchown', 'lutimes', 'createWriteStream']) {
  for (const suffix of ['', 'Sync']) {
    const key = name + suffix, method = fs[key]; if (typeof method !== 'function') continue;
    fs[key] = function (target, ...args) { ensure(target, key); return method.call(this, target, ...args); };
  }
  const method = fs.promises[name];
  if (typeof method === 'function') fs.promises[name] = async function (target, ...args) { ensure(target, 'promises.' + name); return method.call(this, target, ...args); };
}
for (const name of ['rename', 'copyFile', 'cp', 'link', 'symlink']) {
  for (const suffix of ['', 'Sync']) {
    const key = name + suffix, method = fs[key]; if (typeof method !== 'function') continue;
    fs[key] = function (source, target, ...args) { if (name === 'rename') ensure(source, key); ensure(target, key); return method.call(this, source, target, ...args); };
  }
  const method = fs.promises[name];
  if (typeof method === 'function') fs.promises[name] = async function (source, target, ...args) { if (name === 'rename') ensure(source, 'promises.' + name); ensure(target, 'promises.' + name); return method.call(this, source, target, ...args); };
}
const openSync = fs.openSync;
fs.openSync = function (target, flags, ...args) { if (writable(flags)) ensure(target, 'openSync'); const fd = openSync.call(this, target, flags, ...args); fdPaths.set(fd, actual(target)); return fd; };
const open = fs.open;
fs.open = function (target, flags, ...args) {
  if (writable(flags)) ensure(target, 'open');
  const callback = args.pop();
  return open.call(this, target, flags, ...args, (error, fd) => { if (!error) fdPaths.set(fd, actual(target)); callback(error, fd); });
};
const promiseOpen = fs.promises.open;
fs.promises.open = async function (target, flags, ...args) { if (writable(flags)) ensure(target, 'promises.open'); const handle = await promiseOpen.call(this, target, flags, ...args); fdPaths.set(handle.fd, actual(target)); return handle; };
for (const name of ['write', 'writeSync', 'writev', 'writevSync', 'ftruncate', 'ftruncateSync', 'fchmod', 'fchmodSync', 'fchown', 'fchownSync', 'futimes', 'futimesSync']) {
  const method = fs[name]; if (typeof method !== 'function') continue;
  fs[name] = function (fd, ...args) { ensure(fd, name); return method.call(this, fd, ...args); };
}
syncBuiltinESMExports();
process.on('exit', () => {
  const directory = process.env.WEEBDEX_READONLY_AUDIT_DIR;
  if (directory) original.writeFileSync(path.join(directory, `${process.pid}.json`), JSON.stringify({ pid: process.pid, protected_roots: roots, checked_node_mutations: checkedMutations, protected_write_attempts: denied.length, denied }, null, 2) + '\n');
});
