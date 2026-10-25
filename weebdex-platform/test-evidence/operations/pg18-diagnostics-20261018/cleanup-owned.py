"""Fail-closed cleanup which still attempts every captured exact birth."""
import os,signal,time

def cleanup(h,children,known,unknown,deadline):
 errors=[];state={'forced':False};roots={child.pid for child,_ in children}
 def note(message):
  if message not in errors:errors.append(message)
 def exact_rows():
  rows=[]
  for ident in list(known.values()):
   try:
    value=h.same(ident)
    if value:rows.append(value)
   except Exception as error:note('identity inspection: '+type(error).__name__)
  return rows
 def reap(rows):
  for child,_ in children:
   try:child.poll()
   except Exception as error:note('direct-child reap: '+type(error).__name__)
  for value in rows:
   if value['state']=='Z' and value['ppid']==os.getpid() and value['pid'] not in roots:
    try:os.waitpid(value['pid'],os.WNOHANG)
    except ChildProcessError:pass
    except Exception as error:note('adopted-child reap: '+type(error).__name__)
 def inspect(ledger):
  if time.monotonic()>=deadline:raise TimeoutError('absolute cleanup deadline')
  try:return h.members(ledger,unknown=unknown)
  except Exception as error:
   note('process census: '+type(error).__name__)
   return exact_rows()
 def emit(ident,sig):
  try:h.send(ident,sig)
  except ProcessLookupError:pass
  except Exception as error:note('exact-birth signal: '+type(error).__name__)
 try:
  forced,remaining=h.stop(children,known=known,unknown=unknown,inspect=inspect,emit=emit,state=state)
 except BaseException as error:
  note('primary cleanup: '+type(error).__name__);forced=state['forced'];remaining=[]
  # A failed broad census never prevents an attempt on already captured births.
  # Every signal still goes through h.send's exact-birth/pidfd checks.
  for sig,seconds in [(signal.SIGINT,2),(signal.SIGTERM,2),(signal.SIGKILL,30)]:
   phase=min(deadline,time.monotonic()+seconds);sent=set()
   while time.monotonic()<phase:
    rows=exact_rows();reap(rows);live=[v for v in rows if v['state']!='Z']
    if not live:break
    if sig!=signal.SIGINT:forced=True
    for value in live:
     key=(value['pid'],value['start_ticks'])
     if key not in sent:emit(h.identity(value),sig);sent.add(key)
    time.sleep(min(.05,max(0,phase-time.monotonic())))
   if not [v for v in exact_rows() if v['state']!='Z']:break
  rows=exact_rows();reap(rows);remaining=[h.identity(v) for v in exact_rows() if v['state']!='Z']
 # Both the terminal census and each known identity must agree. A census error
 # stays an error even when every captured process is physically stopped.
 try:terminal=h.members(known,unknown=unknown)
 except Exception as error:note('terminal process census: '+type(error).__name__);terminal=exact_rows()
 reap(terminal);remaining={ (v['pid'],v['start_ticks']):h.identity(v) for v in [*terminal,*exact_rows()] if v['state']!='Z'}
 return forced,list(remaining.values()),'; '.join(errors) if errors else None
