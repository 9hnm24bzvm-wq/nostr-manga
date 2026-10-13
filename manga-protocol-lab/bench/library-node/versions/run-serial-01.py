"""Synthetic actual-node benchmark. No production keys/data. Stream-paced WAN model."""
import asyncio, base64, gzip, hashlib, json, os, statistics, time
from pathlib import Path
import aiohttp, cbor2
from coincurve import PrivateKey, PublicKeyXOnly
BASE='http://127.0.0.1:58200'
OUT=Path(os.environ['NODE_RESULTS'])
def dump(x): return json.dumps(x,separators=(',',':'),ensure_ascii=False).encode()
def sha(b): return hashlib.sha256(b).digest()
def b64(b): return base64.b64encode(b).decode()
class Account:
 def __init__(self,n):
  self.key=PrivateKey.from_int(n); self.pub=PublicKeyXOnly.from_secret(self.key.secret).format().hex(); self.clock=int(time.time())-300
 def sign(self,kind,tags,content,at=None):
  at=int(time.time()) if at is None else at
  digest=sha(dump([0,self.pub,at,kind,tags,content]))
  return dict(id=digest.hex(),pubkey=self.pub,created_at=at,kind=kind,tags=tags,content=content,sig=self.key.sign_schnorr(digest,aux_randomness=bytes(32)).hex())
 def pointer(self,root):
  self.clock+=1
  return self.sign(11999,[],dump(dict(manifest=sha(root).hex(),version=1)).decode(),self.clock)
class Wire:
 def __init__(self,session,wan=False): self.s=session; self.wan=wan; self.records=[]
 async def call(self,method,path,value=None,account=None,compressed=False,raw=False):
  body=b'' if value is None else dump(value); body=gzip.compress(body,compresslevel=3,mtime=0) if compressed else body
  headers={'Accept-Encoding':'gzip','Content-Type':'application/json'}
  if compressed: headers['Content-Encoding']='gzip'
  if account:
   tags=[['u',BASE+path],['method',method],['payload',sha(body).hex()],['nonce',os.urandom(16).hex()]]
   headers['Authorization']='Nostr '+b64(dump(account.sign(27235,tags,'')))
  async def paced():
   for i in range(0,len(body),16384):
    part=body[i:i+16384]; await asyncio.sleep(len(part)/250000); yield part
  started=time.perf_counter()
  if self.wan: await asyncio.sleep(.04)
  data=body
  if self.wan and body: headers['Content-Length']=str(len(body)); data=paced()
  async with self.s.request(method,BASE+path,data=data,headers=headers) as r:
   parts=[]
   async for part in r.content.iter_chunked(16384):
    if self.wan: await asyncio.sleep(len(part)/1250000)
    parts.append(part)
   response=b''.join(parts); wirelen=len(response)
   if r.headers.get('Content-Encoding')=='gzip': response=gzip.decompress(response)
   if self.wan: await asyncio.sleep(.04)
   rec=dict(path=path,status=r.status,elapsed_ms=(time.perf_counter()-started)*1000,upload_bytes=len(body),download_bytes=wirelen)
   self.records.append(rec)
   if r.status not in (200,202): raise RuntimeError(f'{path}: {r.status} {response[:400]!r}')
   return (response if raw else json.loads(response)),rec
