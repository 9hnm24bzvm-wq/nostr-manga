"""Owned benchmark fixture configuration CLI used by mixed-fixture.sh."""
import json,os,secrets,sys
from pathlib import Path
from coincurve import PrivateKey,PublicKeyXOnly
r=Path(sys.argv[1]);root=Path(sys.argv[2]);admin=PublicKeyXOnly.from_secret(PrivateKey.from_int(1).secret).format().hex()
max_wal_mb=int(os.environ.get('MIXED_PG_MAX_WAL_MB','512'))
if not 64<=max_wal_mb<=16384:raise ValueError('MIXED_PG_MAX_WAL_MB must be an integer between 64 and 16384')
# This exact file is appended by the real launcher after initdb. The omitted
# override preserves the former inline512MB PostgreSQL configuration bytewise.
(r/'postgresql-fixture.conf').write_text("listen_addresses='127.0.0.1'\nport=55441\nunix_socket_directories=''\nshared_buffers='128MB'\nmax_connections=100\nmax_wal_size='%sMB'\n" % max_wal_mb)
def private(path,value):
 fd=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
 with os.fdopen(fd,'w') as f:f.write(value)
private(r/'password',secrets.token_hex(24))
limits=[]
for index in range(2):
 base=r/f'node-{index}';base.mkdir();private(base/'service.key',f'{11+index:064x}')
 config=dict(bind=f'127.0.0.1:{58200+index}',base_url=f'http://127.0.0.1:{58200+index}',service_key_file=str(base/'service.key'),admin_pubkey=admin,database_plaintext_local=True,
  library_reclamation=True,database_connections=int(os.environ.get('NODE_DB_CONNECTIONS','16')),database_wait_ms=2000,cache_bytes=67108864,sync_worker_enabled=True,
  library_concurrency=dict(request_slots=int(os.environ.get('NODE_LIBRARY_REQUESTS','128')),processing_slots=int(os.environ.get('NODE_LIBRARY_WORK','8')),commit_slots=int(os.environ.get('NODE_LIBRARY_COMMITS','4'))),
  storage=dict(path=str(base/'objects'),volume=f'mixed{index}',formats=['image/jpeg','image/png','image/webp'],physical_bytes=int(os.environ.get('MIXED_PHYSICAL_BYTES','3000000000')),staging_bytes=3000000000,max_blob_bytes=int(os.environ.get('MIXED_MAX_BLOB_BYTES','16777216')),upload_slots=int(os.environ.get('NODE_STORAGE_SLOTS','4')),credit_lanes=int(os.environ.get('NODE_CREDIT_LANES','16')),reclamation_slots=int(os.environ.get('NODE_RECLAMATION_SLOTS','4'))),
  backend=dict(binary=os.environ['MIXED_SELECTED_STRFRY'],data_dir=str(base/'backend'),map_bytes=1073741824))
 if 'NODE_READ_CATALOGUE_ACTIVE' in os.environ or 'NODE_READ_CATALOGUE_WAITING' in os.environ:
  overall=min(4,config['database_connections']);catalogue=int(os.environ.get('NODE_READ_CATALOGUE_ACTIVE',str(min(2,overall))))
  waiting=int(os.environ.get('NODE_READ_CATALOGUE_WAITING','8'))
  if not 1<=catalogue<=overall:raise ValueError('NODE_READ_CATALOGUE_ACTIVE must be positive and fit the default overall read limit')
  if not 0<=waiting<=1024:raise ValueError('NODE_READ_CATALOGUE_WAITING must be an integer between 0 and 1024')
  config['read_admission']=dict(overall=overall,wait_ms=200,catalogue=dict(active=catalogue,waiting=waiting),library_head=dict(active=min(2,overall),waiting=8),live=dict(active=min(2,overall),waiting=8))
 if index==0:config['public_serving']=dict(enabled=True,helper=str(root/'src/serving_supervisor.py'),nginx=os.environ['MIXED_SELECTED_NGINX'],bind='127.0.0.1:58202',workers=int(os.environ.get('MIXED_NGINX_WORKERS','2')),shutdown_seconds=2)
 (base/'node.json').write_text(json.dumps(config))
 limits.append({k:v for k,v in config.items() if k not in ('service_key_file','admin_pubkey')})
(Path(os.environ['NODE_RESULTS'])/'node-limits.json').write_text(json.dumps(limits,indent=2))
