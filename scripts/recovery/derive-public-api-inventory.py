#!/usr/bin/env python3
"""Derive bounded facts from separately downloaded, hash-pinned upstream YAML.
The upstream YAML is not committed. This derived output copies no descriptions.
"""
import hashlib,json,pathlib,sys,yaml
SOURCE='https://gitlab.com/mangadex-pub/mangadex-api-docs/-/raw/60a3bb3c9f25675435a479c925d857015201739e/static/api.yaml'
SHA='004031ccdfc4b66727dd1f85d09b5300cf60235bbab2de6f6ea4816094bd6d33'
def main():
 raw=pathlib.Path(sys.argv[1]).read_bytes();assert len(raw)==195189;assert hashlib.sha256(raw).hexdigest()==SHA
 doc=yaml.safe_load(raw)
 def resolve(value):
  seen=set()
  while isinstance(value,dict) and '$ref' in value:
   ref=value['$ref'];assert ref.startswith('#/') and ref not in seen;seen.add(ref)
   value=doc
   for key in ref[2:].split('/'):value=value[key]
  return value
 def shape(schema):
  ref=schema.get('$ref') if isinstance(schema,dict) else None
  schema=resolve(schema)
  if not isinstance(schema,dict):return {}
  value={k:schema[k] for k in ['type','format','nullable','enum','minimum','maximum','minLength','maxLength','minItems','maxItems','default'] if k in schema}
  if ref:value['schema_ref']=ref.rsplit('/',1)[-1]
  if 'items'in schema:value['items']=shape(schema['items'])
  if 'properties'in schema:value['properties']=list(schema['properties'])
  if 'required'in schema:value['required']=schema['required']
  return value
 ops=[]
 for path,item in doc['paths'].items():
  inherited=item.get('parameters',[])
  for method,operation in item.items():
   if method not in {'get','post','put','delete','patch','head','options'}:continue
   parameters={}
   for parameter in inherited+operation.get('parameters',[]):
    p=resolve(parameter);parameters[(p['in'],p['name'])]={'name':p['name'],'in':p['in'],'required':p.get('required',False),'schema':shape(p.get('schema',{}))}
   body=resolve(operation.get('requestBody',{}))
   request_schemas={kind:shape(value.get('schema',{})) for kind,value in body.get('content',{}).items()}
   value={'method':method.upper(),'path':path,'operation_id':operation.get('operationId'),'tags':operation.get('tags',[]),'parameters':list(parameters.values()),'request_schemas':request_schemas,'response_codes':list(operation.get('responses',{})),'security':operation.get('security',doc.get('security',[])),'coverage':'pending','evidence':[]}
   if path.startswith('/auth/'):
    value['intentional_adaptation']='Nostr original admitted capability; no MangaDex OAuth. /auth/check must verify the real principal, never no-op.'
   elif path=='/captcha/solve':
    value['intentional_adaptation']='Provider-specific; local abuse-challenge adapter decision pending. Not exempt or implemented.'
   elif path.startswith('/settings'):
    value['intentional_adaptation']='Native preferences through explicit bound Nostr capability; upstream JSON schema semantics still required.'
   elif path=='/user/history':
    value['intentional_adaptation']='Original signed graph has no trustworthy per-read timestamp; do not invent history dates.'
   elif path=='/forums/thread':
    value['intentional_adaptation']='Explicit native forum companion; native thread IDs are separate from upstream numeric forum IDs.'
   ops.append(value)
 result={'contract_version':1,'upstream_api_version':doc['info']['version'],'openapi':doc['openapi'],'source_url':SOURCE,'source_sha256':SHA,'source_bytes':len(raw),'public_base':'/api/v1','path_count':len(doc['paths']),'operation_count':len(ops),'schema_count':len(doc['components']['schemas']),'reconstruction_state':'Reconstructed and unverified; all operation coverage pending until fresh implementation/tests','operations':ops}
 assert (result['operation_count'],result['path_count'],result['schema_count'])==(113,80,102)
 pathlib.Path(sys.argv[2]).write_text(json.dumps(result,indent=2,ensure_ascii=False)+'\n')
 print('derived',len(ops),'operations',len(doc['paths']),'paths',len(doc['components']['schemas']),'schemas')
if __name__=='__main__':main()
