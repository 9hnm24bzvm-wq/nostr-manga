from http.server import HTTPServer,BaseHTTPRequestHandler
from pathlib import Path
import json,time,secrets
D=Path(__file__).resolve().parent;nonce=secrets.token_hex(8);seen=[];done=False
html=f'''<!doctype html><meta charset="utf-8"><title>Local browser readiness {nonce}</title><h1>Owned local browser readiness</h1><p>Fixture {nonce}</p><button onclick="fetch('/finish').then(r=>r.text()).then(text=>document.querySelector('[role=status]').textContent=text)">Check browser interaction</button><p role="status">Waiting for browser interaction</p>'''.encode()
class Handler(BaseHTTPRequestHandler):
 def do_GET(self):
  global done
  seen.append({'path':self.path,'userAgent':self.headers.get('User-Agent'),'at':time.time()})
  if self.path=='/finish':body=f'Browser interaction verified {nonce}'.encode();kind='text/plain';done=True
  elif self.path in ['/','/favicon.ico']:body=html if self.path=='/' else b'';kind='text/html'
  else:self.send_error(404);return
  self.send_response(200);self.send_header('Content-Type',kind);self.send_header('Content-Length',str(len(body)));self.send_header('Cache-Control','no-store');self.end_headers();self.wfile.write(body)
 def log_message(self,*args):pass
server=HTTPServer(('127.0.0.1',0),Handler);server.timeout=.5;url=f'http://127.0.0.1:{server.server_address[1]}/';(D/'probe-start.json').write_text(json.dumps({'url':url,'nonce':nonce,'syntheticLocalOnly':True})+'\n');print(json.dumps({'url':url,'nonce':nonce}),flush=True)
start=time.monotonic()
try:
 while not done and time.monotonic()-start<90:server.handle_request()
finally:
 server.server_close();(D/'probe-result.json').write_text(json.dumps({'url':url,'nonce':nonce,'interactionCompleted':done,'requests':seen,'seconds':round(time.monotonic()-start,3),'socketClosed':True},indent=2)+'\n');print('local socket closed',flush=True)
