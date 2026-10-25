import hashlib,importlib,importlib.metadata,json,pathlib,sys,unittest
def main():
 out=pathlib.Path(__file__).resolve().parent;root=out.parents[3];rows=json.loads((out/'selected-files.json').read_text())['modules'];suite=unittest.TestSuite();counts={}
 runtime={'python':sys.version,'versions':{},'origins':{},'aiohttpProject':'3.13.5','fullRequirementsParity':False}
 for name in ['cbor2','coincurve','aiohttp','PIL','cffi']:
  module=importlib.import_module(name);path=pathlib.Path(module.__file__);runtime['versions'][name]=importlib.metadata.version('Pillow' if name=='PIL' else name);runtime['origins'][name]={'path':str(path),'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
 (out/'runtime.json').write_text(json.dumps(runtime,indent=2)+'\n')
 for row in rows:
  path=root/row['path'];assert hashlib.sha256(path.read_bytes()).hexdigest()==row['sha256'];name=path.stem;group=unittest.defaultTestLoader.loadTestsFromName(name);assert group.countTestCases()==row['expectedMethods'];counts[name]=group.countTestCases();suite.addTests(group)
 result=unittest.TextTestRunner(verbosity=2).run(suite);report={'files':counts,'testsRun':result.testsRun,'failures':len(result.failures),'errors':len(result.errors),'skipped':len(result.skipped),'failedMethods':[case.id() for case,_ in result.failures+result.errors],'status':'passed' if result.wasSuccessful() else 'failed'};(out/'results.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report));sys.exit(not result.wasSuccessful())
if __name__=='__main__':main()
