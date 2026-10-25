import pathlib,subprocess,hashlib,json,sys
root=pathlib.Path.cwd();out=pathlib.Path(__file__).resolve().parent;phase=sys.argv[1]
files=subprocess.check_output(['git','ls-files','-z'],text=True).split('\0')
prefixes=('weebdex-platform/frontend/','weebdex-platform/platform-integrations/','weebdex-platform/scripts/','weebdex-platform/contracts/')
products={name:hashlib.sha256((root/name).read_bytes()).hexdigest() for name in files if name.startswith(prefixes) and (root/name).is_file()}
execution={name:hashlib.sha256((root/name).read_bytes()).hexdigest() for name in files if name.startswith(prefixes+('weebdex-platform/tests/ci/','manga-protocol-lab/browser-client/','manga-protocol-lab/tests/','manga-protocol-lab/scripts/','manga-protocol-lab/bench/library-node/')) and (root/name).is_file()}
for name,data in [('product-source-pins',products),('execution-source-pins',execution)]:
 (out/(name+'-'+phase+'.json')).write_text(json.dumps(data,indent=2)+'\n')
 if phase!='before':assert data==json.loads((out/(name+'-before.json')).read_text()), name+' changed'
if phase=='before':(out/'strict-product-source-pins.json').write_text(json.dumps(products,indent=2)+'\n')
print('phase',phase,'product',len(products),'execution',len(execution))
