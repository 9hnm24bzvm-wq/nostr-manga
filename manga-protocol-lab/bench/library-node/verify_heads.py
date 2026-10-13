"""Read-only supplemental exact-head verification for a preserved mixed fixture."""
import argparse
import asyncio
import json
import os
from pathlib import Path
from urllib.parse import urlparse
import aiohttp
from mixed import verify_projected_head
from run import Account, Wire

async def main(args):
    summary=json.loads(args.summary.read_text())
    slots=asyncio.Semaphore(2);results=[]
    async with aiohttp.ClientSession() as session:
        wire=Wire(session,base_url=args.base,record_limit=0)
        async def one(row):
            publisher=row.get('publisher') or Account(1000+row['user']).pub
            result=dict(user=row['user'],publisher=publisher,expected_head=row['acknowledged_head'])
            try:
                async with slots: result.update(await verify_projected_head(wire,publisher,row['acknowledged_head']))
                result['verified']=True
            except Exception as error:result.update(verified=False,error=type(error).__name__,detail=str(error)[:250])
            results.append(result)
        await asyncio.gather(*(one(row) for row in summary['final_heads']))
    results.sort(key=lambda x:x['user'])
    output=dict(read_only=True,original_run_not_overwritten=True,verified=sum(row['verified'] for row in results),expected=len(results),heads=results)
    args.output.write_text(json.dumps(output,indent=2));print(json.dumps({k:v for k,v in output.items() if k!='heads'}))
    if output['verified']!=output['expected']:raise SystemExit(2)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--summary',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);parser.add_argument('--base',default='http://127.0.0.1:58200');args=parser.parse_args()
    if urlparse(args.base).hostname not in ('127.0.0.1','localhost','::1'):parser.error('loopback fixture required')
    if args.output.exists():parser.error('supplemental output already exists')
    asyncio.run(main(args))
