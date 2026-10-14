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

ACTOR_FOLLOW_CASE_CHECKS = {
    'initial-readiness': ['prerequisite', 'initial-session', 'status-pending', 'status-settled'],
    'read-retry': ['prerequisite', 'delivery-failed', 'retry-pending', 'retry-settled'],
    'unblock-loading': ['prerequisite', 'status-held', 'unblock-committed', 'follow-restored'],
    'unblock-error': ['prerequisite', 'status-held', 'unblock-committed', 'follow-restored'],
    'pending-ack': ['prerequisite', 'write-committed', 'pending-controls', 'delivery-settled'],
    'pending-lost': ['prerequisite', 'write-committed', 'pending-controls', 'delivery-settled'],
    'pending-disposed': ['prerequisite', 'write-committed', 'pending-controls', 'delivery-settled'],
}
CHECKS['ui14-actor-follow'] = ['actor-follow-account', *[
    'actor-follow-' + kind + '-' + case + suffix
    for kind in ['group', 'user'] for case, checks in ACTOR_FOLLOW_CASE_CHECKS.items()
    for suffix in ['', *['-' + check for check in checks]]
]]
CHECKS['ui10-reader-transition'] = ['reader-transition-' + control + '-' + check
    for control in ['403', '404', 'delayed', 'normal']
    for check in ['open', 'page-one', 'image-bytes', 'settings', 'shortcut-focus', 'navigation', 'destination']]
CHECKS['ui10-reader-transition'] += ['reader-transition-normal-destination-' + check for check in [
    'title-control', 'title-route', 'chapter-link', 'page-one', 'image-bytes', 'realm', 'progress', 'incomplete', 'settled']]
CHECKS['ui14-home-failures'] = [*['home-pg-' + check for check in [
    'capability', 'open', 'held', 'begin', 'outage', 'first-failure', 'retry-second', 'retry-third',
    'three-denials', 'no-false-empty', 'resume', 'restored', 'recovery-retry', 'recovery-read']],
    *['home-private403-' + check for check in ['login', 'logout', 'request-form', 'mail', 'link', 'cached-session',
        'redemption', 'restricted-auth', 'native-denial', 'widget-open', 'feedback', 'single-read', 'retry', 'repeated-denial']]]
CHECKS['ui14-account-failures'] = ['account429-title-' + check for check in [
    'prerequisite', 'open', 'account-mode', 'durable-baseline', 'trigger', 'option',
    'pressure-captured', 'capacity-attempt', 'capacity-fixture', 'capacity-connections', 'capacity-saturated',
    'capacity-live', 'capacity-ready', 'pressure-released', 'denial', 'feedback', 'no-replay', 'capacity-cleanup', 'durable-unchanged']]

CHECKS['reader'] = ['reader-spread-' + check for check in [
    'open', 'double', 'deep-link', 'first-images', 'reload', 'reloaded-images', 'selected-page', 'quality', 'quality-images', 'single']]
CHECKS['ui14-entity-pagination'] = ['entity-seed-pagination', 'entity-fixture', *[
    'entity-' + kind + '-' + order + '-' + check
    for kind in ['group', 'author', 'user'] for order in ['desc', 'asc']
    for check in ['prerequisite', 'open', 'native-last-page', 'page-one-control', 'page-one-route', 'native-first-page']],
    *['entity-empty-' + check for check in ['withdrawal', 'open', 'empty-visible', 'native-empty', 'page-one-control', 'native-first-page']]]

CHECKS['upload'] = ['upload-' + suffix for suffix in ['account-ready', 'begin-control', 'begin-uncertainty', 'begin-staging', 'corrupt-image-input', 'corrupt-image-feedback', 'corrupt-archive-input', 'corrupt-archive-feedback', 'corrupt-no-persisted-pages', 'failed-batch-feedback', 'failed-page-controls', 'cancel-conflict-retained', 'cancel-confirmed', 'stage-selection', 'stage-metadata', 'stage-durable', 'stage-reload', 'replacement-prerequisite', 'replacement-invalid-retained', 'replacement-lost-reply', 'replacement-durable', 'commit-prerequisite', 'commit-response', 'commit-receipt', 'published-reader', 'revert-prerequisite', 'revert-controls', 'page-edit-prerequisite', 'page-edit-controls']]

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

SIGNED_DOWNSTREAM_CHECKS = {'signed-library': ['auth-preview-send',
                    'auth-exact-job-retained',
                    'auth-intent-bound',
                    'auth-unadmitted-retry',
                    'auth-refusal-verified',
                    'admission-exact-retry',
                    'admission-exact-accepted',
                    'admission-exact-projected',
                    'admission-peer-denied'],
 'signed-social': ['social-conversation-loaded',
                   'social-comment-reviewed',
                   'social-comment-sent',
                   'social-comment-counterpart',
                   'social-reaction-reviewed',
                   'social-reaction-sent',
                   'social-reaction-counterpart',
                   'social-feed-verified',
                   'social-comment-rendered'],
 'signed-source-controls': ['source-account-admitted',
                            'source-identity-selected',
                            'source-conversation-loaded',
                            'source-comment-reviewed',
                            'source-snapshot-reviewed',
                            'source-revoked',
                            'source-hidden',
                            'source-mapping-fenced',
                            'source-library-review-fenced',
                            'source-social-review-fenced',
                            'source-state-preserved',
                            'source-restore-prerequisite',
                            'source-restored',
                            'source-restore-mapping',
                            'source-restore-ui'],
 'signed-recipients': ['secondary-admitted',
                       'secondary-remembered',
                       'secondary-selected',
                       'two-snapshot-reviewed',
                       'two-snapshot-published',
                       'two-snapshot-accepted',
                       'two-snapshot-projected',
                       'peers-revoked',
                       'peers-revoked-reads',
                       'peers-restored',
                       'peers-restored-reads']}
for _phase, _suffixes in SIGNED_DOWNSTREAM_CHECKS.items():
    CHECKS[_phase].extend(_phase + '-' + suffix for suffix in _suffixes)

COMPACT_HTTP_OUTCOMES = [
    'success200', 'rate-limited429', 'unavailable503', 'other4xx',
    'other5xx', 'otherstatus', 'transportfailure', 'invalidJSON',
]

COMPACT_DATABASE_FAILURES = [
    'none', 'unavailable', 'pool-wait-timeout', 'pool-create-timeout',
    'pool-recycle-timeout', 'pool-closed', 'pool-backend', 'pool-other',
    'query-canceled', 'query-lock', 'query-connection', 'query-database', 'query-client',
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
    failure = acceptance.get('compact_feed_database_failure')
    if phase == 'ui12-compact-feed' and 'compact_feed_database_failure' in acceptance:
        valid = valid and type(failure) is str and failure in COMPACT_DATABASE_FAILURES
    value = {'schema': 1, 'record_valid': bool(valid), 'private_values_omitted': True,
            'raw_messages_exported': False,
            'check_results': [{'check_id': check_id, 'status': rows[check_id]} for check_id in identifiers]}

    if phase == 'ui12-compact-feed':
        value['compact_feed_http_outcome'] = outcome if type(outcome) is str and outcome in COMPACT_HTTP_OUTCOMES else 'unavailable'
        value['compact_feed_database_failure'] = failure if type(failure) is str and failure in COMPACT_DATABASE_FAILURES else 'unavailable'
    return value
