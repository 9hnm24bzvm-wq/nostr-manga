"""Benchmark clients must verify the same service and author identities as clients."""
import base64,json,unittest
from types import SimpleNamespace
from run import Account,Wire,verify_head,BASE,sha,dump
class ClientProofs(unittest.TestCase):
 def test_acknowledgement_is_request_and_body_bound(self):
  wire=Wire(None);request=Account(201).sign(27235,[],'');body=b'{"ok":true}';url=BASE+'/libraries'
  tags=[['protocol','manga-federation-response-v1'],['u',url],['method','POST'],['request',request['id']],['status','200'],['payload',sha(body).hex()]]
  proof=Account(11).sign(20006,tags,'');response=SimpleNamespace(status=200,headers={'X-Manga-Response-Proof':'Nostr '+base64.b64encode(dump(proof)).decode()})
  wire.proofs.verify_response(response,url,'POST',request,body)
  for changed in [body+b' ',b'{"ok":false}']:
   with self.assertRaises(Exception):wire.proofs.verify_response(response,url,'POST',request,changed)
  with self.assertRaises(Exception):wire.proofs.verify_response(response,url,'POST',{'id':'ff'*32},body)
  response.headers={}
  with self.assertRaises(Exception):wire.proofs.verify_response(response,url,'POST',request,body)
 def test_public_head_requires_expected_author_and_signature(self):
  account=Account(201);head=account.sign(11999,[],json.dumps({'manifest':'ab'*32,'version':1}));verify_head(head,account.pub)
  with self.assertRaises(Exception):verify_head(head,Account(202).pub)
  head['content']+=' '
  with self.assertRaises(Exception):verify_head(head,account.pub)
if __name__=='__main__':unittest.main()
