"""Closed diagnostic schemas and bounded owned-worker log witnesses only."""
import re
MAX_LOG = 4 * 1024 * 1024
MAX_LINE = 64 * 1024
MAX_EVENTS = 256
SIGNATURES = ('sandbox', 'loader', 'timeout', 'premature_exit', 'spawn_error')
CODES = frozenset(('ENOENT','EACCES','EPERM','ENOEXEC','ENOMEM','ENOSPC','ETIMEDOUT','EPIPE','ECONNRESET'))
SPAWN = frozenset(('ENOENT','EACCES','EPERM','ENOEXEC','ENOMEM','ENOSPC'))
FAMILIES = dict(ENOENT='spawn-not-found',EACCES='permission-refused',EPERM='permission-refused',ENOEXEC='executable-format',ENOMEM='resource-unavailable',ENOSPC='resource-unavailable',ETIMEDOUT='launch-timeout',EPIPE='transport-closed',ECONNRESET='transport-closed')
HOST = {'executable':{'ready','not-executable','not-regular','unavailable'},'tmp':{'ready','refused','unavailable'},'userns_clone':{'enabled','disabled','unavailable'},'userns_max':{'positive','zero','unavailable'},'apparmor_userns':{'enabled','disabled','unavailable'},'seccomp':{'none','strict','filter','unavailable'}}
SIGNALS = frozenset(('none','unknown','other','multiple','unavailable','SIGABRT','SIGBUS','SIGILL','SIGKILL','SIGSEGV','SIGSYS','SIGTERM','SIGTRAP'))
BANNERS = ('Chromium sandboxing failed!','No usable sandbox!','Failed to move to new namespace:','SUID sandbox helper binary','Running as root without --no-sandbox is not supported')

def empty_error():
    return dict(reason='none',code='none',code_source='none',message_state='not-observed',signatures=dict.fromkeys(SIGNATURES,None))
def empty_worker():
    return dict(error=empty_error(),host={k:'unavailable' for k in HOST})
def worker_valid(value, launching=False):
    if type(value)is not dict or set(value)!={'error','host'}:return False
    host=value['host'];error=value['error']
    if type(host)is not dict or set(host)!=set(HOST)or any(type(host[k])is not str or host[k]not in HOST[k]for k in HOST):return False
    if type(error)is not dict or set(error)!={'reason','code','code_source','message_state','signatures'}:return False
    if any(type(error[k])is not str for k in ('reason','code','code_source','message_state')):return False
    if not launching:
        return type(error['signatures'])is dict and set(error['signatures'])==set(SIGNATURES)and all(v is None for v in error['signatures'].values())and error==empty_error()
    code,source,state=error['code'],error['code_source'],error['message_state']
    if code not in CODES|{'none','unknown'}or source not in {'none','unavailable','property','spawn-envelope','conflict'}or state not in {'bounded','unavailable'}:return False
    if (source=='none')!=(code=='none')or (source in {'unavailable','conflict'})!=(code=='unknown'):return False
    flags=error['signatures']
    if type(flags)is not dict or set(flags)!=set(SIGNATURES)or any(type(x)is not bool and x is not None for x in flags.values()):return False
    if state=='bounded'and any(type(x)is not bool for x in flags.values()):return False
    if state=='unavailable'and(any(flags[k]is not None for k in ('sandbox','loader','premature_exit'))or flags['timeout']is False or flags['spawn_error']is False):return False
    if source in {'spawn-envelope','conflict'}and state!='bounded':return False
    if source=='spawn-envelope'and(code not in SPAWN or flags['spawn_error']is not True):return False
    if code in SPAWN and flags['spawn_error']is not True:return False
    if code not in SPAWN and source!='conflict'and flags['spawn_error']is True:return False
    if source=='conflict'and flags['spawn_error']is not True:return False
    if code=='ETIMEDOUT'and flags['timeout']is not True:return False
    families=set()
    for key,family in [('sandbox','sandbox-refused'),('loader','missing-runtime-library'),('timeout','launch-timeout'),('premature_exit','launch-closed-before-ready')]:
        if flags[key]is True:families.add(family)
    if code in FAMILIES:families.add(FAMILIES[code])
    reason='multiple-known-families'if source=='conflict'or len(families)>1 else next(iter(families))if families else 'unknown'if state=='bounded'else 'unavailable'
    return error['reason']==reason

def empty_log(state='unavailable'):
    return {'process':dict(witness='unavailable',state=state,started=None,exited=None,launches=None,exit_events=None,exit_code=None,exit_signal='unavailable'),'log_signatures':dict.fromkeys(SIGNATURES,None)}
