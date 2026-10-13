"""Verify peer NIP42 over a real TLS proxy without disabling certificate checks."""
import asyncio,json,ssl,sys,time
import aiohttp
from src.auth_model import sign_event
origin,ca,internal=sys.argv[1:]
peer=origin.replace('https:','wss:')+'/federation/relay'
internal=internal.replace('http:','ws:')+'/federation/relay'
def auth(challenge,url):
 return sign_event((12).to_bytes(32,'big'),22242,int(time.time()),[['relay',url],['challenge',challenge]],'')
async def main():
 async with aiohttp.ClientSession() as ordinary:
  try:
   await ordinary.ws_connect(peer)
  except (aiohttp.ClientConnectorCertificateError,aiohttp.ClientConnectorSSLError):pass
  else:raise AssertionError('untrusted fixture CA accepted')
 context=ssl.create_default_context(cafile=ca)
 assert context.check_hostname and context.verify_mode==ssl.CERT_REQUIRED
 async with aiohttp.ClientSession(connector=aiohttp.TCPConnector(ssl=context)) as client:
  async with client.ws_connect(peer,headers={'X-Forwarded-Host':'attacker.example','X-Forwarded-Proto':'http'}) as ws:
   challenge=await ws.receive_json(timeout=5);assert challenge[0]=='AUTH'
   for wrong in [internal,'wss://attacker.example/federation/relay']:
    await ws.send_json(['AUTH',auth(challenge[1],wrong)]);assert (await ws.receive_json(timeout=5))[2] is False
   await ws.send_json(['AUTH',auth(challenge[1],peer)]);assert (await ws.receive_json(timeout=5))[2] is True
   await ws.send_json(['REQ','tls-peer',{'limit':0}]);assert await ws.receive_json(timeout=5)==['EOSE','tls-peer']
 print(json.dumps({'passed':True,'checks':['real WSS nginx upgrade','untrusted CA rejected','isolated trusted fixture CA with hostname verification','NIP42 binds configured external WSS endpoint','forwarded headers and internal WS endpoint cannot change signature scope']},indent=2))
asyncio.run(main())
