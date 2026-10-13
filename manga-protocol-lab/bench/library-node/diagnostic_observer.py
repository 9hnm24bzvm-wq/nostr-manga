"""Optional bounded read-only observers; never target offers or acceptance evidence."""
import asyncio
from contextlib import asynccontextmanager
import datetime
import json
import math
import os
from pathlib import Path
import sys
import time

INTERVAL_S = 5.0
STATUS_TIMEOUT_S = 1.0

def settings():
    return dict(interval_s=INTERVAL_S, status_timeout_s=STATUS_TIMEOUT_S,
                pg_cli_timeout_s=5, pg_statement_timeout_ms=2000, pg_lock_timeout_ms=500,
                database='manga_mixed_0', status_node='main', single_flight_per_kind=True,
                retries=False, missing_samples_are_zero=False,
                affects_target_offer_counts=False,
                pg_wal_checkpointer_counters=True, pg_effective_settings=True,
                caveat='Diagnostic overhead;5s snapshots can miss short waits. Status shares Catalogue/PG admission; successful samples are selection-biased. Front processing admission remains unmeasured.')

async def status_sample(wire, admin, current, timeout=STATUS_TIMEOUT_S):
    token=current.set('diagnostic_pool_status')
    try:
        value, record=await asyncio.wait_for(wire.call('GET','/admin/status',account=admin),timeout)
        pool=value['pool']
        if not isinstance(pool,dict) or any(type(pool.get(k)) is not int or pool[k]<0 for k in ('size','available','waiting')) or pool['available']>pool['size']:
            raise ValueError('invalid pool counters')
        return dict(pool={k:pool[k] for k in ('size','available','waiting')},http_status=record['status'])
    finally:current.reset(token)

async def activity_sample(command):
    from pg_diagnostics import collect_sample, validate_wal_snapshot
    value=await collect_sample(command,'manga_mixed_0',expected_field='activity')
    # CLI output is bounded aggregate metadata. Never copy arbitrary extra fields.
    if value.get('error'):return dict(error=value['error'],pg_elapsed_s=value.get('elapsed_s'))
    activity=value.get('activity')
    if not isinstance(activity,list) or len(activity)>1000:raise ValueError('invalid activity rows')
    rows=[]
    for row in activity:
        if not isinstance(row,dict) or type(row.get('sessions')) is not int or row['sessions']<0:raise ValueError('invalid activity count')
        keys=('database','backend_type','state','wait_event_type','wait_event')
        if any(row.get(k) is not None and not isinstance(row[k],str) for k in keys):raise ValueError('invalid activity label')
        rows.append({**{k:row.get(k) for k in keys},'sessions':row['sessions']})
    elapsed=value.get('elapsed_s')
    if not isinstance(elapsed,(int,float)) or not math.isfinite(elapsed) or elapsed<0:raise ValueError('invalid activity elapsed')
    return dict(activity=rows,sampled_at=value.get('sampled_at'),pg_elapsed_s=elapsed,**validate_wal_snapshot(value))

def counter_deltas(previous, current, missing_reason='first_sample'):
    """Only same-reset, same-postmaster monotone observations support deltas."""
    result={}
    for name in ('pg_stat_wal','pg_stat_checkpointer'):
        values=None;elapsed=None;status=missing_reason
        if previous is not None:
            old=previous[name];new=current[name]
            elapsed=(datetime.datetime.fromisoformat(current['sampled_at'])-datetime.datetime.fromisoformat(previous['sampled_at'])).total_seconds()
            if previous['postmaster_started_at']!=current['postmaster_started_at']:status='postmaster_changed'
            elif old['stats_reset'] is None or new['stats_reset'] is None:status='reset_identity_unavailable'
            elif old['stats_reset']!=new['stats_reset']:status='stats_reset_changed'
            elif elapsed<=0:status='nonpositive_sample_interval'
            elif any(new[k]<old[k] for k in new if k!='stats_reset'):status='counter_decreased'
            else:
                status='continuous';values={k:new[k]-old[k] for k in new if k!='stats_reset'}
        result[name]=dict(status=status,elapsed_s=elapsed,values=values)
    return result

async def periodic(kind, sample, interval, phase, stream, summary):
    next_at=time.monotonic();sequence=0;previous=None;missing_reason='first_sample'
    while True:
        await asyncio.sleep(max(0,next_at-time.monotonic()))
        began=time.monotonic();row=dict(kind=kind,sequence=sequence,phase=phase(),scheduled_monotonic=next_at,
                                      started_unix_ms=time.time()*1000,scheduling_lateness_s=max(0,began-next_at))
        cancelled=False
        try:
            row.update(await sample())
            row['outcome']='error' if row.get('error') else 'observed'
        except asyncio.CancelledError:
            row.update(outcome='cancelled_by_owner');cancelled=True
        except Exception as error:
            # Error messages and response payloads may contain secrets. Keep classes only.
            row.update(outcome='error',error=type(error).__name__)
            if type(getattr(error,'status',None)) is int:row['http_status']=error.status
            if getattr(error,'response_verification',False):row['response_verification_failed']=True
        row.update(finished_unix_ms=time.time()*1000,elapsed_s=time.monotonic()-began)
        next_at+=interval
        skipped=max(0,math.floor((time.monotonic()-next_at)/interval)+1) if next_at<=time.monotonic() else 0
        next_at+=skipped*interval
        row['skipped_intervals_after_sample']=skipped
        if kind=='pg_activity':
            if row['outcome']=='observed':
                row['counter_deltas']=counter_deltas(previous,row,missing_reason)
                previous=row
            else:
                previous=None;missing_reason='previous_sample_missing'
        stream.write(json.dumps(row)+'\n');stream.flush()
        stats=summary['observations'].setdefault(kind,dict(attempted=0,observed=0,error=0,cancelled_by_owner=0,skipped_intervals=0))
        stats['attempted']+=1;stats[row['outcome']]+=1;stats['skipped_intervals']+=skipped
        if cancelled:raise asyncio.CancelledError
        sequence+=1

@asynccontextmanager
async def observers(enabled,wire,admin,current,out,phase,*,interval=INTERVAL_S,status_timeout=STATUS_TIMEOUT_S,pg_command=None):
    """Actual driver-owned task lifetime, tested with real signed HTTP/CLI children."""
    if not enabled:
        yield None
        return
    if not math.isfinite(interval) or interval<=0 or not math.isfinite(status_timeout) or status_timeout<=0:
        raise ValueError('finite positive diagnostic bounds required')
    if pg_command is None:
        pg_command=[sys.executable,str(Path(__file__).with_name('pg_diagnostics.py')),'--psql',os.environ.get('MIXED_PSQL','vendor/pgsql/bin/psql'),'--database','manga_mixed_0','--activity-only']
    summary=dict(settings=settings(),observations={},tasks_joined=False,task_errors=[])
    summary['settings'].update(interval_s=interval,status_timeout_s=status_timeout)
    path=Path(out)/'additional-diagnostics.jsonl'
    with path.open('x') as stream:
        tasks=[asyncio.create_task(periodic('pool_status',lambda:status_sample(wire,admin,current,status_timeout),interval,phase,stream,summary)),
               asyncio.create_task(periodic('pg_activity',lambda:activity_sample(pg_command),interval,phase,stream,summary))]
        try:yield summary
        finally:
            for task in tasks:task.cancel()
            results=await asyncio.gather(*tasks,return_exceptions=True)
            summary['task_errors']=[type(v).__name__ for v in results if isinstance(v,Exception)]
            summary['tasks_joined']=all(task.done() for task in tasks)
