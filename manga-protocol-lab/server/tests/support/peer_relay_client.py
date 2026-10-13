"""Actual role-separated NIP42 peer transport. Only synthetic loopback keys."""
import asyncio,base64,hashlib,json,secrets,sys,time
import aiohttp
from src.auth_model import sign_event
origin=sys.argv[1]
reader_url=origin.replace('http:','ws:')+'/'
peer_url=origin.replace('http:','ws:')+'/federation/relay'
def event(key,kind,tags=None,content=''):
 return sign_event(key.to_bytes(32,'big'),kind,int(time.time()),tags or [],content)
def token(key,path,method='POST',body=b''):
 e=event(key,27235,[['u',origin+path],['method',method],['payload',hashlib.sha256(body).hexdigest()],['nonce',secrets.token_hex(16)]])
 return 'Nostr '+base64.b64encode(json.dumps(e,separators=(',',':')).encode()).decode()
async def post(c,path,value):
 body=json.dumps(value,separators=(',',':')).encode()
 async with c.post(origin+path,data=body,headers={'Authorization':token(1,path,body=body)}) as r:
  assert r.status==200,(path,r.status,await r.text())
async def auth(ws,key,challenge,url):
 await ws.send_json(['AUTH',event(key,22242,[['relay',url],['challenge',challenge]])])
 response=await ws.receive_json(timeout=5)
 assert response[0]=='OK',response
 return response[2]
async def main():
 async with aiohttp.ClientSession(timeout=aiohttp.ClientTimeout(total=30)) as c:
  async with c.get(origin) as r:
   cap=await r.json();assert cap['manga']['peer_websocket']['read_only'];assert cap['manga']['peer_websocket']['url']==peer_url
  async with c.get(origin+'/federation/relay') as r:assert r.status==426
  await post(c,'/admin/accounts/'+event(2,1)['pubkey'],{'allowed':True})
  peer_key=event(12,1)['pubkey']
  await post(c,'/admin/peers',{'key':peer_key,'endpoint':origin,'allowed':True})
  async with c.ws_connect(reader_url) as reader:
   challenge=(await reader.receive_json(timeout=5))[1]
   assert await auth(reader,12,challenge,reader_url)
   await reader.send_json(['REQ','peer-is-not-reader',{'limit':0}])
   assert (await reader.receive_json(timeout=5))[0]=='CLOSED'
   assert await auth(reader,2,challenge,reader_url)
   note=event(2,1,[],'peer transport fixture')
   await reader.send_json(['EVENT',note]);assert (await reader.receive_json(timeout=5))[2]
   for _ in range(100):
    path='/events/'+note['id']
    async with c.get(origin+path,headers={'Authorization':token(2,path,'GET')}) as r:
     if r.status==200:break
    await asyncio.sleep(.03)
   else:raise AssertionError('projection did not complete')
   async with c.ws_connect(peer_url) as peer:
    challenge=(await peer.receive_json(timeout=5))[1]
    await peer.send_json(['REQ','anonymous',{'limit':0}]);assert (await peer.receive_json(timeout=5))[0]=='CLOSED'
    assert not await auth(peer,2,challenge,peer_url),'admitted personal key is not an approved peer'
    assert not await auth(peer,12,challenge,reader_url),'AUTH must bind the peer endpoint'
    assert not await auth(peer,12,'wrong',peer_url),'AUTH must bind this connection challenge'
    assert await auth(peer,12,challenge,peer_url)
    await peer.send_json(['REQ','history',{'ids':[note['id']]}])
    history=[]
    while True:
     message=await peer.receive_json(timeout=5)
     if message[0]=='EOSE':break
     assert message[0:2]==['EVENT','history'];history.append(message[2])
    assert history and all(e==note for e in history)
    await peer.send_json(['CLOSE','history'])
    await peer.send_json(['REQ','live',{'kinds':[1],'limit':0}]);assert await peer.receive_json(timeout=5)==['EOSE','live']
    live=event(2,1,[],'new event on peer transport')
    await reader.send_json(['EVENT',live]);assert (await reader.receive_json(timeout=5))[2]
    message=await peer.receive_json(timeout=5);assert message==['EVENT','live',live],message
    # Even a peer that separately receives personal-account admission cannot
    # turn this read-only connection into a publication bypass.
    await post(c,'/admin/accounts/'+peer_key,{'allowed':True})
    attempted=event(12,1,[],'must not be written through peer interface')
    await peer.send_json(['EVENT',attempted]);assert (await peer.receive_json(timeout=5))[2] is False
    await post(c,'/admin/peers',{'key':peer_key,'endpoint':origin,'allowed':False})
    message=await peer.receive_json(timeout=5);assert message[0:2]==['CLOSED','live'],message
    closed=await peer.receive(timeout=5);assert closed.type in (aiohttp.WSMsgType.CLOSE,aiohttp.WSMsgType.CLOSED)
   async with c.ws_connect(peer_url) as denied:
    challenge=(await denied.receive_json(timeout=5))[1]
    assert not await auth(denied,12,challenge,peer_url),'revoked peer must authenticate no further'
   await reader.send_json(['REQ','unwritten',{'ids':[attempted['id']]}]);assert await reader.receive_json(timeout=5)==['EOSE','unwritten']
   await reader.send_json(['REQ','auth-not-stored',{'kinds':[22242]}]);assert await reader.receive_json(timeout=5)==['EOSE','auth-not-stored']
  # Unauthenticated idle sockets must release their finite connection budget.
  async with c.ws_connect(peer_url) as idle:
   assert (await idle.receive_json(timeout=5))[0]=='AUTH'
   closed=await idle.receive(timeout=20)
   assert closed.type in (aiohttp.WSMsgType.CLOSE,aiohttp.WSMsgType.CLOSED),closed
 print(json.dumps({'passed':True,'checks':['separate approved-peer NIP42 read-only endpoint','reader admission grants no peer access','peer approval grants no admitted-reader access','exact endpoint and challenge binding','hosted projected history and live delivery','EVENT cannot bypass publication policy even with personal admission','revocation closes active stream and denies new authentication','AUTH remains ephemeral','unauthenticated idle connection expires'],'scope':'real loopback PostgreSQL/strfry; HTTP remains manifest-aware reconciliation and blob transfer'},indent=2))
asyncio.run(main())
