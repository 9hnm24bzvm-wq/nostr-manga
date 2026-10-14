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
    'ui11-device-progress': ['device-progress-signup-context', *[
        'device-progress-' + gate + '-page' + str(page) + '-' + check
        for gate in ['timer', 'lock'] for page in [1, 3]
        for check in ['setup', 'select-page', 'no-prior-progress', 'gate-captured',
                      'old-intent-cancelled', 'page-retained', 'durable-progress',
                      'reload-progress', 'stable-revision']
    ]],
    'ui12-compact-feed': ['compact-feed-' + check for check in [
        'seed-pagination', 'spanish-preferences', 'fixture-total', 'open-updates',
        'third-page', 'resume-visible', 'bounded-burst', 'compact-geometry',
        'trimmed-cards', 'manual-terminal-page', 'unique-sequential-pages',
        'scenario-membership', 'all-compact-chapters', 'terminal-idle',
        'resize-retains-entries', 'back-restores-entries',
    ]],
}
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
    return {'schema': 1, 'record_valid': bool(valid), 'private_values_omitted': True,
            'raw_messages_exported': False,
            'check_results': [{'check_id': check_id, 'status': rows[check_id]} for check_id in identifiers]}
