import importlib.util,os,pathlib,sys
W=pathlib.Path(__file__).resolve().parents[2];P=W/'weebdex-platform';D=P/'.build/approved-dependency-restore-20261018';E=W/'.build/frontend-current-restore';NODE=pathlib.Path('/opt/codex/runtimes/codex-primary-runtime/dependencies/node/bin/node')
spec=importlib.util.spec_from_file_location('owner',E/'owned-guard.py');owner=importlib.util.module_from_spec(spec);spec.loader.exec_module(owner)
keep={k:v for k,v in os.environ.items()if k in ['LANG','LC_ALL','LC_CTYPE','SSL_CERT_FILE','SSL_CERT_DIR','NODE_EXTRA_CA_CERTS','HTTP_PROXY','HTTPS_PROXY','NO_PROXY','http_proxy','https_proxy','no_proxy']}
os.environ.clear();os.environ.update(keep,PATH=str(NODE.parent)+':/usr/bin:/bin',HOME=str(D/'home'),TMPDIR=str(D/'tmp'),PYTHONDONTWRITEBYTECODE='1',npm_config_cache=str(D/'npm-cache'),npm_config_audit='false',npm_config_fund='false',npm_config_ignore_scripts='true',npm_config_userconfig=str(D/'user.npmrc'),npm_config_globalconfig=str(D/'global.npmrc'),COREPACK_ENABLE_NETWORK='0')
(D/'tmp').mkdir(exist_ok=True)
label=sys.argv[1];phase=label.split('-retry',1)[0]
if phase=='bootstrap':argv=[str(NODE),str(NODE.parent/'../lib/node_modules/npm/bin/npm-cli.js'),'ci','--ignore-scripts','--no-audit','--no-fund'];cwd=D/'yarn-bootstrap';seconds=60
elif phase=='frontend':argv=[str(NODE),str(D/'yarn-bootstrap/node_modules/yarn/bin/yarn.js'),'install','--frozen-lockfile','--ignore-scripts','--non-interactive','--no-default-rc','--cache-folder',str(D/'yarn-cache')];cwd=D/'frontend';seconds=600
elif phase=='signers':argv=[sys.executable,str(D/'signer-host/weebdex-platform/scripts/restore-local-library-noble.py')];cwd=D;seconds=90
else:raise SystemExit('Unknown phase')
raise SystemExit(owner.run(argv,cwd,E/(label+'-owned.json'),seconds,[D],0))
