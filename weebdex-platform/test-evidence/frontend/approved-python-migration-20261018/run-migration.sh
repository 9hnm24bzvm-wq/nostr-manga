#!/usr/bin/env bash
set -euo pipefail
ROOT="$PWD"
E="$PWD/weebdex-platform/test-evidence/frontend/approved-python-migration-20261018"
TARGET="$PWD/weebdex-platform/.build/python-migration-approved-20261018-0556/site-packages"
export PYTHONDONTWRITEBYTECODE=1
export PYTHONPATH="$TARGET:$PWD/manga-protocol-lab:$PWD/manga-protocol-lab/tests"
export WEEBDEX_READONLY_CACHE_ROOTS="$(python3 -c 'import json,sys;print(json.dumps(json.load(open(sys.argv[1]))["roots"]))' "$PWD/weebdex-platform/test-evidence/frontend/complete-discovered-physical-ci-20261018/physical-dependency-root-discovery.json")"
export WEEBDEX_READONLY_AUDIT_DIR="$E/cache-audit/migration"
mkdir -p "$WEEBDEX_READONLY_AUDIT_DIR"
export NODE_OPTIONS="--require=$PWD/weebdex-platform/scripts/semantic-readonly-cache-guard.cjs"
python3 - "$E/runtime.json" "$TARGET" <<'INNER'
import cbor2,coincurve,importlib.metadata,json,pathlib,sys,platform
for module,name,version in [(cbor2,'cbor2','5.6.5'),(coincurve,'coincurve','21.0.0')]:
 assert importlib.metadata.version(name)==version
 assert pathlib.Path(module.__file__).resolve().is_relative_to(pathlib.Path(sys.argv[2]).resolve())
value={'ordinary':'synthetic migration','number':1};assert cbor2.loads(cbor2.dumps(value))==value
key=coincurve.PrivateKey.from_int(1);message=bytes(32);signature=key.sign_schnorr(message);assert coincurve.PublicKeyXOnly.from_secret(key.secret).verify(signature,message)
versions={name:importlib.metadata.version(name)for name in ['cbor2','coincurve','aiohttp','Pillow','cffi']}
report={'python':sys.version,'platform':platform.platform(),'versions':versions,'cborRoundTrip':True,'syntheticSchnorrVerify':True,'importOrigins':{'cbor2':cbor2.__file__,'coincurve':coincurve.__file__},'fullRequirementsParity':False,'runtimeDifference':{'aiohttp':{'required':'3.13.5','actual':versions['aiohttp'],'modified':False}}};pathlib.Path(sys.argv[1]).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
INNER
cd manga-protocol-lab
python3 -m unittest -v test_local_library_migration
