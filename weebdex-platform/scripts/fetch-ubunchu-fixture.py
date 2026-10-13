#!/usr/bin/env python3
"""Fetch exact licensed data, not repository software. No runtime/test is started."""
from pathlib import Path
from urllib.request import Request, build_opener, HTTPRedirectHandler
from urllib.parse import urlsplit
import argparse, hashlib, json, time
ROOT = Path(__file__).resolve().parents[1]
class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise RuntimeError('Unexpected dataset redirect; review source before retrying')
def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--acknowledge-noncommercial',action='store_true',help='Use only for a genuinely noncommercial local fixture and preserve attribution/notices')
    args=parser.parse_args()
    if not args.acknowledge_noncommercial: parser.error('Review docs/ubunchu-fixture.md and acknowledge its noncommercial boundary')
    manifest=json.loads((ROOT/'tests/fixtures/ubunchu-manifest.json').read_text())
    limits=manifest['download_limits']; files=manifest['files']
    if len(files)>limits['files']: raise RuntimeError('Manifest file bound')
    destination=ROOT/'.cache/fixtures/ubunchu'/manifest['revision']
    if destination.is_symlink(): raise RuntimeError('Fixture cache cannot be a symlink')
    destination.mkdir(parents=True,exist_ok=True); destination=destination.resolve()
    total=0;deadline=time.monotonic()+limits['total_seconds'];opener=build_opener(NoRedirect())
    for item in files:
        relative=Path(item['path']);url=item['source_url']; parsed=urlsplit(url)
        if relative.is_absolute() or '..' in relative.parts or parsed.scheme!='https' or parsed.hostname!=limits['allowed_host'] or parsed.username or parsed.password:
            raise RuntimeError('Unsafe fixture source or path')
        target=destination/relative
        target.parent.mkdir(parents=True,exist_ok=True)
        if not target.resolve().is_relative_to(destination): raise RuntimeError('Cache path escaped fixture root')
        maximum=limits['per_notice_bytes'] if item['role']=='license_notice' else limits['per_image_bytes']
        if item['bytes']>maximum or time.monotonic()>deadline: raise RuntimeError('Fixture byte/time limit')
        if target.exists(): body=target.read_bytes() if target.stat().st_size<=maximum else b''
        else:
            with opener.open(Request(url,headers={'User-Agent':'WeebDex-data-fixture/1.0'}),timeout=min(limits['request_seconds'],max(1,deadline-time.monotonic()))) as response:
                body=response.read(maximum+1)
        if len(body)!=item['bytes'] or hashlib.sha256(body).hexdigest()!=item['sha256']: raise RuntimeError('Fixture hash/length mismatch: '+item['path'])
        total+=len(body)
        if total>limits['total_bytes']: raise RuntimeError('Fixture total-byte limit')
        if not target.exists(): target.write_bytes(body)
    print(json.dumps({'verified_files':len(files),'verified_bytes':total,'revision':manifest['revision'],'runtime_started':False,'external_upload':False}))
if __name__=='__main__': main()
