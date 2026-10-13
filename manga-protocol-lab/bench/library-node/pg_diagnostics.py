"""Read-only benchmark diagnostics, never a functional correctness oracle.

Uses libpq environment variables; credentials are never command arguments/output.
Each bounded CLI invocation returns one JSON observation for one database.
"""
import argparse
import asyncio
from datetime import datetime
import json
import math
import os
import re
import subprocess
import time

QUERY = """
BEGIN READ ONLY;
SET LOCAL statement_timeout='2000ms';
SET LOCAL lock_timeout='500ms';
SELECT json_build_object(
 'sampled_at',clock_timestamp(),
 'backend_outbox',(SELECT json_build_object(
   'pending_import',count(*) FILTER(WHERE action='import'),
   'pending_delete',count(*) FILTER(WHERE action='delete'),
   'oldest_pending_age_s',extract(epoch FROM clock_timestamp()-min(created_at)))
   FROM backend_outbox WHERE completed_at IS NULL),
 'library_retirement',(SELECT json_build_object(
   'removed_history_rows',count(*),
   'older_than_grace_rows',count(*) FILTER(WHERE retired_at<=clock_timestamp()-interval '5 minutes'),
   'oldest_retired_age_s',extract(epoch FROM clock_timestamp()-min(retired_at)))
   FROM events WHERE kind=11999 AND state='removed' AND retired_at IS NOT NULL),
 'library_holds',(SELECT json_build_object(
   'awaiting_projection',count(*) FILTER(WHERE state='pending'),
   'grace_rows',count(*) FILTER(WHERE state='grace'),
   'grace_expired_rows',count(*) FILTER(WHERE state='grace' AND grace_until<=clock_timestamp()))
   FROM library_object_holds WHERE state IN ('pending','grace')),
 'blob_cleanup',(SELECT json_build_object(
   'queued',count(*),'due',count(*) FILTER(WHERE due_at<=clock_timestamp()),
   'oldest_overdue_s',greatest(0,extract(epoch FROM clock_timestamp()-min(due_at))),
   'library_candidates',count(*) FILTER(WHERE source_mask & 1 = 1),
   'publication_candidates',count(*) FILTER(WHERE source_mask & 2 = 2),
   'staging_candidates',count(*) FILTER(WHERE source_mask & 4 = 4)) FROM blob_gc_queue),
 'blob_reclaims',(SELECT json_build_object(
   'claimed',count(*),'retry_ready',count(*) FILTER(WHERE retry_after<=clock_timestamp()),
   'oldest_claim_age_s',extract(epoch FROM clock_timestamp()-min(claimed_at))) FROM blob_reclaims),
 'public_export',(SELECT json_build_object(
   'queued',count(*),'due',count(*) FILTER(WHERE due_at<=clock_timestamp()),
   'oldest_overdue_s',greatest(0,extract(epoch FROM clock_timestamp()-min(due_at)))) FROM public_export_queue)
);
COMMIT;
"""

ACTIVITY_QUERY = """
BEGIN READ ONLY;
SET LOCAL statement_timeout='2000ms';
SET LOCAL lock_timeout='500ms';
SELECT json_build_object(
 'sampled_at',clock_timestamp(),
 'postmaster_started_at',pg_postmaster_start_time(),
 'server_version_num',current_setting('server_version_num')::integer,
 'pg_stat_wal',(SELECT json_build_object(
   'wal_records',wal_records,'wal_fpi',wal_fpi,'wal_bytes',wal_bytes,
   'wal_buffers_full',wal_buffers_full,'stats_reset',stats_reset)
   FROM pg_stat_wal),
 'pg_stat_checkpointer',(SELECT json_build_object(
   'num_timed',num_timed,'num_requested',num_requested,'num_done',num_done,
   'restartpoints_timed',restartpoints_timed,'restartpoints_req',restartpoints_req,
   'restartpoints_done',restartpoints_done,'write_time',write_time,
   'sync_time',sync_time,'buffers_written',buffers_written,
   'slru_written',slru_written,'stats_reset',stats_reset)
   FROM pg_stat_checkpointer),
 'settings',(SELECT json_object_agg(name,json_build_object(
   'value',current_setting(name),'setting',setting,'unit',unit,'source',source)
   ORDER BY name) FROM pg_settings WHERE name IN (
   'fsync','synchronous_commit','full_page_writes','shared_buffers','max_wal_size',
   'wal_buffers','wal_compression','checkpoint_timeout',
   'checkpoint_completion_target','wal_sync_method')),
 'activity',COALESCE((
   SELECT json_agg(row_to_json(grouped) ORDER BY
     grouped.database,grouped.backend_type,grouped.state,
     grouped.wait_event_type,grouped.wait_event)
   FROM (
     SELECT datname AS database,backend_type,state,wait_event_type,wait_event,
       count(*) AS sessions
     FROM pg_stat_activity
     WHERE datname=current_database() AND pid<>pg_backend_pid()
     GROUP BY datname,backend_type,state,wait_event_type,wait_event
   ) AS grouped
 ),'[]'::json)
);
COMMIT;
"""

