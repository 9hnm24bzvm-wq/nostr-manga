import ast, pathlib, re
source = pathlib.Path('weebdex-platform/tests/runtime/browser_fixture.py')
tree = ast.parse(source.read_text(), filename=str(source))
helper = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == 'public_upload_payload')
namespace = {'re': re}
exec(compile(ast.Module(body=[helper], type_ignores=[]), str(source), 'exec'), namespace)
validate = namespace['public_upload_payload']
public = 'a' * 64
session = '00000000-0000-8000-8000-000000000001'
assert validate('upload-account', {'pubkey': public}) == {'pubkey': public}
assert validate('upload-snapshot', {'session_id': session}) == {'session_id': session}
cases = [('upload-account', {}), ('upload-account', {'pubkey': 7}), ('upload-account', {'pubkey': public, 'private_key': 'excluded'}), ('upload-account', {'pubkey': public, 'manga_id': session}), ('upload-snapshot', {}), ('upload-snapshot', {'session_id': 7}), ('upload-snapshot', {'session_id': '../invalid'}), ('unknown-action', {})]
for action, payload in cases:
    try:
        validate(action, payload)
    except ValueError:
        continue
    raise AssertionError('unexpected public upload fixture payload acceptance')
print('Python fixture AST syntax and 10 pure payload cases passed')
