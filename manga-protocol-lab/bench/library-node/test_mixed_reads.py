"""Actual workload read helper over controlled HTTP, with real large-tree bytes."""
import asyncio
import base64
import unittest
from mixed import read_library_snapshot
from run import Account, Library, Wire, sha

class LibraryReadHttp(unittest.IsolatedAsyncioTestCase):
    async def test_large_and_extreme_selective_restore_rejects_corrupt_200(self):
        import aiohttp
        from aiohttp import web
        import socket
        libraries=await asyncio.to_thread(lambda:[Library(30000,Account(301)),Library(500000,Account(302))])
        heads={library.account.pub:library.account.pointer(library.root) for library in libraries}
        blobs={sha(body).hex():body for library in libraries for body in [library.root]+[b for family in library.blobs.values() for b in family if b]}
        requests=[];corrupt=[False]
        async def head(request):
            requests.append(request.path)
            return web.json_response({'event':heads[request.match_info['publisher']]})
        async def blob(request):
            requests.append(request.path)
            return web.Response(body=blobs[request.match_info['hash']])
        async def read(request):
            value=await request.json();requests.append(value['hashes'])
            return web.json_response({'objects':[{'data':base64.b64encode(blobs[h]+(b'!' if corrupt[0] else b'')).decode()} for h in value['hashes']]})
        app=web.Application();app.router.add_get('/libraries/{publisher}',head);app.router.add_get('/library/blobs/{hash}',blob);app.router.add_post('/library/read',read)
        runner=web.AppRunner(app);await runner.setup()
        sock=socket.socket();sock.bind(('127.0.0.1',0));sock.listen(16);port=sock.getsockname()[1]
        await web.SockSite(runner,sock).start()
        try:
            async with aiohttp.ClientSession() as session:
                wire=Wire(session,base_url=f'http://127.0.0.1:{port}',record_limit=0)
                for library in libraries:
                    self.assertEqual(await read_library_snapshot(wire,library.account.pub,0),200)
                    self.assertEqual(len(requests[-1]),3)
                self.assertEqual(len(requests),6)
                corrupt[0]=True
                with self.assertRaisesRegex(RuntimeError,'chunk mismatch'):
                    await read_library_snapshot(wire,libraries[1].account.pub,1)
        finally:await runner.cleanup()

if __name__=='__main__':unittest.main()
