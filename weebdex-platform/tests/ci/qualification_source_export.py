"""Explicit, externally pinned finite-source exports. No Git is used by verification.

Origin commit labels are provenance, not ancestry proofs. Complete-repository
and selected-input-closure scopes are deliberately different contracts.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import unicodedata

SCHEMA = 'finite-qualification-source-export-v1'
MAX_MANIFEST_BYTES = 16 * 1024**2
MAX_FILE_BYTES = 64 * 1024**2
MAX_SOURCE_BYTES = 256 * 1024**2
MAX_SOURCE_FILES = 20000
# Complete current source/config roots, plus the historical evidence inputs read
# by finite controls. This is not the entire repository or all archived evidence.
REQUIRED_INPUTS = (
    '.ci', '.github', '.gitignore', 'README.md', 'checkpoint',
    'manga-protocol-lab', 'platform-planning', 'recovery', 'scripts',
    'weebdex-platform-build-environment', 'weebdex-platform/.gitignore',
    *('weebdex-platform/' + name for name in (
        'backend', 'contracts', 'deployment', 'docs', 'frontend',
        'platform-integrations', 'results', 'scripts', 'tests')),
    'weebdex-platform/test-evidence/frontend/browser-actor-follow-plan.json',
    'weebdex-platform/test-evidence/frontend/browser-startup-plan-build19.json',
    'weebdex-platform/test-evidence/frontend/noble-restore/package-provenance.json',
    'weebdex-platform/test-evidence/frontend/source-parity.json',
    'weebdex-platform/test-evidence/native-account-profile-20261018/public_native_profile/cleanup.json',
    'weebdex-platform/test-evidence/public-signed-features-20261022',
)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def git_object(kind, data):
    return hashlib.sha1(kind.encode() + b' ' + str(len(data)).encode() + b'\0' + data).hexdigest()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'Duplicate JSON field: ' + key)
        result[key] = value
    return result


def relative_path(value, *, allow_root=False):
    require(isinstance(value, str), 'Source path must be a string')
    if allow_root and value == '':
        return value
    require(value and not value.startswith('/') and '\\' not in value,
            'Source path must be relative and canonical')
    require(unicodedata.normalize('NFC', value) == value, 'Noncanonical Unicode source path')
    parts = value.split('/')
    require(all(part not in ('', '.', '..', '.git') and not part.endswith(('.', ' '))
                and ':' not in part and all(ord(char) >= 32 and ord(char) != 127 for char in part)
                for part in parts), 'Unsafe source path: ' + value)
    return value


def under(name, parent):
    return not parent or name == parent or name.startswith(parent + '/')


def tree_oid(files):
    """Compute a complete selected Git tree from path/mode/blob identities."""
    tree = {}
    for row in files:
        parts = row['path'].split('/')
        cursor = tree
        for part in parts[:-1]:
            existing = cursor.setdefault(part, {})
            require(isinstance(existing, dict), 'File/directory source identity collision')
            cursor = existing
        require(parts[-1] not in cursor, 'Duplicate source tree identity')
        cursor[parts[-1]] = (row['mode'], row['git_blob'])

    def encode(node):
        values = []
        for name, value in node.items():
            directory = isinstance(value, dict)
            mode, oid = ('40000', encode(value)) if directory else value
            values.append((name.encode('utf8') + (b'/' if directory else b''),
                           mode.encode() + b' ' + name.encode('utf8') + b'\0' + bytes.fromhex(oid)))
        return git_object('tree', b''.join(value for _, value in sorted(values)))
    return encode(tree)


class ExportSource:
    def __init__(self, root, manifest_path, expected_sha256):
        self.root = Path(root).resolve(strict=True)
        self.manifest_path = Path(manifest_path).resolve(strict=True)
        require(not self.manifest_path.is_relative_to(self.root), 'Export manifest must be outside the source tree')
        require(isinstance(expected_sha256, str) and re.fullmatch('[0-9a-f]{64}', expected_sha256),
                'An independent expected export-manifest SHA-256 is required')
        self.expected_sha256 = expected_sha256
        with self.manifest_path.open('rb') as source:
            data = source.read(MAX_MANIFEST_BYTES + 1)
        require(len(data) <= MAX_MANIFEST_BYTES and sha256(data) == expected_sha256,
                'Export manifest SHA-256 differs from the externally supplied witness')
        self.manifest = json.loads(data, object_pairs_hook=unique_object)
        manifest = self.manifest
        require(set(manifest) == {'schema', 'scope', 'origin', 'selection', 'files'}, 'Invalid export manifest fields')
        require(manifest['schema'] == SCHEMA and manifest['scope'] in ('repository-tree', 'selected-input-closure'),
                'Unsupported export manifest scope')
        require(set(manifest['origin']) == {'commit', 'tree'} and all(
            isinstance(value, str) and re.fullmatch('[0-9a-f]{40}', value) for value in manifest['origin'].values()),
            'Invalid declared origin provenance')
        require(isinstance(manifest['selection'], list) and manifest['selection'], 'Missing explicit source selection')
        require(isinstance(manifest['files'], list) and 0 < len(manifest['files']) <= MAX_SOURCE_FILES, 'Invalid source-file count')
        self.files = {}
        aliases = {}
        source_bytes = 0
        for row in manifest['files']:
            require(isinstance(row, dict) and set(row) == {'path', 'mode', 'git_blob', 'bytes', 'sha256'},
                    'Invalid source record fields')
            name = relative_path(row['path'])
            require(name not in self.files, 'Duplicate source identity: ' + name)
            require(row['mode'] in ('100644', '100755'), 'Only regular or executable source files are supported')
            require(type(row['bytes']) is int and 0 <= row['bytes'] <= MAX_FILE_BYTES, 'Invalid source byte count')
            source_bytes += row['bytes']
            require(source_bytes <= MAX_SOURCE_BYTES, 'Aggregate source byte limit exceeded')
            require(re.fullmatch('[0-9a-f]{40}', row['git_blob'] or '') and
                    re.fullmatch('[0-9a-f]{64}', row['sha256'] or ''), 'Invalid source digest')
            for index in range(1, len(name.split('/')) + 1):
                component = '/'.join(name.split('/')[:index])
                old = aliases.setdefault(component.casefold(), component)
                require(old == component, 'Case-ambiguous source identities')
            self.files[name] = row
        require(list(self.files) == sorted(self.files), 'Source records must be canonically sorted')
        selections = []
        covered = set()
        for selected in manifest['selection']:
            require(isinstance(selected, dict) and set(selected) == {'path', 'kind', 'git_oid'}, 'Invalid selection fields')
            name = relative_path(selected['path'], allow_root=True)
            require(selected['kind'] in ('tree', 'blob') and re.fullmatch('[0-9a-f]{40}', selected['git_oid'] or ''),
                    'Invalid selected object identity')
            require(not any(under(name, old) or under(old, name) for old in selections), 'Overlapping source selections')
            selections.append(name)
            chosen = [row for file, row in self.files.items() if under(file, name)]
            require(chosen, 'Selected source object is missing')
            if selected['kind'] == 'blob':
                require(len(chosen) == 1 and chosen[0]['path'] == name, 'Invalid selected blob coverage')
                oid = chosen[0]['git_blob']
            else:
                require(not name or all(row['path'] != name for row in chosen), 'Selected tree is a file')
                oid = tree_oid([dict(row, path=row['path'][len(name) + 1:] if name else row['path']) for row in chosen])
            require(oid == selected['git_oid'], 'Selected source object identity differs: ' + name)
            covered.update(row['path'] for row in chosen)
        require(selections == sorted(selections) and covered == set(self.files), 'Incomplete or unordered source selection')
        if manifest['scope'] == 'repository-tree':
            require(manifest['selection'] == [{'path': '', 'kind': 'tree', 'git_oid': manifest['origin']['tree']}],
                    'Repository-tree scope must prove the entire declared tree')
        else:
            require('' not in selections, 'Selected scope cannot masquerade as a full repository tree')
            require(all(any(under(required, selected) for selected in selections) for required in REQUIRED_INPUTS),
                    'Selected export omits a required finite source, fixture, config or evidence input')
        self.runtime = {}
        self.runtime_before = None
        self.dependency_controls = {}
        self.dependency_controls_before = None

    def provenance(self):
        return {'kind': 'externally-pinned-source-export', 'scope': self.manifest['scope'],
                'manifest_sha256': self.expected_sha256,
                'declared_origin_commit': self.manifest['origin']['commit'],
                'declared_origin_repository_tree': self.manifest['origin']['tree'],
                'origin_commit_ancestry_verified': False,
                'complete_origin_repository_tree_verified': self.manifest['scope'] == 'repository-tree',
                'selected_object_identities_verified': True, 'source_files': len(self.files)}

    def bind_dependency_controls(self, frontend):
        """The external package resolver/lock must match exported canonical inputs."""
        require(not self.dependency_controls, 'Dependency controls may be bound only once')
        for name in ('package.json', 'yarn.lock'):
            key = 'weebdex-platform/frontend/' + name
            require(key in self.files, 'Export omits canonical frontend dependency control: ' + name)
            self.dependency_controls[str(Path(frontend).resolve(strict=True) / name)] = self.files[key]
        self.dependency_controls_before = self.dependency_control_inventory()

    def dependency_control_inventory(self):
        result = {}
        for name, expected in self.dependency_controls.items():
            path = Path(name)
            status = path.lstat()
            require(stat.S_ISREG(status.st_mode), 'Dependency control must be a regular file')
            data = path.read_bytes()
            require(len(data) == expected['bytes'] and sha256(data) == expected['sha256'],
                    'External dependency control differs from exported canonical source')
            result[name] = {'bytes': len(data), 'sha256': sha256(data),
                            'mode': stat.S_IMODE(status.st_mode), 'mtime_ns': status.st_mtime_ns}
        return result

    def bind_runtime(self, roots):
        require(not self.runtime, 'Runtime source exclusions may be bound only once')
        for path in roots:
            path = Path(path).absolute()
            require(path.is_relative_to(self.root), 'Runtime path must be inside the export')
            name = relative_path(str(path.relative_to(self.root)))
            require(not any(under(file, name) or under(name, file) for file in self.files), 'Runtime path overlaps source')
            require(not any(under(name, old) or under(old, name) for old in self.runtime), 'Overlapping runtime roots')
            require(path.exists() or path.is_symlink(), 'Expected runtime root is missing')
            self.runtime[name] = path
        self.runtime_before = self.runtime_inventory()

    def runtime_inventory(self):
        result = {}
        def visit(path, relative):
            status = path.lstat()
            row = {'mode': stat.S_IMODE(status.st_mode)}
            if stat.S_ISLNK(status.st_mode):
                row.update(kind='symlink', target=os.readlink(path))
            elif stat.S_ISREG(status.st_mode):
                data = path.read_bytes()
                row.update(kind='file', bytes=len(data), sha256=sha256(data))
            elif stat.S_ISDIR(status.st_mode):
                row['kind'] = 'directory'
                for child in sorted(path.iterdir()):
                    visit(child, relative + '/' + child.name)
            else:
                raise ValueError('Unsupported runtime entry')
            result[relative] = row
        for name, path in sorted(self.runtime.items()):
            visit(path, name)
        return result

    def verify_files(self, names):
        """Verify an exact consumer subset, not a complete exported tree.

        The outer runner retains full before/after verification. Consumers use
        this bounded check for actual driver inputs and must label its scope.
        """
        with self.manifest_path.open('rb') as source:
            encoded = source.read(MAX_MANIFEST_BYTES + 1)
        require(len(encoded) <= MAX_MANIFEST_BYTES and sha256(encoded) == self.expected_sha256,
                'Export manifest changed during execution')
        require(isinstance(names, (list, tuple)) and names and len(set(names)) == len(names),
                'Consumer source subset must be explicit and unique')
        found = {}
        for name in names:
            name = relative_path(name)
            require(name in self.files, 'Consumer source missing from export manifest: ' + name)
            path = self.root / name
            for component in (path, *path.parents):
                if component == self.root:
                    break
                require(not component.is_symlink(), 'Consumer source symlink is unsupported: ' + name)
            status = path.lstat()
            row = self.files[name]
            require(stat.S_ISREG(status.st_mode), 'Consumer source must be regular: ' + name)
            require(stat.S_IMODE(status.st_mode) == int(row['mode'][-3:], 8), 'Consumer source mode changed: ' + name)
            require(status.st_size == row['bytes'], 'Consumer source byte count changed: ' + name)
            data = path.read_bytes()
            require(sha256(data) == row['sha256'] and git_object('blob', data) == row['git_blob'],
                    'Consumer source bytes changed: ' + name)
            found[name] = row['sha256']
        return found

    def verify(self):
        with self.manifest_path.open('rb') as source:
            manifest_bytes = source.read(MAX_MANIFEST_BYTES + 1)
        require(len(manifest_bytes) <= MAX_MANIFEST_BYTES and sha256(manifest_bytes) == self.expected_sha256,
                'Export manifest changed during execution')
        found = {}
        def visit(directory):
            for path in sorted(directory.iterdir()):
                name = relative_path(str(path.relative_to(self.root)))
                if name in self.runtime:
                    continue
                status = path.lstat()
                require(not stat.S_ISLNK(status.st_mode), 'Source symlinks are unsupported: ' + name)
                if stat.S_ISDIR(status.st_mode):
                    visit(path)
                else:
                    require(stat.S_ISREG(status.st_mode), 'Nonregular source entry: ' + name)
                    require(name in self.files, 'Unlisted source file: ' + name)
                    row = self.files[name]
                    require(stat.S_IMODE(status.st_mode) == int(row['mode'][-3:], 8), 'Source mode changed: ' + name)
                    require(status.st_size == row['bytes'], 'Source byte count changed: ' + name)
                    data = path.read_bytes()
                    require(sha256(data) == row['sha256'] and git_object('blob', data) == row['git_blob'],
                            'Source bytes changed: ' + name)
                    found[name] = row['sha256']
        visit(self.root)
        require(set(found) == set(self.files), 'Required source file is missing')
        if self.dependency_controls_before is not None:
            require(self.dependency_control_inventory() == self.dependency_controls_before,
                    'External dependency control metadata changed')
        if self.runtime_before is not None:
            require(self.runtime_inventory() == self.runtime_before, 'Owned runtime files or dependency aliases changed')
        return found


def prepare_manifest(repo, commit, scope):
    """Read a committed source tree, not a dirty worktree. Preparation needs Git;
    verification and execution of the resulting export do not. No files copied.
    """
    import subprocess
    require(scope in ('repository-tree', 'selected-input-closure'), 'Unsupported export preparation scope')
    repo = Path(repo).resolve(strict=True)
    def git(*args):
        return subprocess.check_output(['git', *args], cwd=repo, timeout=60)
    origin = git('rev-parse', '--verify', commit + '^{commit}').decode().strip()
    require(re.fullmatch('[0-9a-f]{40}', origin), 'Invalid committed source identity')
    root_tree = git('rev-parse', origin + '^{tree}').decode().strip()
    selections = []
    for name in sorted(REQUIRED_INPUTS) if scope == 'selected-input-closure' else ['']:
        reference = origin + ':' + name if name else origin + '^{tree}'
        oid = git('rev-parse', '--verify', reference).decode().strip()
        kind = git('cat-file', '-t', oid).decode().strip()
        require(kind in ('tree', 'blob'), 'Unsupported selected source object')
        selections.append({'path': name, 'kind': kind, 'git_oid': oid})
    arguments = ['ls-tree', '-rz', '--full-tree', origin]
    if scope == 'selected-input-closure':
        arguments += ['--', *sorted(REQUIRED_INPUTS)]
    files = []
    for encoded in git(*arguments).split(b'\0'):
        if not encoded:
            continue
        metadata, name = encoded.split(b'\t', 1)
        mode, kind, oid = metadata.decode().split()
        require(kind == 'blob' and mode in ('100644', '100755'), 'Source export supports no symlinks or submodules')
        files.append({'path': relative_path(name.decode('utf8')), 'mode': mode, 'git_blob': oid})
    require(0 < len(files) <= MAX_SOURCE_FILES, 'Invalid source-file count')
    files.sort(key=lambda row: row['path'])
    query = ''.join(row['git_blob'] + '\n' for row in files).encode()
    # Validate exact metadata before capturing any blob content. Git object IDs
    # are immutable; duplicate blob IDs still count once per selected file.
    checked = subprocess.run(['git', 'cat-file', '--batch-check'], input=query, cwd=repo,
                             capture_output=True, timeout=60, check=True).stdout.splitlines()
    require(len(checked) == len(files), 'Committed-object preflight count differs')
    total = 0
    for row, encoded in zip(files, checked):
        values = encoded.decode().split()
        require(len(values) == 3, 'Invalid committed-object preflight record')
        oid, kind, size = values
        require(size.isdecimal(), 'Invalid committed-object preflight byte count')
        size = int(size)
        require(oid == row['git_blob'] and kind == 'blob' and 0 <= size <= MAX_FILE_BYTES,
                'Unexpected committed-object preflight identity, type or size')
        total += size
        require(total <= MAX_SOURCE_BYTES, 'Aggregate source byte limit exceeded')
        row['bytes'] = size
    result = subprocess.run(['git', 'cat-file', '--batch'], input=query, cwd=repo, capture_output=True, timeout=60, check=True)
    payload, offset = result.stdout, 0
    for row in files:
        end = payload.index(b'\n', offset)
        oid, kind, size = payload[offset:end].decode().split()
        size = int(size)
        require(oid == row['git_blob'] and kind == 'blob' and size == row['bytes'], 'Unexpected committed blob')
        offset = end + 1
        data = payload[offset:offset + size]
        require(len(data) == size and payload[offset + size:offset + size + 1] == b'\n' and
                git_object('blob', data) == oid, 'Incomplete committed blob')
        row.update(bytes=size, sha256=sha256(data))
        offset += size + 1
    require(offset == len(payload), 'Unexpected extra committed-object bytes')
    return {'schema': SCHEMA, 'scope': scope, 'origin': {'commit': origin, 'tree': root_tree},
            'selection': selections, 'files': files}


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(description='Prepare a source manifest for separate review; never run qualification or copy sources.')
    parser.add_argument('--prepare-from-git', type=Path, required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--scope', choices=['repository-tree', 'selected-input-closure'], required=True)
    parser.add_argument('--output', type=Path, required=True, help='New manifest outside the source/export tree')
    args = parser.parse_args()
    require(not args.output.resolve().is_relative_to(args.prepare_from_git.resolve()), 'Manifest output must be outside the source tree')
    manifest = prepare_manifest(args.prepare_from_git, args.commit, args.scope)
    encoded = (json.dumps(manifest, indent=2) + '\n').encode()
    require(len(encoded) <= MAX_MANIFEST_BYTES, 'Export manifest size limit exceeded')
    with args.output.open('xb') as output:
        output.write(encoded)
    # Validate structure and selected object coverage, without requiring a full
    # local checkout or claiming these worktree bytes match the selected commit.
    source = ExportSource(args.prepare_from_git, args.output, sha256(encoded))
    print(json.dumps({'manifest_sha256': sha256(encoded), 'manifest_bytes': len(encoded),
                      'source_files': len(manifest['files']), 'source_bytes': sum(row['bytes'] for row in manifest['files']),
                      'scope': manifest['scope'], 'declared_origin_commit': manifest['origin']['commit'],
                      'worktree_bytes_verified': False, 'qualification_executed': False}, indent=2))
