"""Finite check identities and statuses only; never publish inspected values."""
CHECKS = {
    'account-recovery': [
        'device-library-seeded', 'logout-before-recovery', 'recovery-mail-delivered',
        'mail-link-local', 'reset-ui-mounted', 'reset-route', 'restricted-capability',
        'password-only-controls', 'private-library-denied', 'device-library-retained',
        'password-reset-accepted', 'reset-completion-visible', 'normal-session-confirmed',
        'account-settings-reached', 'old-password-rejected', 'new-password-authenticated',
        'device-library-retained-after-reset', 'recovery-link-replay-rejected',
    ],
    'ui10-recovery-lifetimes': ['recovery-reset-' + check for check in [
        'realm', 'hold-session', 'submit', 'held-session', 'newer-search',
        'late-response', 'no-stale-ui', 'logout', 'old-login-form', 'old-password',
        'old-login-settled', 'old-session-denied', 'new-password', 'link-replay',
    ]],
    'ui11-device-progress': ['device-progress-signup-context', *[
        'device-progress-' + gate + '-page' + str(page) + '-' + check
        for gate in ['timer', 'lock'] for page in [1, 3]
        for check in ['setup', 'select-page', 'no-prior-progress', 'gate-captured',
                      'old-intent-cancelled', 'page-retained', 'durable-progress',
                      'reload-progress', 'stable-revision']
    ]],
    'ui12-compact-feed': ['compact-feed-' + check for check in [
        'seed-pagination', 'spanish-preferences', 'fixture-total', 'fixture-response-received', 'fixture-http-200', 'fixture-total-integer',
        'fixture-total-at-least-seeded', 'fixture-total-within-known-scenarios', 'open-updates',
        'third-page', 'resume-visible', 'bounded-burst', 'compact-geometry',
        'trimmed-cards', 'manual-terminal-page', 'unique-sequential-pages',
        'scenario-membership', 'all-compact-chapters', 'terminal-idle',
        'resize-retains-entries', 'back-restores-entries',
    ]],
}

CHECKS['ui14-actor-follow'] = ['actor-follow-account', *[
    'actor-follow-' + kind + '-' + case for kind in ['group', 'user']
    for case in ['initial-readiness', 'read-retry', 'unblock-loading', 'unblock-error',
                 'pending-ack', 'pending-lost', 'pending-disposed']
]]

CHECKS['local-library'] = ['local-library-' + suffix for suffix in ['device-title-navigation', 'device-follow-trigger-visible-unique',
 'device-follow-popup-open', 'device-follow-option-selected', 'device-status-durable',
 'device-authority-unpublished', 'device-reload-status', 'device-library-navigation',
 'custom-list-navigation', 'custom-list-durable', 'custom-member-durable',
 'custom-backup-export', 'custom-foreign-backup-fenced', 'custom-backup-merged',
 'account-mode-ready', 'account-follow-trigger-visible-unique',
 'account-follow-popup-open', 'account-follow-option-selected',
 'account-status-durable', 'account-device-preserved', 'account-import-reviewed',
 'signer-section-open', 'signer-binding-installed', 'signer-dismissal-visible',
 'signer-outbox-empty']]
for _phase in ['signed-library', 'signed-profile', 'signed-social', 'signed-context',
 'signed-source-controls', 'signed-namespace', 'signed-recipients', 'signed-restart']:
    CHECKS[_phase] = [_phase + '-' + suffix for suffix in ['mapped-title-navigation', 'mapped-follow-trigger-visible-unique',
     'mapped-follow-popup-open', 'mapped-follow-option-selected',
     'mapped-status-durable', 'native-title-navigation',
     'native-follow-trigger-visible-unique', 'native-follow-popup-open',
     'native-follow-option-selected', 'reader-bytes', 'reader-progress-durable',
     'library-navigation', 'device-list-durable', 'device-member-durable',
     'device-authority-unpublished', 'missing-signer-refused',
     'signer-binding-installed', 'signer-selection-denied', 'signer-connected',
     'recipient-wrong-key-refused', 'recipient-disallowed-origin-refused',
     'recipient-verified', 'recipient-selected']]

COMPACT_HTTP_OUTCOMES = [
    'success200', 'rate-limited429', 'unavailable503', 'other4xx',
    'other5xx', 'otherstatus', 'transportfailure', 'invalidJSON',
]

STATUSES = {'pending', 'running', 'passed', 'failed'}


def project(phase, acceptance):
    identifiers = CHECKS[phase]
    key = 'account_recovery_checks' if phase == 'account-recovery' else 'journey_checks'
    observed = acceptance.get(key)
    valid = type(observed) is list and len(observed) <= len(identifiers)
    rows = {check_id: 'pending' for check_id in identifiers}
    seen = set()
    prior_index = -1
    for row in observed[:len(identifiers)] if type(observed) is list else []:
        if type(row) is not dict or set(row) != {'check_id', 'status'}:
            valid = False
            continue
        check_id, status = row.get('check_id'), row.get('status')
        if type(check_id) is not str or check_id not in rows or type(status) is not str or status not in STATUSES or check_id in seen:
            valid = False
            continue
        index = identifiers.index(check_id)
        valid = valid and index > prior_index
        prior_index = index
        seen.add(check_id)
        rows[check_id] = status
    outcome = acceptance.get('compact_feed_http_outcome')
    if phase == 'ui12-compact-feed' and 'compact_feed_http_outcome' in acceptance:
        valid = valid and type(outcome) is str and outcome in COMPACT_HTTP_OUTCOMES
    value = {'schema': 1, 'record_valid': bool(valid), 'private_values_omitted': True,
            'raw_messages_exported': False,
            'check_results': [{'check_id': check_id, 'status': rows[check_id]} for check_id in identifiers]}

    if phase == 'ui12-compact-feed':
        value['compact_feed_http_outcome'] = outcome if type(outcome) is str and outcome in COMPACT_HTTP_OUTCOMES else 'unavailable'
    return value
