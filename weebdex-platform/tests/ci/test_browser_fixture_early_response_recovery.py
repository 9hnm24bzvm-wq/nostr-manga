"""AST-only fake-I/O seams. Never import/start the fixture or open a socket."""
import ast
import hashlib
import json
from http.client import RemoteDisconnected
from pathlib import Path
from types import SimpleNamespace
from urllib.parse import urlsplit
import re
import threading
import unittest

SOURCE=Path(__file__).resolve().parents[1]/'runtime/browser_fixture.py'
TREE=ast.parse(SOURCE.read_text())
HELPER=next(n for n in TREE.body if isinstance(n,ast.FunctionDef)and n.name=='read_oversized_json_response')
FIXTURE=next(n for n in TREE.body if isinstance(n,ast.ClassDef)and n.name=='Fixture')
OPERATIONS=next(n for n in FIXTURE.body if isinstance(n,ast.FunctionDef)and n.name=='operational_checks')
CHECK=next(n for n in OPERATIONS.body if isinstance(n,ast.FunctionDef)and n.name=='check')
BODY=b'x'*(1024*1024+1)


class Clock:
    now=0
    def monotonic(self):return self.now


class Socket:
    def __init__(self):self.closed=False;self.timeouts=[];self.shutdowns=[]
    def settimeout(self,value):
        if self.closed:raise OSError('closed socket')
        self.timeouts.append(value)
    def shutdown(self,how):self.shutdowns.append(how)


class Watchdog:
    def __init__(self,seconds,callback):
        self.seconds=seconds;self.callback=callback;self.ident=None;self.alive=False;self.joins=[];self.sticky=False;self.start_error=None
    def start(self):
        if self.start_error:raise self.start_error
        self.ident=1;self.alive=True
    def cancel(self):
        if not self.sticky:self.alive=False
    def join(self,timeout):
        self.joins.append(timeout)
        if not self.sticky:self.alive=False
    def is_alive(self):return self.alive
    def fire(self):self.callback()


class Response:
    def __init__(self,body=b'{"error":"too large"}',status=413,declare_length=True):
        self.body=body;self.status=status;self.reads=[];self.close_count=0
        self.read_error=None;self.read_hook=lambda:None;self.close_error=None;self.socket=None
        self.length=len(body)if declare_length else None;self.declared=self.length
    def isclosed(self):return not self.body
    def read1(self,size):
        self.reads.append(size);self.read_hook()
        if self.read_error:raise self.read_error
        chunk,self.body=self.body[:size],self.body[size:]
        if self.length is not None:self.length-=len(chunk)
        if not self.body and self.socket:self.socket.closed=True
        return chunk
    def getheaders(self):return [('Content-Type','application/json'),('Cache-Control','no-store')]+([('Content-Length',str(self.declared))]if self.declared is not None else[])
    def close(self):
        self.close_count+=1
        if self.close_error:raise self.close_error


class Connection:
    def __init__(self,response=None):
        self.sock=Socket();self.response=response or Response();self.response.socket=self.sock
        self.connect_count=0;self.close_count=0;self.sent=[];self.headers=[];self.request=None
        self.send_error=None;self.header_error=None;self.response_error=None;self.close_error=None
        self.connect_hook=lambda:None;self.send_hook=lambda:None;self.response_hook=lambda:None
    def connect(self):self.connect_count+=1;self.connect_hook()
    def putrequest(self,*args):self.request=args
    def putheader(self,*args):self.headers.append(args)
    def endheaders(self):
        if self.header_error:raise self.header_error
    def send(self,data):
        self.send_hook()
        if self.send_error:raise self.send_error
        self.sent.append(data)
    def getresponse(self):
        self.response_hook()
        if self.response_error:raise self.response_error
        return self.response
    def close(self):
        self.close_count+=1
        if self.close_error:raise self.close_error


def harness(connection=None,clock=None):
    connection=connection or Connection();clock=clock or Clock();calls=[];watchdogs=[]
    def factory(*args,**kwargs):calls.append((args,kwargs));return connection
    def timer(seconds,callback):
        value=Watchdog(seconds,callback);watchdogs.append(value);return value
    connection.watchdogs=watchdogs
    namespace={'urlsplit':urlsplit,'time':clock,'HTTPConnection':factory,'re':re,
               'threading':SimpleNamespace(Lock=threading.Lock,Event=threading.Event,Timer=timer),
               'socket':SimpleNamespace(SHUT_RDWR=2)}
    exec(compile(ast.Module(body=[HELPER],type_ignores=[]),str(SOURCE),'exec'),namespace)
    return namespace['read_oversized_json_response'],connection,clock,calls


