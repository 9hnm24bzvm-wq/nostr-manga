"""Private instance read-policy and revocation regression."""
import asyncio,base64,json,sys,time,secrets,hashlib
import aiohttp
from src.auth_model import sign_event
origin,policy=sys.argv[1:]
url=origin.replace('http:','ws:')+'/'
def event(key,kind,tags=None,content=''):
 return sign_event(key.to_bytes(32,'big'),kind,int(time.time()),tags or [],content)
def token(key,path,method='GET',body=b''):
 e=event(key,27235,[['u',origin+path],['method',method],['payload',hashlib.sha256(body).hexdigest()],['nonce',secrets.token_hex(16)]])
 return 'Nostr '+base64.b64encode(json.dumps(e,separators=(',',':')).encode()).decode()
async def main():
 async with aiohttp.ClientSession(timeout=aiohttp.ClientTimeout(total=10)) as c:
  missing='ab'*32
  routes=['/events/'+missing,'/originals/'+missing,'/query?order=asc','/social?target='+missing,'/libraries/'+missing,'/library/blobs/'+missing,'/accounts/'+missing+'/metadata/0','/account-deletions/'+missing]
  for path in routes:
   async with c.get(origin+path) as r: assert r.status==403,(path,r.status)
   async with c.get(origin+path,headers={'Authorization':token(2,path)}) as r: assert r.status!=403,(path,r.status,await r.text())
  path='/query?order=asc'
  h={'Authorization':token(2,path)}
  async with c.get(origin+path,headers=h) as r:assert r.status==200
  async with c.get(origin+path,headers=h) as r:assert r.status==403,'replayed NIP98 accepted'
  async with c.get(origin+path,headers={'Authorization':token(4,path)}) as r:assert r.status==(200 if policy=='authenticated' else 403)
  body=b'{"hashes":[]}'
  h={'Authorization':token(2,'/library/read','POST',body)}
  async with c.post(origin+'/library/read',data=body+b' ',headers=h) as r:assert r.status==403,'tampered read POST accepted'
  async with c.get(origin,headers={'Accept':'application/nostr+json'}) as r:assert (await r.json(content_type=None))['limitation']['auth_required']
  async with c.ws_connect(url) as ws:
   challenge=(await ws.receive_json(timeout=5))[1]
   await ws.send_json(['REQ','private',{'limit':0}]);assert (await ws.receive_json(timeout=5))[2].startswith('auth-required:')
   await ws.send_json(['AUTH',event(4,22242,[['relay',url],['challenge',challenge]])]);assert (await ws.receive_json(timeout=5))[2]
   await ws.send_json(['REQ','unadmitted',{'limit':0}]);r=await ws.receive_json(timeout=5)
   assert r[0]==('EOSE' if policy=='authenticated' else 'CLOSED')
   await ws.send_json(['CLOSE','unadmitted'])
   await ws.send_json(['AUTH',event(2,22242,[['relay',url],['challenge',challenge]])]);assert (await ws.receive_json(timeout=5))[2]
   await ws.send_json(['REQ','admitted',{'limit':0}]);assert await ws.receive_json(timeout=5)==['EOSE','admitted']
   if policy=='admitted':
    key=event(2,1)['pubkey'];path='/admin/accounts/'+key;body=b'{"allowed":false}'
    async with c.post(origin+path,data=body,headers={'Authorization':token(1,path,'POST',body)}) as r:assert r.status==200
    closed=await ws.receive_json(timeout=5);assert closed[0:2]==['CLOSED','admitted'],closed
    note=event(2,1,[], 'revoked author')
    await ws.send_json(['EVENT',note]);assert (await ws.receive_json(timeout=5))[2] is False
 print(json.dumps({'passed':True,'policy':policy,'checks':['all public HTTP routes gated','valid reader allowed','NIP98 replay refused','POST body binding','NIP11 auth flag','WS read authentication','admitted reader permission','revoked reader subscription closed' if policy=='admitted' else 'unadmitted authenticated reader permitted']},indent=2))
asyncio.run(main())
