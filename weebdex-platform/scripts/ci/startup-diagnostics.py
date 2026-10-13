"""Project private startup observations into fixed, non-personal categories."""
from collections import Counter
import re
from urllib.parse import urlsplit

CHECKS = ['homepage', 'search', 'groups', 'users', 'popular', 'trending', 'updates', 'announcements', 'about', 'guidelines', 'terms', 'privacy', 'login', 'register', 'random-title', 'seed-title', 'seed-group', 'seed-chapter', 'missing-assets']
ROUTES = ['/', '/search', '/groups', '/users', '/popular', '/trending', '/updates', '/announcements', '/about', '/guidelines', '/terms', '/privacy', '/auth/login', '/auth/register', '/title/random', '/title/SeedManga1', '/group/SeedGroup1', '/chapter/SeedChapt1']
CHECK_NAMES = ['homepage bootstraps under exact generated CSP', *['fresh deep link ' + route for route in ROUTES[1:]], 'missing built/static assets produce real 404']
PATHS = set(ROUTES) | {'/_app/immutable/entry/<module>.js', '/_app/immutable/chunks/<module>.js', '/_app/immutable/nodes/<module>.js', '/_app/immutable/assets/<asset>', '/_app/<asset>', '/fonts/<asset>', '/images/<asset>', '/service-worker.js', '/manifest.webmanifest', '/api/<endpoint>', '/auth/<endpoint>', '/<unlisted-path>', '<other-origin>', '<unavailable>'}
CLASSES = {'None', 'Error', 'TypeError', 'ReferenceError', 'SyntaxError', 'RangeError', 'EvalError', 'AggregateError', 'TimeoutError', 'Unknown'}
GLOBALS = {'process', 'Buffer', 'require', 'module', 'global', 'exports', 'window', 'document', 'navigator', 'crypto', 'indexedDB', 'localStorage', '$state', '$derived', '$effect', '$props', '$inspect', '$bindable'}
CATEGORIES = {'none', 'module-fetch', 'module-export', 'module-specifier', 'module-mime', 'missing-global', 'reference-error', 'undefined-property', 'non-callable-value', 'svelte-runtime', 'csp-blocked', 'bootstrap-csp-missing', 'bootstrap-csp-unsafe', 'document-status', 'locator-or-navigation-timeout', 'no-rendered-content', 'page-javascript', 'syntax-error', 'runtime-error', 'unknown'}
DIRECTIVES = {'script-src', 'script-src-elem', 'script-src-attr', 'style-src', 'style-src-elem', 'style-src-attr', 'connect-src', 'img-src', 'font-src', 'worker-src', 'manifest-src', 'frame-src', 'base-uri', 'form-action', 'default-src', 'unknown'}
RESOURCES = {'document', 'script', 'stylesheet', 'fetch', 'xhr', 'image', 'font', 'other'}
MIMES = {'html', 'javascript', 'json', 'css', 'image', 'font', 'octet-stream', 'empty', 'other'}
FAILURES = {'none', 'blocked-client', 'blocked-response', 'aborted', 'connection-refused', 'connection-reset', 'network-failed', 'unknown'}
LIMIT = 256


def public_path(value, origin):
    try:
        if type(value) is not str or type(origin) is not str:
            return '<unavailable>'
        target, expected = urlsplit(value), urlsplit(origin)
        if expected.scheme != 'http' or expected.hostname not in ['127.0.0.1', 'localhost'] or not expected.port or expected.username or expected.password:
            return '<unavailable>'
        if (target.scheme, target.netloc) != (expected.scheme, expected.netloc):
            return '<other-origin>'
        path = target.path
        if path in ROUTES or path in ['/service-worker.js', '/manifest.webmanifest']:
            return path
        for prefix, label in [('/_app/immutable/entry/', '/_app/immutable/entry/<module>.js'), ('/_app/immutable/chunks/', '/_app/immutable/chunks/<module>.js'), ('/_app/immutable/nodes/', '/_app/immutable/nodes/<module>.js'), ('/_app/immutable/assets/', '/_app/immutable/assets/<asset>'), ('/_app/', '/_app/<asset>'), ('/fonts/', '/fonts/<asset>'), ('/images/', '/images/<asset>'), ('/api/', '/api/<endpoint>'), ('/auth/', '/auth/<endpoint>')]:
            if path.startswith(prefix):
                return label
        return '/<unlisted-path>'
    except (TypeError, ValueError):
        return '<unavailable>'