class EarlyResponseTests(unittest.TestCase):
    def test_full_body_retains_actual_response_and_one_connection(self):
        read,c,clock,calls=harness();observation={}
        status,body,headers=read('http://127.0.0.1:12345',BODY,observation)
        self.assertEqual((status,body,headers['content-type']),(413,b'{"error":"too large"}','application/json'))
        self.assertEqual(len(calls),1);self.assertEqual(c.connect_count,1)
        self.assertEqual(c.request,('POST','/api/manga'))
        self.assertIn(('Content-Length',str(len(BODY))),c.headers)
        self.assertEqual(b''.join(c.sent),BODY)
        self.assertEqual(observation['completed_body_send_bytes'],len(BODY))
        self.assertTrue(observation['response_body_complete'])
        self.assertEqual((c.close_count,c.response.close_count),(1,1))
        self.assertTrue(observation['deadline_thread_settled']);self.assertFalse(c.watchdogs[0].alive)
        self.assertEqual(c.watchdogs[0].joins,[1])

    def test_broken_pipe_can_only_retain_an_actual_response_on_same_connection(self):
        read,c,clock,calls=harness();c.send_error=BrokenPipeError();observation={}
        self.assertEqual(read('http://127.0.0.1:12345',BODY,observation)[0],413)
        self.assertEqual(observation['write_error_class'],'BrokenPipeError')
        self.assertEqual(observation['completed_body_send_bytes'],0)
        self.assertEqual(observation['observed_status'],413)
        self.assertEqual(len(calls),1);self.assertEqual(c.connect_count,1)

    def test_header_send_reset_still_requires_actual_response(self):
        read,c,clock,calls=harness();c.header_error=ConnectionResetError();observation={}
        self.assertEqual(read('http://127.0.0.1:12345',BODY,observation)[0],413)
        self.assertEqual(observation['write_error_class'],'ConnectionResetError')
        self.assertEqual(c.sent,[]);self.assertEqual(len(calls),1)

    def test_missing_response_is_failure_never_synthetic_413(self):
        read,c,clock,calls=harness();c.send_error=BrokenPipeError();failure=RemoteDisconnected();c.response_error=failure;observation={}
        with self.assertRaises(RemoteDisconnected)as caught:read('http://127.0.0.1:12345',BODY,observation)
        self.assertIs(caught.exception,failure)
        self.assertIsNone(observation['observed_status']);self.assertFalse(observation['response_body_complete'])
        self.assertEqual(observation['write_error_class'],'BrokenPipeError')
        self.assertEqual((len(calls),c.close_count),(1,1))

    def test_actual_wrong_status_stays_wrong(self):
        read,c,clock,calls=harness(Connection(Response(status=401)));c.send_error=BrokenPipeError();observation={}
        self.assertEqual(read('http://127.0.0.1:12345',BODY,observation)[0],401)
        self.assertEqual(observation['observed_status'],401)

    def test_other_send_errors_are_fatal_and_do_not_read_response(self):
        read,c,clock,calls=harness();failure=TimeoutError();c.send_error=failure;observation={}
        with self.assertRaises(TimeoutError)as caught:read('http://127.0.0.1:12345',BODY,observation)
        self.assertIs(caught.exception,failure);self.assertIsNone(observation['observed_status'])

    def test_response_body_failure_preserves_observed_status_without_acceptance(self):
        read,c,clock,calls=harness();failure=OSError();c.response.read_error=failure;observation={}
        with self.assertRaises(OSError)as caught:read('http://127.0.0.1:12345',BODY,observation)
        self.assertIs(caught.exception,failure);self.assertEqual(observation['observed_status'],413)
        self.assertFalse(observation['response_body_complete']);self.assertEqual(c.response.close_count,1)

    def test_bounded_response_reads_and_oversize_refusal(self):
        read,c,clock,calls=harness(Connection(Response(body=b'x'*(1024*1024+1),declare_length=False)));observation={}
        with self.assertRaisesRegex(RuntimeError,'response budget'):read('http://127.0.0.1:12345',BODY,observation)
        self.assertLessEqual(max(c.response.reads),65536);self.assertFalse(observation['response_body_complete'])
        self.assertEqual((c.close_count,c.response.close_count),(1,1))

    def test_original_deadline_rejects_late_connect_headers_send_and_last_body(self):
        for where in ['connect','response','send','last-body']:
            with self.subTest(where=where):
                read,c,clock,calls=harness();observation={}
                def expire():clock.now=6
                if where=='last-body':c.response.read_hook=expire
                else:setattr(c,where+'_hook',expire)
                with self.assertRaises(TimeoutError):read('http://127.0.0.1:12345',BODY,observation)
                self.assertFalse(observation['response_body_complete']);self.assertEqual(c.close_count,1)

    def test_absolute_watchdog_interrupts_original_socket_during_header_read(self):
        read,c,clock,calls=harness();observation={};failure=OSError('interrupted header read')
        def blocked_headers():
            c.watchdogs[0].fire()
            self.assertEqual(c.sock.shutdowns,[2])
            raise failure
        c.response_hook=blocked_headers
        with self.assertRaises(OSError)as caught:read('http://127.0.0.1:12345',BODY,observation)
        self.assertIs(caught.exception,failure);self.assertTrue(observation['deadline_expired'])
        self.assertTrue(observation['deadline_thread_settled']);self.assertFalse(observation['response_body_complete'])

    def test_unsettled_watchdog_cannot_pass_even_with_actual_response(self):
        read,c,clock,calls=harness();observation={}
        c.connect_hook=lambda:setattr(c.watchdogs[0],'sticky',True)
        with self.assertRaisesRegex(RuntimeError,'deadline_thread_unsettled'):read('http://127.0.0.1:12345',BODY,observation)
        self.assertFalse(observation['deadline_thread_settled']);self.assertFalse(observation['response_body_complete'])

    def test_watchdog_start_failure_still_closes_connection(self):
        read,c,clock,calls=harness();observation={}
        # Replace only the bounded fake timer factory before extracting the helper.
        namespace={'urlsplit':urlsplit,'time':clock,'HTTPConnection':lambda *a,**k:c,'re':re,'socket':SimpleNamespace(SHUT_RDWR=2)}
        timer=Watchdog(5,lambda:None);timer.start_error=RuntimeError('start failed')
        namespace['threading']=SimpleNamespace(Lock=threading.Lock,Event=threading.Event,Timer=lambda *a:timer)
        exec(compile(ast.Module(body=[HELPER],type_ignores=[]),str(SOURCE),'exec'),namespace)
        with self.assertRaisesRegex(RuntimeError,'start failed'):namespace['read_oversized_json_response']('http://127.0.0.1:12345',BODY,observation)
        self.assertEqual(c.close_count,1);self.assertTrue(observation['deadline_thread_settled'])

    def test_truncated_content_length_is_not_a_complete_response(self):
        response=Response();response.length+=5;response.declared=response.length
        read,c,clock,calls=harness(Connection(response));observation={}
        with self.assertRaisesRegex(RuntimeError,'Incomplete'):read('http://127.0.0.1:12345',BODY,observation)
        self.assertEqual(observation['observed_status'],413);self.assertFalse(observation['response_body_complete'])

    def test_invalid_or_duplicate_content_length_is_rejected_before_body_read(self):
        for lengths in [['7','7'],['01'],['-1'],[str(1024*1024+1)]]:
            with self.subTest(lengths=lengths):
                read,c,clock,calls=harness();observation={}
                c.response.getheaders=lambda:[('Content-Length',value)for value in lengths]
                with self.assertRaisesRegex(RuntimeError,'framing'):read('http://127.0.0.1:12345',BODY,observation)
                self.assertEqual(c.response.reads,[]);self.assertFalse(observation['response_body_complete'])

    def test_deadline_is_not_reset_and_socket_timeout_only_uses_remaining_budget(self):
        read,c,clock,calls=harness();c.connect_hook=lambda:setattr(clock,'now',1);c.send_hook=lambda:setattr(clock,'now',2);observation={}
        read('http://127.0.0.1:12345',BODY,observation)
        self.assertTrue(all(0<value<=4 for value in c.sock.timeouts))
        self.assertEqual(c.sock.timeouts[-1],3)

    def test_closed_response_socket_is_not_reconfigured_after_final_read(self):
        read,c,clock,calls=harness();observation={}
        read('http://127.0.0.1:12345',BODY,observation)
        self.assertTrue(c.sock.closed);self.assertTrue(observation['response_body_complete'])

    def test_cleanup_failure_retains_primary_error_and_closes_both_owners(self):
        read,c,clock,calls=harness();failure=OSError('primary');c.response.read_error=failure;c.response.close_error=RuntimeError();c.close_error=ValueError()
        with self.assertRaises(OSError)as caught:read('http://127.0.0.1:12345',BODY,{})
        self.assertIs(caught.exception,failure);self.assertEqual((c.close_count,c.response.close_count),(1,1))
        self.assertIn('Owned HTTP cleanup failed: RuntimeError,ValueError',failure.__notes__)

    def test_cleanup_failure_cannot_turn_success_into_pass(self):
        read,c,clock,calls=harness();c.close_error=ValueError()
        with self.assertRaisesRegex(RuntimeError,'Owned HTTP cleanup'):read('http://127.0.0.1:12345',BODY,{})

    def test_invalid_origin_body_and_deadline_do_not_create_connection(self):
        for origin in ['https://127.0.0.1:12345','http://localhost:12345','http://127.0.0.1:12345/path','http://user@127.0.0.1:12345','http://127.0.0.1:12345?','http://127.0.0.1:12345#']:
            read,c,clock,calls=harness()
            with self.subTest(origin=origin),self.assertRaises(ValueError):read(origin,BODY,{})
            self.assertEqual(calls,[])
        for body,timeout in [(b'{}',5),(BODY,0),(BODY,6),(BODY,True),(BODY,float('nan'))]:
            read,c,clock,calls=harness()
            with self.assertRaises(ValueError):read('http://127.0.0.1:12345',body,{},timeout)
            self.assertEqual(calls,[])