class Library:
 def __init__(self,n,account):
  self.account=account; self.head=None; self.rows={}; self.blobs={}; self.desc={}
  sizes={'chapters':64 if n==30000 else 256,'series':8 if n==30000 else 64,'lists':1,'members':4 if n==30000 else 8}
  now=int(time.time()); active=60 if n==30000 else 200; series=1500 if n==30000 else 10000
  for family,count in sizes.items(): self.rows[family]=[[] for _ in range(count)]
  def add(f,r,key): self.rows[f][int.from_bytes(sha(key)[:4],'big')&(sizes[f]-1)].append(r)
  for i in range(n+active):
   key=sha(f'{account.pub}:chapter:{i}'.encode()); add('chapters',[key,i<n,None if i<n else 1,now-86400 if i<n else now],key)
   if i==n: self.active=key
  keys=[sha(f'series:{i}'.encode()) for i in range(series)]
  for i,key in enumerate(keys): add('series',[key,['reading','completed','plan_to_read'][i%3],now-100],key)
  for j in range(2):
   key=sha(f'{account.pub}:list:{j}'.encode())[:16]; add('lists',[key,['Favorites','Queue'][j],now-100],key)
   for i,s in enumerate(keys[j::20]): add('members',[key,s,i,now-100],key+s)
  for family,rows in self.rows.items():
   self.blobs[family]=[None]*len(rows); self.desc[family]=[None]*len(rows)
   for i,records in enumerate(rows):
    records.sort(key=lambda r:r[0]+r[1] if family=='members' else r[0]); self.encode(family,i)
  self.root=self.encode_root()
 def encode(self,f,i):
  records=self.rows[f][i]
  if records:
   blob=cbor2.dumps(dict(type='library-chunk',version=1,family=f,entries=records),canonical=True)
   self.blobs[f][i]=blob; self.desc[f][i]=[sha(blob),len(blob),len(records)]
 def encode_root(self): return cbor2.dumps(dict(type='library-root',version=1,families=[dict(family=f,chunks=self.desc[f]) for f in self.rows]),canonical=True)
 def changed(self):
  i=int.from_bytes(sha(self.active)[:4],'big')&(len(self.rows['chapters'])-1)
  row=next(r for r in self.rows['chapters'][i] if r[0]==self.active); row[2]+=1; row[3]+=1
  self.encode('chapters',i); self.root=self.encode_root()
  return [('chunk',self.blobs['chapters'][i]),('root',self.root)]
 @staticmethod
 def objects(items): return [dict(type=t,data=b64(b)) for t,b in items]
 async def commit(self,wire,items):
  event=self.account.pointer(self.root)
  value=dict(event=event,expected_head=self.head,objects=self.objects(items))
  result,rec=await wire.call('POST','/libraries',value,self.account,True); self.head=event['id']; return rec
 async def initialize(self,wire):
  items=[('chunk',b) for family in self.blobs.values() for b in family if b]+[('root',self.root)]
  group=[]; size=0
  for item in items:
   if len(group)==60 or size+len(item[1])>4_000_000:
    await wire.call('POST','/library/batches',dict(objects=self.objects(group)),self.account,True); group=[];size=0
   group.append(item);size+=len(item[1])
  return await self.commit(wire,group)
 async def projected(self,wire):
  start=time.perf_counter()
  while time.perf_counter()-start<120:
   head,_=await wire.call('GET','/libraries/'+self.account.pub)
   assert head['event']['id']==self.head
   if head['projected']: return (time.perf_counter()-start)*1000
   await asyncio.sleep(.02)
  raise RuntimeError('projection timeout')
 async def restore(self,wire):
  start=time.perf_counter(); head,_=await wire.call('GET','/libraries/'+self.account.pub)
  root_hash=json.loads(head['event']['content'])['manifest']; root,_=await wire.call('GET','/library/blobs/'+root_hash,raw=True)
  assert sha(root).hex()==root_hash
  descriptors=[d for f in cbor2.loads(root)['families'] for d in f['chunks'] if d]
  # Stay below actual 8 MiB response budget, not merely the 64-object count.
  groups=[];group=[];size=0
  for d in descriptors:
   if len(group)==60 or size+d[1]>4_000_000: groups.append(group);group=[];size=0
   group.append(d);size+=d[1]
  if group: groups.append(group)
  records=0
  for group in groups:
   data,_=await wire.call('POST','/library/read',dict(hashes=[d[0].hex() for d in group]))
   for obj,d in zip(data['objects'],group):
    b=base64.b64decode(obj['data']); assert sha(b)==d[0] and len(b)==d[1]; parsed=cbor2.loads(b); assert len(parsed['entries'])==d[2];records+=d[2]
  return dict(elapsed_ms=(time.perf_counter()-start)*1000,objects=len(descriptors)+1,records=records)
 async def selective(self,wire):
  start=time.perf_counter();head,_=await wire.call('GET','/libraries/'+self.account.pub)
  root_hash=json.loads(head['event']['content'])['manifest']; root,_=await wire.call('GET','/library/blobs/'+root_hash,raw=True);assert sha(root).hex()==root_hash
  parsed=cbor2.loads(root); family=parsed['families'][0]; i=int.from_bytes(sha(self.active)[:4],'big')&(len(family['chunks'])-1)
  d=family['chunks'][i]; chunk,_=await wire.call('GET','/library/blobs/'+d[0].hex(),raw=True); assert sha(chunk)==d[0]
  # Populate from downloaded data: do not rely on the warm client cache.
  self.rows['chapters'][i]=cbor2.loads(chunk)['entries']; self.head=head['event']['id']
  await self.commit(wire,self.changed())
  return dict(elapsed_ms=(time.perf_counter()-start)*1000,requests=4)
def stats(values):
 s=sorted(values);return dict(n=len(s),median_ms=statistics.median(s),p95_ms=s[min(len(s)-1,int(len(s)*.95))],max_ms=max(s))
async def main():
 results=[]
 async with aiohttp.ClientSession(auto_decompress=False,timeout=aiohttp.ClientTimeout(total=180)) as session:
  local=Wire(session);admin=Account(1)
  for n,key in [(30000,201),(500000,202)]:
   account=Account(key);await local.call('POST','/admin/accounts/'+account.pub,dict(allowed=True),admin)
   start=time.perf_counter();lib=Library(n,account);prep=time.perf_counter()-start
   await lib.initialize(local);await lib.projected(local)
   for wan in (False,True):
    wire=Wire(session,wan); saves=[];project=[]
    for _ in range(12):
     start=time.perf_counter();rec=await lib.commit(wire,lib.changed());saves.append(rec);project.append(await lib.projected(local)+(time.perf_counter()-start)*1000)
    restore=await lib.restore(wire); selective=await lib.selective(wire);await lib.projected(local)
    row=dict(completed_chapters=n,wan=wan,preparation_s=prep,saves=saves,acceptance=stats([r['elapsed_ms'] for r in saves]),observed_projection=stats(project),restore=restore,selective=selective,requests=wire.records,final_head=lib.head,publisher=account.pub)
    results.append(row);(OUT/'summary.json').write_text(json.dumps(results,indent=2));print(json.dumps({k:v for k,v in row.items() if k not in ('requests','saves')}),flush=True)
asyncio.run(main())
