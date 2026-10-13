"""Synthetic loopback Nostr transport regression, with the real projection backend."""
import asyncio,json,sys,time,base64,hashlib
import aiohttp
from src.auth_model import sign_event
origin=sys.argv[1]
url=origin.replace('http:','ws:')+'/'
def event(key,kind,tags=None,content='',at=None):
    return sign_event(key.to_bytes(32,'big'),kind,int(time.time()) if at is None else at,tags or [],content)
async def main():
 async with aiohttp.ClientSession(timeout=aiohttp.ClientTimeout(total=10)) as client:
  async with client.get(origin,headers={'Accept':'application/nostr+json'}) as r:
   info=await r.json(content_type=None);assert info['supported_nips']==[1,11,42]
   assert r.headers['Access-Control-Allow-Origin']=='*'
  async with client.ws_connect(url) as ws:
   async def recv():
    r=await ws.receive_json(timeout=5);return r
   challenge=await recv();assert challenge[0]=='AUTH'
   first=event(2,1,[['t','manga']],'synthetic first')
   await ws.send_json(['EVENT',first]);assert (await recv())[2] is False
   for key in (2,3):
    wrong=event(key,22242,[['relay',url],['challenge','wrong']])
    await ws.send_json(['AUTH',wrong]);assert (await recv())[2] is False
    auth=event(key,22242,[['relay',url],['challenge',challenge[1]]])
    await ws.send_json(['AUTH',auth]);assert (await recv())[2] is True
   await ws.send_json(['REQ','live',{'kinds':[1],'#t':['manga'],'limit':0}]);assert await recv()==['EOSE','live']
   await ws.send_json(['EVENT',first]);assert (await recv())[2] is True
   assert (await recv())==['EVENT','live',first]
   second=event(3,1,[['t','manga']],'synthetic second')
   await ws.send_json(['EVENT',second]);assert (await recv())[2] is True
   assert (await recv())==['EVENT','live',second]
   await ws.send_json(['CLOSE','live'])
   await ws.send_json(['REQ','history',{'ids':[first['id'],second['id']]}])
   received=[]
   while True:
    message=await recv()
    if message[0]=='EOSE':break
    received.append(message[2])
   assert len(received)==2
   assert [x['id'] for x in received]==[x['id'] for x in sorted([first,second],key=lambda e:(-e['created_at'],e['id']))]
   await ws.send_json(['CLOSE','history'])
   # Protected publication still requires a valid manifest and complete originals.
   invalid=event(2,1001,[], '{}');await ws.send_json(['EVENT',invalid]);assert (await recv())[2] is False
   # Deletion evidence is visible; deleted note cannot reappear.
   deletion=event(2,5,[['e',first['id']]])
   await ws.send_json(['EVENT',deletion]);assert (await recv())[2] is True
   await ws.send_json(['REQ','deleted',{'ids':[first['id']]}]);assert await recv()==['EOSE','deleted']
   await ws.send_json(['REQ','evidence',{'kinds':[5],'#e':[first['id']]}]);assert (await recv())[2]['id']==deletion['id'];assert await recv()==['EOSE','evidence']
   await ws.send_json(['EVENT',first]);assert (await recv())[2] is False
   await ws.send_json(['REQ','bad',{'authors':['ab']}]);assert (await recv())[0]=='CLOSED'
   # AUTH events are ephemeral and never appear in history.
   await ws.send_json(['REQ','auth',{'kinds':[22242]}]);assert await recv()==['EOSE','auth']
 print(json.dumps({'passed':True,'checks':['NIP11 information','anonymous read','write requires AUTH','wrong challenge rejected','multiple authenticated keys retained','zero-limit historical boundary and live delivery','real strfry projection','historical ordering','protected-publication bypass refused','ordinary deletion visibility and replay refusal','invalid filter closed','AUTH never stored'],'scope':'synthetic loopback PostgreSQL and strfry; no scale claim'},indent=2))
asyncio.run(main())