# PostgreSQL 18.6: system_views.sql and monitoring.sgml define these fixed
# single-row cluster counters. WAL write/sync timings are NOT in pg_stat_wal.
WAL_COUNTERS = ('wal_records','wal_fpi','wal_bytes','wal_buffers_full')
CHECKPOINTER_COUNTERS = ('num_timed','num_requested','num_done',
                        'restartpoints_timed','restartpoints_req','restartpoints_done',
                        'write_time','sync_time','buffers_written','slru_written')
SETTING_NAMES = ('fsync','synchronous_commit','full_page_writes','shared_buffers',
                 'max_wal_size','wal_buffers','wal_compression','checkpoint_timeout',
                 'checkpoint_completion_target','wal_sync_method')
SETTING_SOURCES = frozenset(('default','environment variable','configuration file',
                            'command line','global','database','user','database user',
                            'client','override','interactive','test','session'))
SETTING_ENUMS = dict(fsync=('on','off'),full_page_writes=('on','off'),
                    synchronous_commit=('on','off','local','remote_write','remote_apply'),
                    wal_compression=('on','off','pglz','lz4','zstd'),
                    wal_sync_method=('fsync','fsync_writethrough','fdatasync','open_sync','open_datasync'))


def validate_wal_snapshot(value):
    """Return only approved bounded metadata; reject missing/invalid data.

    Raw cumulative counters retain their independent reset timestamps. This
    layer never computes deltas, resets stats, or replaces missing data by zero.
    Settings describe this sampler session, not other sessions' overrides.
    """
    if not isinstance(value,dict):raise ValueError('invalid WAL snapshot')
    def timestamp(raw):
        if not isinstance(raw,str) or len(raw)>64:raise ValueError('invalid statistics timestamp')
        parsed=datetime.fromisoformat(raw)
        if parsed.tzinfo is None:raise ValueError('statistics timezone required')
        if not math.isfinite(parsed.timestamp()):raise ValueError('invalid statistics timestamp')
        return raw
    timestamp(value.get('sampled_at'))
    result=dict(postmaster_started_at=timestamp(value.get('postmaster_started_at')))
    version=value.get('server_version_num')
    if type(version) is not int or not 180000<=version<190000:raise ValueError('PostgreSQL 18 statistics required')
    result['server_version_num']=version
    for name,keys in (('pg_stat_wal',WAL_COUNTERS),('pg_stat_checkpointer',CHECKPOINTER_COUNTERS)):
        raw=value.get(name)
        if not isinstance(raw,dict):raise ValueError('missing statistics object')
        counters={}
        for key in keys:
            number=raw.get(key)
            # PostgreSQL bigint/uint64 counters, plus finite millisecond timings.
            numeric=key in ('wal_bytes','write_time','sync_time')
            if type(number) not in ((int,float) if numeric else (int,)) or not 0<=number<=2**64 or not math.isfinite(number):
                raise ValueError('invalid statistics counter')
            counters[key]=number
        if 'stats_reset' not in raw:raise ValueError('missing statistics reset identity')
        counters['stats_reset']=None if raw['stats_reset'] is None else timestamp(raw['stats_reset'])
        result[name]=counters
    settings=value.get('settings')
    if not isinstance(settings,dict):raise ValueError('missing PostgreSQL settings')
    result['settings']={}
    for name in SETTING_NAMES:
        raw=settings.get(name)
        if not isinstance(raw,dict):raise ValueError('missing PostgreSQL setting')
        setting={key:raw.get(key) for key in ('value','setting','unit','source')}
        for key in ('value','setting'):
            text=setting[key]
            if not isinstance(text,str) or not 0<len(text)<=64:raise ValueError('invalid PostgreSQL setting')
            if name in SETTING_ENUMS:
                if text not in SETTING_ENUMS[name]:raise ValueError('invalid PostgreSQL enum')
            elif re.fullmatch(r'[0-9]+(?:\.[0-9]+)?(?:[kMGT]?B|ms|s|min|h|d)?',text) is None:
                raise ValueError('invalid PostgreSQL numeric setting')
        if setting['unit'] not in (None,'B','kB','MB','GB','8kB','ms','s','min') or not isinstance(setting['source'],str) or setting['source'] not in SETTING_SOURCES:
            raise ValueError('invalid PostgreSQL setting metadata')
        result['settings'][name]=setting
    return result