class NamedCheckTests(unittest.TestCase):
    def make_check(self,reader=None,early=None,write_error=False):
        self.writes=[];self.records=[];self.fixture=SimpleNamespace(receipt={'binary':{'sha256':'fixed'}},run=Path('/owned'),origin='http://127.0.0.1:12345')
        def write(path,value):
            self.writes.append((path,value.copy()))
            if write_error and path.name=='operational-failure.json':raise OSError('diagnostic failure')
        namespace={'self':self.fixture,'records':self.records,'read':reader,'read_oversized_json_response':early,'write_json':write}
        exec(compile(ast.Module(body=[CHECK],type_ignores=[]),str(SOURCE),'exec'),namespace)
        return namespace['check']

    def test_named_metadata_is_persisted_before_transport_failure(self):
        failure=BrokenPipeError('never serialize arbitrary error text')
        def reader(*args):raise failure
        check=self.make_check(reader=reader)
        with self.assertRaises(BrokenPipeError)as caught:check('bounded oversized JSON','/api/manga',413,method='POST',data=BODY)
        self.assertIs(caught.exception,failure);self.assertEqual(self.fixture.receipt['stage'],'operational-http')
        report=self.writes[-1][1];self.assertEqual(report['failed_check']['name'],'bounded oversized JSON')
        self.assertIsNone(report['observed_status']);self.assertEqual(report['transport_error_class'],'BrokenPipeError')
        self.assertNotIn('never serialize',repr(report));self.assertEqual(self.records,[])

    def test_diagnostic_write_failure_does_not_replace_original_transport_error(self):
        failure=OSError('primary')
        def reader(*args):raise failure
        check=self.make_check(reader=reader,write_error=True)
        with self.assertRaises(OSError)as caught:check('named','/api/manga',413)
        self.assertIs(caught.exception,failure)

    def test_wrong_actual_early_response_fails_expected_413(self):
        def early(origin,data,observation):observation.update(observed_status=401,response_body_complete=True);return 401,b'{}',{}
        check=self.make_check(early=early)
        with self.assertRaisesRegex(RuntimeError,'status=401'):check('bounded oversized JSON','/api/manga',413,method='POST',data=BODY,early_response=True)
        self.assertEqual(self.records[0]['status'],401);self.assertFalse(self.records[0]['passed'])

    def test_explicit_early_write_failure_with_no_response_still_fails(self):
        failure=RemoteDisconnected()
        def early(origin,data,observation):observation.update(write_error_class='BrokenPipeError',observed_status=None,response_body_complete=False);raise failure
        check=self.make_check(early=early)
        with self.assertRaises(RemoteDisconnected):check('bounded oversized JSON','/api/manga',413,method='POST',data=BODY,early_response=True)
        self.assertIsNone(self.writes[-1][1]['observed_status']);self.assertEqual(self.records,[])

    def test_only_exact_declared_probe_uses_early_response_owner(self):
        def early(*args):self.fail('wrong target dispatched')
        check=self.make_check(early=early)
        with self.assertRaises(ValueError):check('wrong','/api/other',413,method='POST',data=BODY,early_response=True)
        self.assertEqual(self.records,[])

    def test_real_operational_call_keeps_expected413_and_exact_full_body(self):
        calls=[n for n in ast.walk(OPERATIONS)if isinstance(n,ast.Call)and isinstance(n.func,ast.Name)and n.func.id=='check'and n.args and isinstance(n.args[0],ast.Constant)and n.args[0].value=='bounded oversized JSON']
        self.assertEqual(len(calls),1);call=calls[0]
        self.assertEqual([a.value for a in call.args],['bounded oversized JSON','/api/manga',413])
        keywords={k.arg:k.value for k in call.keywords}
        self.assertTrue(keywords['early_response'].value)
        self.assertEqual(ast.literal_eval(ast.Expression(keywords['method'])),'POST')
        self.assertEqual(ast.dump(keywords['data']),ast.dump(ast.parse("b'x'*(1024*1024+1)",mode='eval').body))


