// Passive process observations only. Never handle uncaughtException or
// unhandledRejection: Node must retain its original fatal-exit behavior.
const stages = new Set(['initializing', 'launching-browser', 'creating-context', 'running-checks',
  'public-upload-fixture', 'public-upload-signer', 'public-upload-observers',
  'closing-context', 'closing-browser', 'closing-proxy', 'finalizing-report', 'complete']);
const fatalCodes = new Set(['ECONNRESET', 'EPIPE', 'ERR_STREAM_DESTROYED', 'ERR_SERVER_NOT_RUNNING']);
// A caught failure is distinct from Node's passive fatal monitor. Read private
// text only to recognize fixed Chromium refusal banners; never copy it out.
function caughtCategory(stage, error, steps) {
  if (stage === 'initializing') return ['EACCES', 'EPERM'].includes(error?.code) ? 'proxy-bind-permission' : 'proxy-listen-failed';
  if (stage === 'launching-browser') {
    const message = typeof error?.message === 'string' ? error.message : '';
    return ['No usable sandbox!', 'Failed to move to new namespace:', 'SUID sandbox helper binary', 'Running as root without --no-sandbox is not supported']
      .some(banner => message.includes(banner)) ? 'browser-sandbox-refused' : 'browser-launch-failed';
  }
  if (stage === 'creating-context') return 'context-setup-failed';
  if (stage.startsWith('public-upload-')) return 'journey-setup-failed';
  if (stage === 'running-checks') return Array.isArray(steps) && steps.length ? 'journey-failed' : 'journey-setup-failed';
  return 'harness-failed';
}

export function installRunnerLifecycle(report, save, runtime = process) {
  const value = report.runner_lifecycle = { stage: 'initializing', fatal_kind: 'none', fatal_code: 'none',
    fatal_stage: 'none', caught_stage: 'none', caught_category: 'none', exit_observed: false, exit_code: null };
  const persist = () => { try { save(); } catch { /* Observing cannot change the original process outcome. */ } };
  runtime.on('uncaughtExceptionMonitor', (error, origin) => {
    value.fatal_kind = origin === 'unhandledRejection' ? 'unhandled-rejection' : 'uncaught-exception';
    value.fatal_stage = value.stage;
    value.fatal_code = 'unknown';
    try { const code = error?.code; if (fatalCodes.has(code)) value.fatal_code = code; }
    catch { /* A passive monitor must not replace the original fatal outcome. */ }
    persist();
  });
  runtime.on('exit', code => {
    value.exit_observed = true;
    value.exit_code = Number.isInteger(code) && code >= 0 && code <= 255 ? code : null;
    persist();
  });
  const transition = stage => {
    if (!stages.has(stage)) throw Error('Unknown fixed runner stage');
    value.stage = stage; persist();
  };
  transition.caught = error => {
    // Keep the original site through all finally transitions. Classification or
    // evidence persistence must never replace the original caught outcome.
    if (value.caught_stage !== 'none') return;
    value.caught_stage = value.stage;
    value.caught_category = 'harness-failed';
    try { value.caught_category = caughtCategory(value.stage, error, report.steps); } catch { /* Unreadable errors stay generic. */ }
    persist();
  };
  return transition;
}