async def collect_sample(command, database, *, expected_field='backend_outbox'):
    """Supervise a bounded diagnostic CLI and validate its public JSON result.

    The production CLI owns its child timeout/reaping. On cancellation we wait
    for that bounded CLI to finish rather than orphan its psql child.
    """
    process = await asyncio.create_subprocess_exec(*command, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.DEVNULL)
    communication=asyncio.create_task(process.communicate())
    try: stdout, _ = await asyncio.shield(communication)
    except asyncio.CancelledError:
        await communication
        raise
    try:
        value=json.loads(stdout)
        if not isinstance(value, dict) or (expected_field not in value and 'error' not in value): raise ValueError('diagnostic shape')
        if expected_field=='activity' and 'error' not in value and not isinstance(value['activity'],list): raise ValueError('activity shape')
    except ValueError: value=dict(database=database,error='invalid diagnostic output')
    if process.returncode and not value.get('error'): value['error']='diagnostic process failed'
    return value


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--psql',default='vendor/pgsql/bin/psql')
    parser.add_argument('--database',required=True)
    parser.add_argument('--activity-only',action='store_true',help='Grouped activity/waits for this database plus cluster WAL/checkpointer counters and selected settings; no query text or session identities.')
    args=parser.parse_args()
    started=time.monotonic()
    env=dict(os.environ,PGDATABASE=args.database,PGCONNECT_TIMEOUT='2',PGOPTIONS='-c default_transaction_read_only=on')
    try:
        result=subprocess.run([args.psql,'-X','-q','-A','-t','-v','ON_ERROR_STOP=1'],input=ACTIVITY_QUERY if args.activity_only else QUERY,text=True,capture_output=True,env=env,timeout=5,check=True)
        value=json.loads(result.stdout)
        if not isinstance(value,dict): raise ValueError('diagnostic shape')
        if args.activity_only:value.update(validate_wal_snapshot(value))
        caveat=('Activity and waits are sampled internal performance diagnostics, never proof of acceptance or correctness. Brief waits may be missed. Activity includes only the selected database; peer databases and cluster background processes without a database are absent, not evidence of no waits. WAL/checkpointer counters are raw cluster-wide cumulative statistics, may lag current activity, and require reset/restart checks before deltas. Checkpointer write_time/sync_time are milliseconds; write_time includes checkpoint pacing and is not foreground blocked time. Settings are actual sampler-session current_setting values with pg_settings raw setting/unit/source, not proof of application-session overrides.' if args.activity_only else
                'Retired history/expired grace rows are candidates, not necessarily eligible: references and active leases may still protect them. Queue counts are internal performance diagnostics, never proof of acceptance or correctness.')
        value.update(database=args.database,diagnostic_only=True,elapsed_s=time.monotonic()-started,
                     caveat=caveat)
        print(json.dumps(value))
    except (subprocess.SubprocessError,ValueError,OSError) as error:
        # libpq errors can contain connection strings. Record only a safe class.
        print(json.dumps(dict(database=args.database,diagnostic_only=True,error=type(error).__name__,elapsed_s=time.monotonic()-started)))
        raise SystemExit(2)

if __name__=='__main__':main()
