"""Paced public-head observation over real controlled HTTP."""
import asyncio
import os
import socket
import time
import unittest
from mixed import verify_projected_head
from run import Account, Wire

class HeadVerification(unittest.IsolatedAsyncioTestCase):
    async def test_429_is_retried_after_delay_and_never_accepted(self):
        import aiohttp
        from aiohttp import web
        account=Account(600);event=account.sign(11999,[],'{"manifest":"'+'ab'*32+'","version":1}')
        seen=[]
        async def head(request):
            seen.append(time.monotonic())
            if len(seen)==1:return web.json_response({'error':'read_slots_busy'},status=429,headers={'Retry-After':'.05'})
            return web.json_response({'event':event,'projected':len(seen)>=3})
        app=web.Application();app.router.add_get('/libraries/{key}',head);runner=web.AppRunner(app);await runner.setup()
        sock=socket.socket();sock.bind(('127.0.0.1',0));sock.listen(16);port=sock.getsockname()[1];await web.SockSite(runner,sock).start()
        try:
            async with aiohttp.ClientSession() as session:
                wire=Wire(session,base_url=f'http://127.0.0.1:{port}',record_limit=0)
                result=await verify_projected_head(wire,account.pub,event['id'],timeout=2)
                self.assertEqual(result,{'attempts':3,'throttled_responses':1})
                self.assertGreaterEqual(seen[1]-seen[0],.045)
                with self.assertRaisesRegex(RuntimeError,'identity mismatch'):
                    await verify_projected_head(wire,account.pub,'ff'*32,timeout=2)
        finally:await runner.cleanup()

    async def test_permanent_throttle_obeys_overall_deadline(self):
        import aiohttp
        from aiohttp import web
        calls=[]
        async def head(request):
            calls.append(request.path)
            return web.json_response({'error':'read_slots_busy'},status=429,headers={'Retry-After':'100'})
        app=web.Application();app.router.add_get('/libraries/{key}',head);runner=web.AppRunner(app);await runner.setup()
        sock=socket.socket();sock.bind(('127.0.0.1',0));sock.listen(16);port=sock.getsockname()[1];await web.SockSite(runner,sock).start()
        try:
            async with aiohttp.ClientSession() as session:
                with self.assertRaises(asyncio.TimeoutError):await verify_projected_head(Wire(session,base_url=f'http://127.0.0.1:{port}'),Account(600).pub,'ab'*32,timeout=.1)
                self.assertEqual(len(calls),1)
        finally:await runner.cleanup()

if __name__=='__main__':unittest.main()