def valid_log(value):
    if type(value)is not dict or set(value)!={'process','log_signatures'}:return False
    p=value['process'];flags=value['log_signatures']
    if type(p)is not dict or set(p)!={'witness','state','started','exited','launches','exit_events','exit_code','exit_signal'}:return False
    if type(p['state'])is not str or p['state']not in {'complete','partial','unavailable','malformed','oversized'}:return False
    if p['state']!='complete':
        return type(p['witness'])is str and type(p['exit_signal'])is str and all(p[k]is None for k in ('started','exited','launches','exit_events','exit_code'))and type(flags)is dict and set(flags)==set(SIGNATURES)and all(x is None for x in flags.values())and value==empty_log(p['state'])
    if p['witness']!='owned-worker-log'or type(flags)is not dict or set(flags)!=set(SIGNATURES)or any(type(x)is not bool for x in flags.values()):return False
    if any(type(p[k])is not int or not 0<=p[k]<=MAX_EVENTS for k in ('launches','exit_events')):return False
    if type(p['started'])is not bool or type(p['exited'])is not bool or p['started']!=(p['launches']>0)or p['exited']!=(p['exit_events']>0):return False
    if p['exit_events']>0 and p['launches']==0:return False
    if p['exit_code']is not None and(type(p['exit_code'])is not int or not 0<=p['exit_code']<=255):return False
    if type(p['exit_signal'])is not str or p['exit_signal']not in SIGNALS:return False
    if p['launches']!=1 or p['exit_events']!=1:
        return p['exit_code']is None and p['exit_signal']==('multiple'if max(p['launches'],p['exit_events'])>1 else'unavailable')
    if p['exit_signal']in {'multiple','unavailable'}:return False
    if p['exit_code']is not None and p['exit_signal']!='none':return False
    if p['exit_code']is None and p['exit_signal']=='none':return False
    return True

def parse_log(raw):
    if type(raw)is not bytes:return empty_log()
    if len(raw)>MAX_LOG:return empty_log('oversized')
    try:text=raw.decode('utf-8')
    except UnicodeError:return empty_log('malformed')
    if raw and not raw.endswith(b'\n'):return empty_log('partial')
    lines=text.split('\n')[:-1] if raw else []
    if any(len(line.encode('utf-8'))>MAX_LINE for line in lines):return empty_log('oversized')
    starts=[];exits=[];flags=dict.fromkeys(SIGNATURES,False)
    start_re=re.compile(r'^pw:browser <launched> pid=([1-9][0-9]{0,9})$')
    exit_re=re.compile(r'^pw:browser \[pid=([1-9][0-9]{0,9})\] <process did exit: exitCode=(null|[0-9]{1,3}), signal=(null|SIG[A-Z0-9]{1,10})>$')
    stderr_re=re.compile(r'^pw:browser \[pid=[1-9][0-9]{0,9}\]\[(?:err|out)\] (.*)$')
    for line in lines:
        match=start_re.fullmatch(line)
        if match:starts.append(match[1])
        elif line.startswith('pw:browser <launched>'):return empty_log('malformed')
        match=exit_re.fullmatch(line)
        if match:
            code=None if match[2]=='null'else int(match[2])
            if code is not None and not 0<=code<=255:return empty_log('malformed')
            sig='none'if match[3]=='null'and code is not None else 'unknown'if match[3]=='null'else match[3]if match[3]in SIGNALS else'other'
            if code is not None and sig!='none':return empty_log('malformed')
            exits.append((match[1],code,sig))
        elif line.startswith('pw:browser ')and '<process did exit:'in line and not stderr_re.fullmatch(line):return empty_log('malformed')
        match=stderr_re.fullmatch(line)
        if match:
            body=match[1]
            flags['sandbox']|=any(x in body for x in BANNERS)
            flags['loader']|='error while loading shared libraries:'in body or'Host system is missing dependencies to run browsers.'in body
            flags['timeout']|=bool(re.search(r'\bTimeout [0-9]{1,9}ms exceeded\b',body))
            flags['premature_exit']|='Failed to launch the browser process.'in body or'Target page, context or browser has been closed'in body
            flags['spawn_error']|=bool(re.search(r'(?:^|\n)(?:browserType\.launch: )?(?:Failed to launch: (?:Error: )?)?spawn(?:Sync)? [^\r\n]{1,4096} (ENOENT|EACCES|EPERM|ENOEXEC|ENOMEM|ENOSPC)(?=$|\n|\r)',body))
        if len(starts)>MAX_EVENTS or len(exits)>MAX_EVENTS:return empty_log('oversized')
    if any(pid not in starts for pid,_,_ in exits):return empty_log('partial')
    code=None;sig='multiple'if max(len(starts),len(exits))>1 else'unavailable'
    if len(starts)==len(exits)==1:_,code,sig=exits[0]
    value={'process':dict(witness='owned-worker-log',state='complete',started=bool(starts),exited=bool(exits),launches=len(starts),exit_events=len(exits),exit_code=code,exit_signal=sig),'log_signatures':flags}
    return value if valid_log(value)else empty_log('malformed')
