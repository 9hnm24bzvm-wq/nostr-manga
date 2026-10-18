// Pure fixed-value diagnostics; importing this module launches no process.
import fs from 'node:fs';
export const MESSAGE_LIMIT = 64 * 1024;
export const SIGNATURES = Object.freeze(['sandbox', 'loader', 'timeout', 'premature_exit', 'spawn_error']);
const CODES = new Set(['ENOENT', 'EACCES', 'EPERM', 'ENOEXEC', 'ENOMEM', 'ENOSPC', 'ETIMEDOUT', 'EPIPE', 'ECONNRESET']);
const SPAWN = new Set(['ENOENT', 'EACCES', 'EPERM', 'ENOEXEC', 'ENOMEM', 'ENOSPC']);
const FAMILIES = { ENOENT: 'spawn-not-found', EACCES: 'permission-refused', EPERM: 'permission-refused', ENOEXEC: 'executable-format', ENOMEM: 'resource-unavailable', ENOSPC: 'resource-unavailable', ETIMEDOUT: 'launch-timeout', EPIPE: 'transport-closed', ECONNRESET: 'transport-closed' };
const BANNERS = ['Chromium sandboxing failed!', 'No usable sandbox!', 'Failed to move to new namespace:', 'SUID sandbox helper binary', 'Running as root without --no-sandbox is not supported'];
const read = (object, key) => { try { return { ok: true, value: object?.[key] }; } catch { return { ok: false }; } };
export function emptyError() {
  return { reason: 'none', code: 'none', code_source: 'none', message_state: 'not-observed', signatures: Object.fromEntries(SIGNATURES.map(k => [k, null])) };
}
export function errorFacts(stage, error) {
  if (stage !== 'launching-browser') return emptyError();
  const message = read(error, 'message'), observedCode = read(error, 'code'), name = read(error, 'name');
  const bounded = message.ok && typeof message.value === 'string' && Buffer.byteLength(message.value, 'utf8') <= MESSAGE_LIMIT;
  const text = bounded ? message.value : '';
  let code = observedCode.ok && observedCode.value === undefined ? 'none' : observedCode.ok && CODES.has(observedCode.value) ? observedCode.value : 'unknown';
  let codeSource = code === 'none' ? 'none' : code === 'unknown' ? 'unavailable' : 'property';
  const envelope = bounded ? text.match(/(?:^|\n)(?:browserType\.launch: )?(?:Failed to launch: (?:Error: )?)?spawn(?:Sync)? [^\r\n]{1,4096} (ENOENT|EACCES|EPERM|ENOEXEC|ENOMEM|ENOSPC)(?=$|\n|\r)/) : null;
  const conflict = envelope && CODES.has(code) && envelope[1] !== code;
  if (conflict) { code = 'unknown'; codeSource = 'conflict'; }
  else if (envelope && !CODES.has(code)) { code = envelope[1]; codeSource = 'spawn-envelope'; }
  const signatures = {
    sandbox: bounded ? BANNERS.some(s => text.includes(s)) : null,
    loader: bounded ? text.includes('error while loading shared libraries:') || text.includes('Host system is missing dependencies to run browsers.') : null,
    timeout: (name.ok && name.value === 'TimeoutError') || code === 'ETIMEDOUT' ? true : bounded ? /\bTimeout [0-9]{1,9}ms exceeded\b/.test(text) : null,
    premature_exit: bounded ? text.includes('Failed to launch the browser process.') || text.includes('Target page, context or browser has been closed') : null,
    spawn_error: SPAWN.has(code) || !!envelope ? true : bounded ? false : null,
  };
  const families = new Set();
  if (signatures.sandbox) families.add('sandbox-refused');
  if (signatures.loader) families.add('missing-runtime-library');
  if (signatures.timeout) families.add('launch-timeout');
  if (signatures.premature_exit) families.add('launch-closed-before-ready');
  if (FAMILIES[code]) families.add(FAMILIES[code]);
  const reason = conflict || families.size > 1 ? 'multiple-known-families' : families.size ? [...families][0] : bounded ? 'unknown' : 'unavailable';
  return { reason, code, code_source: codeSource, message_state: bounded ? 'bounded' : 'unavailable', signatures };
}
export function emptyHost() {
  return { executable: 'unavailable', tmp: 'unavailable', userns_clone: 'unavailable', userns_max: 'unavailable', apparmor_userns: 'unavailable', seccomp: 'unavailable' };
}
export function emptyDiagnostic() { return { error: emptyError(), host: emptyHost() }; }
export function readBounded(file, limit, io = fs) {
  const fd = io.openSync(file, io.constants.O_RDONLY | io.constants.O_NOFOLLOW);
  try {
    if (!io.fstatSync(fd).isFile()) throw Error('Nonregular fixed diagnostic input');
    const buffer = Buffer.alloc(limit + 1); let used = 0;
    while (used < buffer.length) { const n = io.readSync(fd, buffer, used, buffer.length - used, null); if (!n) break; used += n; }
    if (used > limit) throw Error('Fixed diagnostic input over budget');
    return new TextDecoder('utf-8', { fatal: true }).decode(buffer.subarray(0, used));
  } finally { io.closeSync(fd); }
}
export function collectHostFacts(executable, tmp, io = fs) {
  const facts = emptyHost();
  try {
    if (io.realpathSync(executable) !== executable || !io.lstatSync(executable).isFile()) facts.executable = 'not-regular';
    else { try { io.accessSync(executable, io.constants.X_OK); facts.executable = 'ready'; } catch { facts.executable = 'not-executable'; } }
  } catch { /* unavailable */ }
  try {
    const info = io.lstatSync(tmp);
    if (io.realpathSync(tmp) !== tmp || !info.isDirectory() || info.uid !== process.getuid()) facts.tmp = 'refused';
    else { try { io.accessSync(tmp, io.constants.W_OK | io.constants.X_OK); facts.tmp = 'ready'; } catch { facts.tmp = 'refused'; } }
  } catch { /* unavailable */ }
  const policy = (file, kind) => {
    try {
      const text = readBounded(file, 64, io).trim();
      if (kind === 'switch') return text === '0' ? 'disabled' : text === '1' ? 'enabled' : 'unavailable';
      return /^(0|[1-9][0-9]{0,18})$/.test(text) ? BigInt(text) === 0n ? 'zero' : 'positive' : 'unavailable';
    } catch { return 'unavailable'; }
  };
  facts.userns_clone = policy('/proc/sys/kernel/unprivileged_userns_clone', 'switch');
  facts.userns_max = policy('/proc/sys/user/max_user_namespaces', 'count');
  facts.apparmor_userns = policy('/proc/sys/kernel/apparmor_restrict_unprivileged_userns', 'switch');
  try {
    const matches = [...readBounded('/proc/self/status', 16384, io).matchAll(/^Seccomp:[\t ]*([012])[\t ]*\r?$/gm)];
    if (matches.length === 1) facts.seccomp = ['none', 'strict', 'filter'][Number(matches[0][1])];
  } catch { /* unavailable */ }
  return facts;
}