class OperationalPublicationTests(unittest.TestCase):
    def test_receipt_carries_exact_run_and_listener_restart_clears_marker(self):
        receipt=next(n.value for n in OPERATIONS.body if isinstance(n,ast.Assign)and any(isinstance(t,ast.Name)and t.id=='receipt'for t in n.targets))
        fields={k.value:v for k,v in zip(receipt.keys,receipt.values)}
        self.assertEqual(ast.dump(fields['run_directory']),ast.dump(ast.parse('str(self.run)',mode='eval').body))
        listeners=next(n for n in FIXTURE.body if isinstance(n,ast.FunctionDef)and n.name=='start_listeners')
        clear=next(n for n in listeners.body if isinstance(n,ast.Expr)and isinstance(n.value,ast.Call)and isinstance(n.value.func,ast.Attribute)and n.value.func.attr=='pop')
        self.assertEqual(ast.dump(clear.value),ast.dump(ast.parse("self.receipt.pop('operational_http',None)",mode='eval').body))
        self.assertLess(listeners.body.index(clear),next(i for i,n in enumerate(listeners.body)if isinstance(n,ast.Assign)and any(isinstance(t,ast.Name)and t.id=='deadline'for t in n.targets)))

    def publish(self,fail=None):
        class Target:
            def __init__(self,name):self.name=name;self.bytes=b''
            def __str__(self):return '/owned/'+self.name
            def read_bytes(self):
                if fail=='read':raise OSError('read failed')
                return self.bytes
        path=Target('live-native-http.json');supervisor=Target('supervisor.json');writes=[]
        class Directory:
            def __truediv__(self,name):return path if name=='live-native-http.json'else supervisor
        fixture=SimpleNamespace(receipt={'status':'http_ready_pending_operational_checks'},run=Directory())
        receipt={'run_directory':'exact-new-run','checks':[{'status':413}]}
        def write(target,value):
            if fail=='write'and target is path:raise OSError('write failed')
            target.bytes=json.dumps(value,sort_keys=True).encode();writes.append((target,value.copy()))
        start=next(i for i,n in enumerate(OPERATIONS.body)if isinstance(n,ast.Assign)and any(isinstance(t,ast.Name)and t.id=='operational_path'for t in n.targets))
        tail=OPERATIONS.body[start:]
        namespace={'self':fixture,'receipt':receipt,'evidence':Directory(),'write_json':write,'hashlib':hashlib,'json':json,'print':lambda *args,**kwargs:None,'records':receipt['checks'],'index_hash':'fixed-index'}
        try:exec(compile(ast.Module(body=tail,type_ignores=[]),str(SOURCE),'exec'),namespace)
        except OSError:return fixture,path,writes,True
        return fixture,path,writes,False

    def test_completion_marker_is_exact_saved_receipt_hash_before_ready(self):
        fixture,path,writes,failed=self.publish()
        self.assertFalse(failed);self.assertEqual(fixture.receipt['operational_http'],{'path':str(path),'sha256':hashlib.sha256(path.bytes).hexdigest()})
        self.assertEqual(writes[0][0].name,'live-native-http.json')
        self.assertEqual(writes[1][1]['status'],'ready');self.assertEqual(writes[1][1]['operational_http'],fixture.receipt['operational_http'])

    def test_failed_save_or_hash_read_never_publishes_completion(self):
        for fail in ['write','read']:
            with self.subTest(fail=fail):
                fixture,path,writes,failed=self.publish(fail)
                self.assertTrue(failed);self.assertNotIn('operational_http',fixture.receipt)
                self.assertEqual(fixture.receipt['status'],'http_ready_pending_operational_checks')
                self.assertFalse(any(target.name=='supervisor.json'for target,value in writes))


if __name__=='__main__':unittest.main()
