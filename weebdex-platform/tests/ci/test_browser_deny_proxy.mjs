// Bounded real loopback sockets in short-lived Node children. No browser,
// application, authentication data, database or outbound connection.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const root = fs.mkdtempSync(path.join(os.tmpdir(), 'deny-proxy-seams-'));
const proxyUrl = new URL('../../scripts/browser-deny-proxy.mjs', import.meta.url).href;
const lifecycleUrl = new URL('../../scripts/runner-lifecycle.mjs', import.meta.url).href;
const pythonPeers = String.raw`import socket,struct,sys
peers=[]
try:
 for _ in range(int(sys.argv[2])):
  peer=socket.create_connection(("127.0.0.1",int(sys.argv[1])),timeout=2); peers.append(peer); peer.sendall(b"CONNECT synthetic.example.invalid:443 HTTP/1.1\r\nHost: synthetic.example.invalid:443\r\n\r\n"); assert peer.recv(4096).startswith(b"HTTP/1.1 403 Forbidden")
 print("ready",flush=True); sys.stdin.readline()
finally:
 for peer in peers:
  peer.setsockopt(socket.SOL_SOCKET,socket.SO_LINGER,struct.pack("ii",1,0)); peer.close()
`;
let cases = 0;
function run(name, body, { status = 0, fatalCode, resets } = {}) {
  const output = path.join(root, name + '.json');
  const script = `import fs from 'node:fs';import net from 'node:net';import assert from 'node:assert/strict';
    import {once} from 'node:events';import {spawn} from 'node:child_process';import {createBrowserDenyProxy} from ${JSON.stringify(proxyUrl)};
    import {installRunnerLifecycle} from ${JSON.stringify(lifecycleUrl)};
    const report={proxy_requests_blocked:[]};const save=()=>fs.writeFileSync(${JSON.stringify(output)},JSON.stringify(report));
    const stage=installRunnerLifecycle(report,save);const {proxy,sockets,close}=createBrowserDenyProxy(report,()=>'<synthetic>',save);
    await new Promise(resolve=>proxy.listen(0,'127.0.0.1',resolve));const port=proxy.address().port;const clientCloses=[];stage('running-checks');
    async function separatePeers(count){
      const code=${JSON.stringify(pythonPeers)};
      const peer=spawn('python3',['-c',code,String(port),String(count)],{stdio:['pipe','pipe','pipe']});
      let stderr='';peer.stderr.on('data',data=>stderr+=data);
      const ended=new Promise((resolve,reject)=>{peer.once('error',reject);peer.once('close',code=>code===0?resolve():reject(Error('Synthetic peer failed: '+stderr)));});
      clientCloses.push(ended);await new Promise((resolve,reject)=>{peer.stdout.once('data',data=>String(data).trim()==='ready'?resolve():reject(Error('Synthetic peer readiness invalid')));ended.then(()=>reject(Error('Synthetic peer ended before readiness')),reject);});
      return {reset:()=>peer.stdin.end('reset\\n'),ended};
    }
    async function connection(raw){
      const accepted=once(proxy,'connection');const client=net.connect(port,'127.0.0.1');client.on('error',()=>{});
      clientCloses.push(new Promise(resolve=>client.once('close',resolve)));await once(client,'connect');const [socket]=await accepted;
      const ended=new Promise(resolve=>socket.once('close',resolve));
      const response=new Promise(resolve=>client.once('data',value=>resolve(value.toString())));
      if(raw)client.write(raw);return {client,socket,ended,response};
    }
    const connect='CONNECT synthetic.example.invalid:443 HTTP/1.1\\r\\nHost: synthetic.example.invalid:443\\r\\n\\r\\n';
    ${body}
    stage('closing-proxy');await close();assert.equal(sockets.size,0);assert.equal(proxy.listening,false);await Promise.all(clientCloses);await new Promise(resolve=>setImmediate(resolve));
    const refused=net.connect(port,'127.0.0.1');const refusedClosed=new Promise(resolve=>refused.once('close',resolve));const refusedCode=await new Promise(resolve=>{refused.once('error',error=>resolve(error.code));refused.once('connect',()=>resolve('unexpected-connect'));});refused.destroy();await refusedClosed;await new Promise(resolve=>setImmediate(resolve));
    assert.equal(refusedCode,'ECONNREFUSED');report.passed=true;report.completed_utc='synthetic';stage('complete');process.exitCode=0;`;
  const child = spawnSync(process.execPath, ['--unhandled-rejections=strict', '--input-type=module', '-e', script],
    { encoding: 'utf8', timeout: 4000 });
  assert.equal(child.error, undefined, name + ': child deadline');
  assert.equal(child.signal, null); assert.equal(child.status, status, name + ': unexpected exit ' + child.stderr);
  const report = JSON.parse(fs.readFileSync(output, 'utf8'));
  assert.equal(report.runner_lifecycle.exit_code, status); assert.equal(report.runner_lifecycle.exit_observed, true);
  if (fatalCode) {
    assert.equal(report.runner_lifecycle.fatal_code, fatalCode);
    assert.notEqual(report.runner_lifecycle.fatal_kind, 'none');
    assert.equal(report.completed_utc, undefined);
  } else {
    assert.equal(report.runner_lifecycle.fatal_kind, 'none'); assert.equal(report.runner_lifecycle.fatal_code, 'none');
    assert.equal(report.proxy_lifecycle.closed, true); assert.equal(report.proxy_lifecycle.remaining_socket_count, 0);
    if (resets !== undefined) assert.equal(report.proxy_lifecycle.expected_connect_resets, resets);
  }
  assert.equal(JSON.stringify(report).includes('private-test-value'), false);
  cases++; process.stdout.write('PASS ' + name + '\n');
}
try {
  run('normal HTTP request is denied and all sockets close', `
    const row=await connection('GET http://synthetic.example.invalid/path HTTP/1.1\\r\\nHost: synthetic.example.invalid\\r\\nConnection: close\\r\\n\\r\\n');
    assert.match(await row.response,/^HTTP\\/1.1 403 Forbidden/);row.client.end();await row.ended;`, { resets: 0 });
  run('normal CONNECT is denied without forwarding', `
    const row=await connection(connect);assert.match(await row.response,/^HTTP\\/1.1 403 Forbidden/);row.client.end();await row.ended;
    assert.equal(report.proxy_lifecycle.connect_denials,1);`, { resets: 0 });
  for (let index = 0; index < 10; index++) run('denied CONNECT peer reset ' + index, `
    const row=await connection(connect);assert.match(await row.response,/^HTTP\\/1.1 403 Forbidden/);
    stage('closing-browser');row.client.resetAndDestroy();await row.ended;
    assert.equal(report.proxy_lifecycle.connect_denials,1);`, { resets: 1 });
  run('early TCP reset before CONNECT retains HTTP parser ownership', `
    const row=await connection();row.client.resetAndDestroy();await row.ended;`, { resets: 0 });
  run('reset while denied tunnel is half closed is owned', `
    const row=await connection(connect);await row.response;row.client.resetAndDestroy();await row.ended;`, { resets: 1 });
  run('malformed request is rejected and socket cannot leak', `
    const row=await connection('INVALID HTTP REQUEST\\r\\n\\r\\n');assert.match(await row.response,/^HTTP\\/1.1 400 Bad Request/);
    row.client.end();await row.ended;`, { resets: 0 });
  run('owned shutdown closes idle accepted sockets', `const row=await connection();row.client.on('close',()=>{});`, { resets: 0 });
  // Separate peer processes match Chromium's boundary and avoid coupling the
  // proxy shutdown assertion to Node's outbound-client native teardown.
  run('multiple external peer resets are owned and joined', `
    const peers=await separatePeers(8);const ended=[...sockets].map(socket=>new Promise(resolve=>socket.once('close',resolve)));
    assert.equal(report.proxy_lifecycle.connect_denials,8);stage('closing-browser');peers.reset();
    await peers.ended;await Promise.all(ended);`, {resets:8});
  run('multiple external resets and shutdown race remains joined', `
    const peers=await separatePeers(8);assert.equal(report.proxy_lifecycle.connect_denials,8);
    stage('closing-browser');peers.reset();`);
  run('unexpected denied socket error stays fatal', `
    const row=await connection(connect);await row.response;stage('closing-browser');
    setImmediate(()=>row.socket.emit('error',Object.assign(Error('private-test-value'),{code:'EPIPE'})));
    await new Promise(resolve=>setTimeout(resolve,2000));`, { status: 1, fatalCode: 'EPIPE' });
  run('reset outside owned socket membership stays fatal', `
    const row=await connection(connect);await row.response;sockets.delete(row.socket);stage('closing-browser');
    setImmediate(()=>row.socket.emit('error',Object.assign(Error('private-test-value'),{code:'ECONNRESET'})));
    await new Promise(resolve=>setTimeout(resolve,2000));`, { status: 1, fatalCode: 'ECONNRESET' });
  run('reset on non-CONNECT socket is not suppressed by proxy ownership', `
    const row=await connection();assert.equal(report.proxy_lifecycle.connect_denials,0);
    // Remove only Node's parser-owned handler for this negative EventEmitter
    // seam; the deny-proxy repair must not have installed a replacement.
    row.socket.removeAllListeners('error');stage('closing-browser');
    setImmediate(()=>row.socket.emit('error',Object.assign(Error('private-test-value'),{code:'ECONNRESET'})));
    await new Promise(resolve=>setTimeout(resolve,2000));`, { status: 1, fatalCode: 'ECONNRESET' });
  run('shutdown deadline remains fatal and unresolved', `
    // Pure deliberately stuck socket double; no network connection is leaked.
    sockets.add({once(){},destroy(){}});stage('closing-proxy');await close({timeoutMs:10});`,
    { status: 1, fatalCode: 'unknown' });
  run('unexpected server error stays fatal', `
    stage('closing-browser');setImmediate(()=>proxy.emit('error',Error('private-test-value')));
    await new Promise(resolve=>setTimeout(resolve,2000));`, { status: 1, fatalCode: 'unknown' });
  run('unexpected repeated close is not swallowed', `stage('closing-proxy');await close();await close();`,
    { status: 1, fatalCode: 'ERR_SERVER_NOT_RUNNING' });
  process.stdout.write(JSON.stringify({ status: 'passed', cases,
    boundary: 'Actual Node HTTP/net loopback deny/reset ownership and joined cleanup; no outbound connection, browser, application or database.' }) + '\n');
} finally { fs.rmSync(root, { recursive: true, force: true }); }