def error_kind(message, name=None):
    message = message if type(message) is str else ''
    kind = name if type(name) is str and name in CLASSES - {'None'} else 'Unknown'
    patterns = [
        (r'Failed to fetch dynamically imported module|Importing a module script failed|error loading dynamically imported module', 'module-fetch'),
        (r'does not provide an export named', 'module-export'),
        (r'Failed to resolve module specifier', 'module-specifier'),
        (r'MIME type|JavaScript-or-Wasm module', 'module-mime'),
        (r'Cannot read properties of (?:undefined|null)|Cannot convert undefined or null', 'undefined-property'),
        (r'is not a function', 'non-callable-value'),
        (r'https://svelte\.dev/e/|rune_outside_svelte|effect_orphan|lifecycle_outside_component', 'svelte-runtime'),
        (r'Served CSP is missing current exact bootstrap hash', 'bootstrap-csp-missing'),
        (r'Script CSP unexpectedly weakened', 'bootstrap-csp-unsafe'),
        (r'Content Security Policy|unsafe-eval|Refused to (?:execute|load)', 'csp-blocked'),
        (r'Navigation returned', 'document-status'),
        (r'Timeout|timed out|timeout', 'locator-or-navigation-timeout'),
        (r'Page never rendered meaningful content', 'no-rendered-content'),
        (r'JavaScript page errors', 'page-javascript'),
    ]
    # Try only at an identifier boundary, not at every suffix of a long token.
    # Otherwise an unrecognized 9000-character private value causes quadratic
    # rescans before it is safely reduced to a fixed category.
    missing = re.search(r'(?<![\w$])([A-Za-z_$][\w$]*) is not defined', message)
    if missing:
        return {'error_class': 'ReferenceError', 'category': 'missing-global' if missing[1] in GLOBALS else 'reference-error', 'missing_global': missing[1] if missing[1] in GLOBALS else 'none'}
    category = next((category for pattern, category in patterns if re.search(pattern, message, re.I)), 'syntax-error' if kind == 'SyntaxError' else 'runtime-error' if kind != 'Unknown' else 'unknown')
    if category in ['undefined-property', 'non-callable-value']:
        kind = 'TypeError'
    return {'error_class': kind, 'category': category, 'missing_global': 'none'}


def mime_kind(value):
    value = value.lower().split(';', 1)[0].strip() if type(value) is str else ''
    if not value:
        return 'empty'
    if value in ['application/javascript', 'text/javascript', 'application/ecmascript', 'text/ecmascript']:
        return 'javascript'
    if value in ['text/html', 'application/json', 'text/css', 'application/octet-stream']:
        return {'text/html': 'html', 'application/json': 'json', 'text/css': 'css', 'application/octet-stream': 'octet-stream'}[value]
    return 'image' if value.startswith('image/') else 'font' if value.startswith('font/') else 'other'


def failure_kind(value):
    mapping = {'net::ERR_BLOCKED_BY_CLIENT': 'blocked-client', 'net::ERR_BLOCKED_BY_RESPONSE': 'blocked-response', 'net::ERR_ABORTED': 'aborted', 'net::ERR_CONNECTION_REFUSED': 'connection-refused', 'net::ERR_CONNECTION_RESET': 'connection-reset', 'net::ERR_FAILED': 'network-failed'}
    return mapping.get(value, 'none' if not value else 'unknown') if type(value) is str or value is None else 'unknown'


def grouped(rows, keys):
    counts = Counter(tuple(row[key] for key in keys) for row in rows)
    return [dict(zip(keys, values), count=count) for values, count in sorted(counts.items())]


def project(acceptance):
    origin = acceptance.get('origin', '')
    checks = []
    steps = acceptance.get('steps', [])
    for index, row in enumerate(steps[:len(CHECKS)]):
        status = row.get('status') if row.get('status') in ['passed', 'failed'] else 'unresolved'
        kind = {'error_class': 'None', 'category': 'none', 'missing_global': 'none'} if status == 'passed' else error_kind(row.get('error'))
        checks.append(dict(index=index, check_id=CHECKS[index], check_identity_verified=row.get('name') == CHECK_NAMES[index], status=status, **kind))
    output = {'schema': 1, 'check_results': checks, 'private_values_omitted': True, 'raw_messages_exported': False, 'observations_truncated': len(steps) > len(CHECKS) or acceptance.get('startup_observations_truncated') is True}
    for field in ['page_errors', 'console_errors']:
        rows = acceptance.get(field, [])
        output['observations_truncated'] |= len(rows) > LIMIT
        projected = [dict(path=public_path(row.get('url'), origin), **error_kind(row.get('error'), row.get('error_class'))) for row in rows[:LIMIT]]
        output[field] = grouped(projected, ['path', 'error_class', 'category', 'missing_global'])
    violations = acceptance.get('csp_violations', [])
    output['observations_truncated'] |= len(violations) > LIMIT
    projected = []
    for row in violations[:LIMIT]:
        blocked = row.get('blocked_uri')
        directive = row.get('directive')
        projected.append({'path': public_path(row.get('page'), origin), 'directive': directive if type(directive) is str and directive in DIRECTIVES else 'unknown', 'disposition': row.get('disposition') if row.get('disposition') in ['enforce', 'report'] else 'unknown', 'blocked': blocked if blocked in ['inline', 'eval'] else 'same-origin' if public_path(blocked, origin) not in ['<other-origin>', '<unavailable>'] else 'other-origin'})
    output['csp_violations'] = grouped(projected, ['path', 'directive', 'disposition', 'blocked'])
    network = acceptance.get('startup_network', [])
    output['observations_truncated'] |= len(network) > LIMIT
    projected = []
    for row in network[:LIMIT]:
        status = row.get('status')
        resource = row.get('resource')
        projected.append({'path': public_path(row.get('url'), origin), 'resource': resource if type(resource) is str and resource in RESOURCES else 'other', 'status': status if type(status) is int and 100 <= status <= 599 else 0, 'mime': mime_kind(row.get('content_type')), 'failure': failure_kind(row.get('failure'))})
    output['network'] = grouped(projected, ['path', 'resource', 'status', 'mime', 'failure'])
    return output
