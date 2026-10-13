"""Synthetic publisher acceptance through NIP42 + EVENT, never a user key."""
import asyncio,json,sys,time
import aiohttp
from src.auth_model import sign_event
async def main():
 origin,path,expected=sys.argv[1:]
 event=json.load(open(path));url=origin.replace('http:','ws:')+'/'
 async with aiohttp.ClientSession(timeout=aiohttp.ClientTimeout(total=10)) as c:
  async with c.ws_connect(url) as ws:
   challenge=await ws.receive_json(timeout=5);assert challenge[0]=='AUTH'
   auth=sign_event((2).to_bytes(32,'big'),22242,int(time.time()),[['relay',url],['challenge',challenge[1]]],'')
   await ws.send_json(['AUTH',auth]);assert (await ws.receive_json(timeout=5))[2] is True
   await ws.send_json(['EVENT',event]);answer=await ws.receive_json(timeout=5)
   assert answer[0]=='OK' and answer[1]==event['id'] and answer[2] is (expected=='true'),answer
   print(json.dumps({'accepted':answer[2],'event':event['id']}))
asyncio.run(main())
