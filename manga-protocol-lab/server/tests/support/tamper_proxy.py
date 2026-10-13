"""Loopback-only synthetic transport adversary; not a production reverse proxy."""
import asyncio,sys
from pathlib import Path
from aiohttp import web,ClientSession,ClientTimeout
port,target,mode_path=int(sys.argv[1]),int(sys.argv[2]),Path(sys.argv[3])
async def serve():
 cache={}
 async with ClientSession(auto_decompress=False,trust_env=False,timeout=ClientTimeout(total=15)) as client:
  async def forward(request):
   body=await request.read()
   headers={k:v for k,v in request.headers.items() if k.lower() not in ('host','content-length','connection','transfer-encoding')}
   try:
    async with client.request(request.method,f'http://127.0.0.1:{target}'+str(request.rel_url),headers=headers,data=body) as response:
     data=await response.read();status=response.status
     headers={k.lower():v for k,v in response.headers.items() if k.lower() not in ('content-length','connection','transfer-encoding')}
   except Exception:return web.Response(status=502,text='synthetic backend unavailable')
   signed=request.path=='/reconcile' or request.path.startswith('/federation/') and '/blobs/' not in request.path
   if signed:
    mode=mode_path.read_text().strip()
    proof=headers.get('x-manga-response-proof')
    if mode=='replay' and request.path in cache:headers['x-manga-response-proof']=cache[request.path]
    elif proof:cache[request.path]=proof
    if mode=='tamper':data+=b' '
     # Valid JSON and unchanged signed Nostr event semantics; only wire authenticity changes.
    if mode=='strip':headers.pop('x-manga-response-proof',None)
   if mode_path.read_text().strip()=='acktamper' and request.path=='/admin/check':data+=b' '
   return web.Response(status=status,body=data,headers=headers)
  app=web.Application(client_max_size=2*1024*1024);app.router.add_route('*','/{path:.*}',forward)
  runner=web.AppRunner(app);await runner.setup();await web.TCPSite(runner,'127.0.0.1',port).start()
  try:await asyncio.Event().wait()
  finally:await runner.cleanup()
asyncio.run(serve())
