// Passive process observations only. Never handle uncaughtException or
// unhandledRejection: Node must retain its original fatal-exit behavior.
const stages = new Set(['initializing', 'launching-browser', 'creating-context', 'running-checks',
  'closing-context', 'closing-browser', 'closing-proxy', 'finalizing-report', 'complete']);
const fatalCodes = new Set(['ECONNRESET', 'EPIPE', 'ERR_STREAM_DESTROYED', 'ERR_SERVER_NOT_RUNNING']);

export function installRunnerLifecycle(report, save, runtime = process) {
  const value = report.runner_lifecycle = { stage: 'initializing', fatal_kind: 'none', fatal_code: 'none',
    fatal_stage: 'none', exit_observed: false, exit_code: null };
  const persist = () => { try { save(); } catch { /* Observing cannot change the original process outcome. */ } };
  runtime.on('uncaughtExceptionMonitor', (error, origin) => {
    value.fatal_kind = origin === 'unhandledRejection' ? 'unhandled-rejection' : 'uncaught-exception';
    value.fatal_stage = value.stage;
    value.fatal_code = fatalCodes.has(error?.code) ? error.code : 'unknown';
    persist();
  });
  runtime.on('exit', code => {
    value.exit_observed = true;
    value.exit_code = Number.isInteger(code) && code >= 0 && code <= 255 ? code : null;
    persist();
  });
  return stage => {
    if (!stages.has(stage)) throw Error('Unknown fixed runner stage');
    value.stage = stage; persist();
  };
}
